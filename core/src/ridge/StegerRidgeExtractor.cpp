#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/ridge/HessianMath.hpp"
#include <algorithm>
#include <cmath>
#include <deque>
#include <opencv2/imgproc.hpp>

namespace CurvEngine::ridge {

namespace {

struct CandidatePoint {
    bool valid{false};
    float px{0.0f};
    float py{0.0f};
    float salience{0.0f};
    float nx{0.0f};
    float ny{0.0f};
    float tx{0.0f}; // Tangent X = -ny
    float ty{0.0f}; // Tangent Y =  nx
    float intensity{0.0f};
    float confidence{0.0f};
};

void generate1DGaussianKernels(float sigma, cv::Mat& k_g, cv::Mat& k_g1, cv::Mat& k_g2) {
    const float safe_sigma = std::max(0.1f, sigma);
    const int radius = std::max(2, static_cast<int>(std::ceil(3.0f * safe_sigma)));
    const int ksize = 2 * radius + 1;

    k_g.create(1, ksize, CV_32F);
    k_g1.create(1, ksize, CV_32F);
    k_g2.create(1, ksize, CV_32F);

    const float s2 = safe_sigma * safe_sigma;
    const float s4 = s2 * s2;

    double sum_g = 0.0;
    double sum_x_g1 = 0.0;
    double sum_g2 = 0.0;

    for (int i = 0; i < ksize; ++i) {
        const float x = static_cast<float>(i - radius);
        const float g = std::exp(-0.5f * (x * x) / s2);
        // For correlation: positive slope should yield positive correlation
        const float g1 = (x / s2) * g;
        const float g2 = ((x * x - s2) / s4) * g;

        k_g.at<float>(0, i) = g;
        k_g1.at<float>(0, i) = g1;
        k_g2.at<float>(0, i) = g2;

        sum_g += g;
        sum_x_g1 += static_cast<double>(x) * g1;
        sum_g2 += g2;
    }

    // Zero-mean enforcement for 2nd derivative
    const float mean_g2 = static_cast<float>(sum_g2 / ksize);
    double sum_x2_g2 = 0.0;
    for (int i = 0; i < ksize; ++i) {
        const float x = static_cast<float>(i - radius);
        k_g2.at<float>(0, i) -= mean_g2;
        sum_x2_g2 += 0.5 * static_cast<double>(x * x) * k_g2.at<float>(0, i);
    }

    // Normalize discrete kernels
    if (sum_g != 0.0) k_g /= static_cast<float>(sum_g);
    if (sum_x_g1 != 0.0) k_g1 /= static_cast<float>(sum_x_g1);
    if (sum_x2_g2 != 0.0) k_g2 /= static_cast<float>(sum_x2_g2);
}

} // anonymous namespace

StegerRidgeExtractor::StegerRidgeExtractor(StegerConfig config) : config_(config) {}

RidgeGraph StegerRidgeExtractor::extractRidgeGraph(const cv::Mat& image) const {
    RidgeGraph result;
    if (image.empty()) {
        return result;
    }

    // 1. Grayscale Conversion and Float32 Normalization
    cv::Mat gray;
    if (image.channels() == 3) {
        cv::cvtColor(image, gray, cv::COLOR_BGR2GRAY);
    } else if (image.channels() == 4) {
        cv::cvtColor(image, gray, cv::COLOR_BGRA2GRAY);
    } else {
        gray = image;
    }

    cv::Mat gray_f;
    gray.convertTo(gray_f, CV_32F);

    const int rows = gray.rows;
    const int cols = gray.cols;
    result.debug_mask = cv::Mat::zeros(rows, cols, CV_8UC1);

    // 2. Generate Separable 1D Gaussian Derivative Kernels
    cv::Mat k_g;
    cv::Mat k_g1;
    cv::Mat k_g2;
    generate1DGaussianKernels(config_.sigma, k_g, k_g1, k_g2);

    // 3. Compute 1st and 2nd Order Partial Derivatives via Separable Filtering
    cv::Mat rx;
    cv::Mat ry;
    cv::Mat rxx;
    cv::Mat ryy;
    cv::Mat rxy;
    cv::sepFilter2D(gray_f, rx, CV_32F, k_g1, k_g, cv::Point(-1, -1), 0.0, cv::BORDER_REPLICATE);
    cv::sepFilter2D(gray_f, ry, CV_32F, k_g, k_g1, cv::Point(-1, -1), 0.0, cv::BORDER_REPLICATE);
    cv::sepFilter2D(gray_f, rxx, CV_32F, k_g2, k_g, cv::Point(-1, -1), 0.0, cv::BORDER_REPLICATE);
    cv::sepFilter2D(gray_f, ryy, CV_32F, k_g, k_g2, cv::Point(-1, -1), 0.0, cv::BORDER_REPLICATE);
    cv::sepFilter2D(gray_f, rxy, CV_32F, k_g1, k_g1, cv::Point(-1, -1), 0.0, cv::BORDER_REPLICATE);

    // 4. Multi-Threaded Pixel-wise Hessian Decomposition & Sub-Pixel Localization via cv::parallel_for_
    std::vector<CandidatePoint> candidates(static_cast<size_t>(rows * cols));

    cv::parallel_for_(cv::Range(0, rows), [&](const cv::Range& range) {
        for (int y = range.start; y < range.end; ++y) {
            const float* p_rx = rx.ptr<float>(y);
            const float* p_ry = ry.ptr<float>(y);
            const float* p_rxx = rxx.ptr<float>(y);
            const float* p_ryy = ryy.ptr<float>(y);
            const float* p_rxy = rxy.ptr<float>(y);
            const float* p_val = gray_f.ptr<float>(y);

            for (int x = 0; x < cols; ++x) {
                const auto eigen = computeHessianEigen2x2(p_rxx[x], p_rxy[x], p_ryy[x]);
                const float salience = std::abs(eigen.lambda1);

                // Polarity test: dark line (valleys) -> lambda1 > 0; bright line (ridges) -> lambda1 < 0
                const bool correct_polarity =
                    config_.extract_dark_lines ? (eigen.lambda1 > 0.0f) : (eigen.lambda1 < 0.0f);

                if (!correct_polarity || salience < config_.low_threshold) {
                    continue;
                }

                // Directional first and second derivatives along normal vector
                const float f_prime = p_rx[x] * eigen.nx + p_ry[x] * eigen.ny;
                const float f_dprime = p_rxx[x] * eigen.nx * eigen.nx + 2.0f * p_rxy[x] * eigen.nx * eigen.ny +
                                       p_ryy[x] * eigen.ny * eigen.ny;

                if (std::abs(f_dprime) < 1e-6f) continue;

                const float t = -f_prime / f_dprime;
                const float sub_x = t * eigen.nx;
                const float sub_y = t * eigen.ny;

                // Sub-pixel position offset inside current pixel unit square
                if (std::abs(sub_x) <= 0.5f && std::abs(sub_y) <= 0.5f && std::abs(t) <= 0.7f) {
                    const int idx = y * cols + x;
                    auto& cand = candidates[static_cast<size_t>(idx)];
                    cand.valid = true;
                    cand.px = static_cast<float>(x) + sub_x;
                    cand.py = static_cast<float>(y) + sub_y;
                    cand.salience = salience;
                    cand.nx = eigen.nx;
                    cand.ny = eigen.ny;
                    cand.tx = -eigen.ny; // Tangent
                    cand.ty = eigen.nx;
                    cand.intensity = p_val[x];
                    cand.confidence = std::min(1.0f, salience / std::max(0.01f, config_.high_threshold));
                }
            }
        }
    });

    // 5. Hysteresis Linking along Tangent Direction
    std::vector<uint8_t> visited(static_cast<size_t>(rows * cols), 0);
    int next_segment_id = 1;

    const int dx[8] = {1, 1, 0, -1, -1, -1, 0, 1};
    const int dy[8] = {0, 1, 1, 1, 0, -1, -1, -1};

    for (int y = 1; y < rows - 1; ++y) {
        for (int x = 1; x < cols - 1; ++x) {
            const int start_idx = y * cols + x;
            const auto& seed = candidates[static_cast<size_t>(start_idx)];

            if (!seed.valid || visited[static_cast<size_t>(start_idx)] || seed.salience < config_.high_threshold) {
                continue;
            }

            std::deque<Point2D> polyline;
            polyline.emplace_back(seed.px, seed.py, seed.intensity, seed.confidence);
            visited[static_cast<size_t>(start_idx)] = 1;

            // Trace forward along +tangent
            {
                int curr_x = x;
                int curr_y = y;
                float curr_tx = seed.tx;
                float curr_ty = seed.ty;

                while (true) {
                    int best_nx = -1;
                    int best_ny = -1;
                    float best_score = -1.0f;

                    for (int k = 0; k < 8; ++k) {
                        const int nx_k = curr_x + dx[k];
                        const int ny_k = curr_y + dy[k];
                        if (nx_k < 0 || nx_k >= cols || ny_k < 0 || ny_k >= rows) continue;

                        const int n_idx = ny_k * cols + nx_k;
                        if (visited[static_cast<size_t>(n_idx)]) continue;

                        const auto& cand = candidates[static_cast<size_t>(n_idx)];
                        if (!cand.valid) continue;

                        const float dir_x = static_cast<float>(dx[k]);
                        const float dir_y = static_cast<float>(dy[k]);
                        const float dist = std::sqrt(dir_x * dir_x + dir_y * dir_y);
                        const float dot = (dir_x * curr_tx + dir_y * curr_ty) / dist;

                        if (dot > 0.35f && dot > best_score) {
                            best_score = dot;
                            best_nx = nx_k;
                            best_ny = ny_k;
                        }
                    }

                    if (best_nx < 0) break;

                    const int chosen_idx = best_ny * cols + best_nx;
                    const auto& chosen = candidates[static_cast<size_t>(chosen_idx)];
                    visited[static_cast<size_t>(chosen_idx)] = 1;
                    polyline.emplace_back(chosen.px, chosen.py, chosen.intensity, chosen.confidence);

                    const float dot_t = chosen.tx * curr_tx + chosen.ty * curr_ty;
                    if (dot_t >= 0.0f) {
                        curr_tx = chosen.tx;
                        curr_ty = chosen.ty;
                    } else {
                        curr_tx = -chosen.tx;
                        curr_ty = -chosen.ty;
                    }
                    curr_x = best_nx;
                    curr_y = best_ny;
                }
            }

            // Trace backward along -tangent
            {
                int curr_x = x;
                int curr_y = y;
                float curr_tx = -seed.tx;
                float curr_ty = -seed.ty;

                while (true) {
                    int best_nx = -1;
                    int best_ny = -1;
                    float best_score = -1.0f;

                    for (int k = 0; k < 8; ++k) {
                        const int nx_k = curr_x + dx[k];
                        const int ny_k = curr_y + dy[k];
                        if (nx_k < 0 || nx_k >= cols || ny_k < 0 || ny_k >= rows) continue;

                        const int n_idx = ny_k * cols + nx_k;
                        if (visited[static_cast<size_t>(n_idx)]) continue;

                        const auto& cand = candidates[static_cast<size_t>(n_idx)];
                        if (!cand.valid) continue;

                        const float dir_x = static_cast<float>(dx[k]);
                        const float dir_y = static_cast<float>(dy[k]);
                        const float dist = std::sqrt(dir_x * dir_x + dir_y * dir_y);
                        const float dot = (dir_x * curr_tx + dir_y * curr_ty) / dist;

                        if (dot > 0.35f && dot > best_score) {
                            best_score = dot;
                            best_nx = nx_k;
                            best_ny = ny_k;
                        }
                    }

                    if (best_nx < 0) break;

                    const int chosen_idx = best_ny * cols + best_nx;
                    const auto& chosen = candidates[static_cast<size_t>(chosen_idx)];
                    visited[static_cast<size_t>(chosen_idx)] = 1;
                    polyline.emplace_front(chosen.px, chosen.py, chosen.intensity, chosen.confidence);

                    const float dot_t = chosen.tx * curr_tx + chosen.ty * curr_ty;
                    if (dot_t >= 0.0f) {
                        curr_tx = chosen.tx;
                        curr_ty = chosen.ty;
                    } else {
                        curr_tx = -chosen.tx;
                        curr_ty = -chosen.ty;
                    }
                    curr_x = best_nx;
                    curr_y = best_ny;
                }
            }

            float total_length = 0.0f;
            for (size_t i = 1; i < polyline.size(); ++i) {
                const float ddx = polyline[i].x - polyline[i - 1].x;
                const float ddy = polyline[i].y - polyline[i - 1].y;
                total_length += std::sqrt(ddx * ddx + ddy * ddy);
            }

            if (total_length >= config_.min_segment_length) {
                CurveSegment seg;
                seg.id = next_segment_id++;
                // Plain loop, not assign(): GCC 13 -Wnull-dereference false-positives
                // inside libstdc++'s std::copy for deque->vector copies at -O3.
                seg.points.reserve(polyline.size());
                for (const auto& pt : polyline) {
                    seg.points.push_back(pt);
                }
                seg.total_length = total_length;
                seg.average_curvature = 0.0f;
                seg.is_closed = false;

                for (const auto& pt : seg.points) {
                    const int ix = std::clamp(static_cast<int>(std::round(pt.x)), 0, cols - 1);
                    const int iy = std::clamp(static_cast<int>(std::round(pt.y)), 0, rows - 1);
                    result.debug_mask.at<uint8_t>(iy, ix) = 255;
                }

                result.segments.push_back(std::move(seg));
            }
        }
    }

    return result;
}

std::vector<Evidence> StegerRidgeExtractor::extractFromGraph(const RidgeGraph& graph) const {
    std::vector<Evidence> evidence_list;
    evidence_list.reserve(graph.segments.size());

    for (const auto& seg : graph.segments) {
        nlohmann::json geom_points = nlohmann::json::array();
        for (const auto& pt : seg.points) {
            geom_points.push_back(
                {{"x", pt.x}, {"y", pt.y}, {"intensity", pt.intensity}, {"confidence", pt.confidence}});
        }

        const nlohmann::json geometry = {{"segment_id", seg.id},
                                         {"point_count", seg.points.size()},
                                         {"total_length", seg.total_length},
                                         {"points", geom_points}};

        const nlohmann::json metadata = {{"sigma", config_.sigma}, {"extract_dark_lines", config_.extract_dark_lines}};

        evidence_list.emplace_back("ridge_seg_" + std::to_string(seg.id), "line_candidate", 0.95, geometry, metadata);
    }

    return evidence_list;
}

std::vector<Evidence> StegerRidgeExtractor::extract(const Frame& frame) {
    if (frame.empty()) return {};
    return extractFromGraph(extractRidgeGraph(frame.image));
}

} // namespace CurvEngine::ridge
