#include "curv/domain/CreaseChainLinker.hpp"
#include <algorithm>
#include <cmath>
#include <limits>

namespace CurvEngine::domain {

namespace {

inline float dist2d(const Point2D& a, const Point2D& b) {
    return std::hypot(a.x - b.x, a.y - b.y);
}

inline Point2D normalizeVec(const Point2D& v) {
    const float len = std::hypot(v.x, v.y);
    if (len < 1e-5f) return Point2D(0.0f, 0.0f);
    return Point2D(v.x / len, v.y / len);
}

inline float dotVec(const Point2D& a, const Point2D& b) {
    return a.x * b.x + a.y * b.y;
}

} // anonymous namespace

std::vector<Point2D> CreaseChainLinker::fitCatmullRomSpline(const std::vector<Point2D>& control_points, float step) {
    if (control_points.empty()) {
        return {};
    }
    if (control_points.size() == 1) {
        return control_points;
    }

    // Filter duplicate consecutive control points
    std::vector<Point2D> pts;
    pts.reserve(control_points.size());
    pts.push_back(control_points.front());
    for (size_t i = 1; i < control_points.size(); ++i) {
        if (dist2d(control_points[i], pts.back()) > 0.5f) {
            pts.push_back(control_points[i]);
        }
    }

    if (pts.size() < 2) {
        return pts;
    }

    if (pts.size() == 2) {
        const float total_d = dist2d(pts[0], pts[1]);
        const int steps = std::max(2, static_cast<int>(std::ceil(total_d / std::max(0.5f, step))));
        std::vector<Point2D> result;
        result.reserve(static_cast<size_t>(steps + 1));
        for (int i = 0; i <= steps; ++i) {
            const float t = static_cast<float>(i) / static_cast<float>(steps);
            result.emplace_back(pts[0].x + t * (pts[1].x - pts[0].x), pts[0].y + t * (pts[1].y - pts[0].y));
        }
        return result;
    }

    const size_t n = pts.size();
    std::vector<Point2D> ext_pts;
    ext_pts.reserve(n + 2);

    // Phantom boundary endpoints
    ext_pts.emplace_back(2.0f * pts[0].x - pts[1].x, 2.0f * pts[0].y - pts[1].y);
    for (const auto& p : pts) {
        ext_pts.push_back(p);
    }
    ext_pts.emplace_back(2.0f * pts[n - 1].x - pts[n - 2].x, 2.0f * pts[n - 1].y - pts[n - 2].y);

    std::vector<Point2D> smooth;
    smooth.reserve(n * 8);

    for (size_t i = 0; i < n - 1; ++i) {
        const auto& p0 = ext_pts[i];
        const auto& p1 = ext_pts[i + 1];
        const auto& p2 = ext_pts[i + 2];
        const auto& p3 = ext_pts[i + 3];

        const float seg_len = dist2d(p1, p2);
        const int steps = std::max(1, static_cast<int>(std::ceil(seg_len / std::max(0.5f, step))));

        for (int k = 0; k < steps; ++k) {
            const float t = static_cast<float>(k) / static_cast<float>(steps);
            const float t2 = t * t;
            const float t3 = t2 * t;

            const float x =
                0.5f * ((2.0f * p1.x) + (-p0.x + p2.x) * t + (2.0f * p0.x - 5.0f * p1.x + 4.0f * p2.x - p3.x) * t2 +
                        (-p0.x + 3.0f * p1.x - 3.0f * p2.x + p3.x) * t3);
            const float y =
                0.5f * ((2.0f * p1.y) + (-p0.y + p2.y) * t + (2.0f * p0.y - 5.0f * p1.y + 4.0f * p2.y - p3.y) * t2 +
                        (-p0.y + 3.0f * p1.y - 3.0f * p2.y + p3.y) * t3);
            smooth.emplace_back(x, y);
        }
    }
    smooth.push_back(pts.back());
    return smooth;
}

std::vector<ChainedCreaseLine> CreaseChainLinker::linkSegments(const std::vector<CurveSegment>& segments,
                                                               const CreaseLinkOptions& options) {
    std::vector<ChainedCreaseLine> result;
    if (segments.empty()) {
        return result;
    }

    std::vector<const CurveSegment*> valid_segs;
    valid_segs.reserve(segments.size());
    for (const auto& s : segments) {
        if (s.points.size() >= 2) {
            valid_segs.push_back(&s);
        }
    }

    if (valid_segs.empty()) {
        return result;
    }

    // Sort by length descending to start chains with dominant crease seeds
    std::ranges::sort(valid_segs,
                      [](const CurveSegment* a, const CurveSegment* b) { return a->total_length > b->total_length; });

    std::vector<bool> used(valid_segs.size(), false);

    for (size_t i = 0; i < valid_segs.size(); ++i) {
        if (used[i]) continue;

        std::vector<Point2D> chain = valid_segs[i]->points;
        std::vector<int> chain_ids = {valid_segs[i]->id};
        float total_len = valid_segs[i]->total_length;
        used[i] = true;

        // Greedily extend forward from chain tail
        while (true) {
            size_t best_j = valid_segs.size();
            float best_score = std::numeric_limits<float>::max();
            bool reverse_candidate = false;

            const Point2D tail = chain.back();
            const Point2D tail_tangent = normalizeVec(
                Point2D(chain.back().x - chain[chain.size() - 2].x, chain.back().y - chain[chain.size() - 2].y));

            for (size_t j = 0; j < valid_segs.size(); ++j) {
                if (used[j]) continue;
                const auto& cand = *valid_segs[j];

                // Check candidate front
                const float d_front = dist2d(tail, cand.points.front());
                if (d_front <= options.max_gap_distance) {
                    const Point2D gap_v =
                        (d_front > 1e-2f)
                            ? normalizeVec(Point2D(cand.points.front().x - tail.x, cand.points.front().y - tail.y))
                            : tail_tangent;
                    const Point2D cand_head_tangent =
                        normalizeVec(Point2D(cand.points[1].x - cand.points[0].x, cand.points[1].y - cand.points[0].y));

                    const float dot1 = dotVec(tail_tangent, gap_v);
                    const float dot2 = dotVec(gap_v, cand_head_tangent);

                    if (dot1 >= options.min_collinearity && dot2 >= options.min_collinearity) {
                        const float score = d_front - 20.0f * ((dot1 + dot2) * 0.5f);
                        if (score < best_score) {
                            best_score = score;
                            best_j = j;
                            reverse_candidate = false;
                        }
                    }
                }

                // Check candidate back
                const float d_back = dist2d(tail, cand.points.back());
                if (d_back <= options.max_gap_distance) {
                    const Point2D gap_v =
                        (d_back > 1e-2f)
                            ? normalizeVec(Point2D(cand.points.back().x - tail.x, cand.points.back().y - tail.y))
                            : tail_tangent;
                    const size_t c_n = cand.points.size();
                    const Point2D cand_tail_tangent =
                        normalizeVec(Point2D(cand.points[c_n - 2].x - cand.points[c_n - 1].x,
                                             cand.points[c_n - 2].y - cand.points[c_n - 1].y));

                    const float dot1 = dotVec(tail_tangent, gap_v);
                    const float dot2 = dotVec(gap_v, cand_tail_tangent);

                    if (dot1 >= options.min_collinearity && dot2 >= options.min_collinearity) {
                        const float score = d_back - 20.0f * ((dot1 + dot2) * 0.5f);
                        if (score < best_score) {
                            best_score = score;
                            best_j = j;
                            reverse_candidate = true;
                        }
                    }
                }
            }

            if (best_j < valid_segs.size()) {
                used[best_j] = true;
                chain_ids.push_back(valid_segs[best_j]->id);
                total_len += valid_segs[best_j]->total_length;
                const auto& cand_pts = valid_segs[best_j]->points;
                if (!reverse_candidate) {
                    chain.insert(chain.end(), cand_pts.begin(), cand_pts.end());
                } else {
                    chain.insert(chain.end(), cand_pts.rbegin(), cand_pts.rend());
                }
            } else {
                break;
            }
        }

        // Fit Catmull-Rom spline
        auto smooth_points = fitCatmullRomSpline(chain, options.spline_step);
        ChainedCreaseLine line;
        line.constituent_segment_ids = chain_ids;
        line.points = std::move(smooth_points);
        line.total_length = total_len;
        line.is_valid = true;
        result.push_back(std::move(line));
    }

    return result;
}

std::unordered_map<std::string, ChainedCreaseLine>
CreaseChainLinker::extractMajorLines(const RidgeGraph& graph, bool is_left_hand, int patch_dim) {
    std::unordered_map<std::string, ChainedCreaseLine> major_lines;
    if (graph.segments.empty()) {
        return major_lines;
    }

    const float dim = static_cast<float>(patch_dim);

    std::vector<CurveSegment> heart_cands;
    std::vector<CurveSegment> head_cands;
    std::vector<CurveSegment> life_cands;
    std::vector<CurveSegment> fate_cands;

    for (const auto& s : graph.segments) {
        if (s.points.size() < 2) continue;

        float sum_x = 0.0f;
        float sum_y = 0.0f;
        for (const auto& pt : s.points) {
            sum_x += pt.x;
            sum_y += pt.y;
        }
        const float n = static_cast<float>(s.points.size());
        const float mean_x = sum_x / n;
        const float mean_y = sum_y / n;
        const float norm_x = mean_x / dim;
        const float norm_y = mean_y / dim;

        const float dx = std::abs(s.points.back().x - s.points.front().x);
        const float dy = std::abs(s.points.back().y - s.points.front().y);
        const float angle = std::atan2(dy, dx);

        // Heart Line: upper palm transverse crease
        if (norm_y >= 0.15f && norm_y <= 0.48f && angle < 0.85f && s.total_length >= 15.0f) {
            heart_cands.push_back(s);
        }
        // Head Line: mid palm transverse / diagonal crease
        if (norm_y >= 0.28f && norm_y <= 0.65f && angle < 0.85f && s.total_length >= 15.0f) {
            head_cands.push_back(s);
        }
        // Life Line: curving crease around thenar/Venus mount on radial side
        const bool radial_side = is_left_hand ? (norm_x > 0.35f) : (norm_x < 0.65f);
        if (radial_side && norm_y >= 0.28f && norm_y <= 0.80f && s.total_length >= 15.0f) {
            life_cands.push_back(s);
        }
        // Fate Line: central longitudinal vertical crease
        if (norm_x >= 0.30f && norm_x <= 0.70f && norm_y >= 0.30f && norm_y <= 0.85f && angle >= 0.65f &&
            s.total_length >= 15.0f) {
            fate_cands.push_back(s);
        }
    }

    CreaseLinkOptions opts;
    opts.max_gap_distance = 35.0f;
    opts.min_collinearity = 0.70f;
    opts.spline_step = 2.0f;

    auto chain_cands = [&](const std::string& name, const std::vector<CurveSegment>& cands) {
        auto chains = linkSegments(cands, opts);
        if (!chains.empty()) {
            std::ranges::sort(chains, [](const ChainedCreaseLine& a, const ChainedCreaseLine& b) {
                return a.total_length > b.total_length;
            });
            chains.front().name = name;
            major_lines[name] = chains.front();
        }
    };

    chain_cands("heart_line", heart_cands);
    chain_cands("head_line", head_cands);
    chain_cands("life_line", life_cands);
    chain_cands("fate_line", fate_cands);

    return major_lines;
}

} // namespace CurvEngine::domain
