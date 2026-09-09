#include "curv/adapters/HandBoundaryDetector.hpp"
#include <algorithm>
#include <cmath>
#include <opencv2/imgproc.hpp>
#if __has_include(<opencv2/geometry/2d.hpp>)
#include <opencv2/geometry/2d.hpp>
#endif

namespace CurvEngine::adapters {

HandBoundaryResult HandBoundaryDetector::detectBoundary(const cv::Mat& image, float downscale_factor) {
    HandBoundaryResult res;
    if (image.empty()) {
        return res;
    }

    const int orig_w = image.cols;
    const int orig_h = image.rows;

    cv::Mat proc_img;
    const bool needs_resize = (downscale_factor > 0.0f && downscale_factor < 0.99f);
    if (needs_resize) {
        cv::resize(image, proc_img, cv::Size(), downscale_factor, downscale_factor, cv::INTER_AREA);
    } else {
        proc_img = image;
    }

    // 1. Convert to CIE Lab color space
    cv::Mat lab;
    cv::cvtColor(proc_img, lab, cv::COLOR_BGR2Lab);

    // Extract a* channel (green-red chroma, ~135-185 for skin vs ~128 for neutral)
    std::vector<cv::Mat> lab_channels;
    cv::split(lab, lab_channels);
    cv::Mat const& a_channel = lab_channels[1];

    // 2. Bilateral filter on a* channel preserves sharp silhouette boundaries while smoothing skin pores
    cv::Mat a_blur;
    cv::bilateralFilter(a_channel, a_blur, 9, 75.0, 75.0);

    // 3. Dynamic Otsu binarization on a* channel
    cv::Mat thresh_a;
    cv::threshold(a_blur, thresh_a, 0, 255, cv::THRESH_BINARY | cv::THRESH_OTSU);

    // 4. Find initial largest contour
    std::vector<std::vector<cv::Point>> raw_contours;
    cv::findContours(thresh_a, raw_contours, cv::RETR_EXTERNAL, cv::CHAIN_APPROX_NONE);

    if (raw_contours.empty()) {
        return res;
    }

    size_t best_idx = 0;
    double max_area = 0.0;
    for (size_t i = 0; i < raw_contours.size(); ++i) {
        const double a = cv::contourArea(raw_contours[i]);
        if (a > max_area) {
            max_area = a;
            best_idx = i;
        }
    }

    if (max_area < 50.0) {
        return res;
    }

    // 5. Render solid mask for largest contour
    cv::Mat solid_mask = cv::Mat::zeros(proc_img.size(), CV_8UC1);
    cv::drawContours(solid_mask, raw_contours, static_cast<int>(best_idx), cv::Scalar(255), cv::FILLED);

    // 6. Regularize Fingertip Caps: Bridge small concave nail notches
    const auto& c = raw_contours[best_idx];
    const int n = static_cast<int>(c.size());
    const int span = 20;
    const int max_y_lim = static_cast<int>(static_cast<float>(proc_img.rows) * 0.70f);

    cv::Mat bridged_mask = solid_mask.clone();

    if (n > 2 * span) {
        for (int i = 0; i < n; ++i) {
            const auto& p_prev = c[static_cast<size_t>((i - span + n) % n)];
            const auto& p_curr = c[static_cast<size_t>(i)];
            const auto& p_next = c[static_cast<size_t>((i + span) % n)];

            const float v1x = static_cast<float>(p_curr.x - p_prev.x);
            const float v1y = static_cast<float>(p_curr.y - p_prev.y);
            const float v2x = static_cast<float>(p_next.x - p_curr.x);
            const float v2y = static_cast<float>(p_next.y - p_curr.y);
            const float cross = v1x * v2y - v1y * v2x;

            const float chord =
                std::hypot(static_cast<float>(p_next.x - p_prev.x), static_cast<float>(p_next.y - p_prev.y));

            if (cross < -50.0f && chord > 10.0f && chord < 55.0f && p_curr.y < max_y_lim) {
                const cv::Point mid((p_prev.x + p_next.x) / 2, (p_prev.y + p_next.y) / 2);
                if (p_curr.y + 15 < proc_img.rows && solid_mask.at<uint8_t>(p_curr.y + 15, p_curr.x) == 255) {
                    if (mid.y - 20 >= 0 && solid_mask.at<uint8_t>(mid.y - 20, mid.x) == 0) {
                        const cv::Point tri_pts[3] = {p_prev, p_curr, p_next};
                        const cv::Point* tri_arr[1] = {tri_pts};
                        const int n_tri[1] = {3};
                        cv::fillPoly(solid_mask, tri_arr, n_tri, 1, cv::Scalar(255));
                    }
                }
            }
        }
    }

    // 7. Subtle Gaussian smoothing to anti-alias the silhouette contour without clipping finger clefts
    cv::Mat blur_mask;
    cv::GaussianBlur(solid_mask, blur_mask, cv::Size(7, 7), 2.0);
    cv::Mat smooth_thresh;
    cv::threshold(blur_mask, smooth_thresh, 127, 255, cv::THRESH_BINARY);

    std::vector<std::vector<cv::Point>> smooth_contours;
    cv::findContours(smooth_thresh, smooth_contours, cv::RETR_EXTERNAL, cv::CHAIN_APPROX_TC89_KCOS);

    if (smooth_contours.empty()) {
        smooth_contours = {raw_contours[best_idx]};
    }

    size_t smooth_best_idx = 0;
    double smooth_max_area = 0.0;
    for (size_t i = 0; i < smooth_contours.size(); ++i) {
        const double a = cv::contourArea(smooth_contours[i]);
        if (a > smooth_max_area) {
            smooth_max_area = a;
            smooth_best_idx = i;
        }
    }

    // 9. Rescale back to original dimensions if downscaled
    if (needs_resize) {
        cv::resize(smooth_thresh, res.mask, cv::Size(orig_w, orig_h), 0.0, 0.0, cv::INTER_NEAREST);

        const float inv_scale = 1.0f / downscale_factor;
        res.contour.reserve(smooth_contours[smooth_best_idx].size());
        for (const auto& pt : smooth_contours[smooth_best_idx]) {
            res.contour.emplace_back(static_cast<int>(std::round(static_cast<float>(pt.x) * inv_scale)),
                                     static_cast<int>(std::round(static_cast<float>(pt.y) * inv_scale)));
        }
        res.area = smooth_max_area * static_cast<double>(inv_scale * inv_scale);
    } else {
        res.mask = smooth_thresh;
        res.contour = smooth_contours[smooth_best_idx];
        res.area = smooth_max_area;
    }

    res.contours.push_back(res.contour);
    res.is_valid = true;
    return res;
}

void HandBoundaryDetector::filterGraphByMask(RidgeGraph& graph, const cv::Mat& mask, float min_inside_ratio) {
    if (mask.empty() || graph.segments.empty()) {
        return;
    }

    const int rows = mask.rows;
    const int cols = mask.cols;

    std::erase_if(graph.segments, [&](const auto& seg) {
        if (seg.points.empty()) return true;

        size_t inside_count = 0;
        for (const auto& pt : seg.points) {
            const int ix = static_cast<int>(std::round(pt.x));
            const int iy = static_cast<int>(std::round(pt.y));
            if (ix >= 0 && ix < cols && iy >= 0 && iy < rows) {
                if (mask.at<uint8_t>(iy, ix) > 0) {
                    ++inside_count;
                }
            }
        }

        const float ratio = static_cast<float>(inside_count) / static_cast<float>(seg.points.size());
        return ratio < min_inside_ratio;
    });
}

} // namespace CurvEngine::adapters
