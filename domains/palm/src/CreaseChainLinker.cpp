#include "curv/domain/CreaseChainLinker.hpp"
#include "curv/domain/HandAnatomyDecomposer.hpp"
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

    struct AttachHit {
        float score{std::numeric_limits<float>::max()};
        bool reverse{false};
        bool ok{false};
    };

    auto evaluateAttach = [&](const std::vector<Point2D>& chain, size_t j, bool extend_front) -> AttachHit {
        AttachHit hit;
        if (used[j] || chain.size() < 2) {
            return hit;
        }
        const auto& cand = *valid_segs[j];
        const Point2D tip = extend_front ? chain.front() : chain.back();
        const Point2D tip_tangent = extend_front
                                        ? normalizeVec(Point2D(chain[0].x - chain[1].x, chain[0].y - chain[1].y))
                                        : normalizeVec(Point2D(chain.back().x - chain[chain.size() - 2].x,
                                                               chain.back().y - chain[chain.size() - 2].y));

        const size_t c_n = cand.points.size();
        const Point2D head_t =
            normalizeVec(Point2D(cand.points[1].x - cand.points[0].x, cand.points[1].y - cand.points[0].y));
        const Point2D tail_t = normalizeVec(
            Point2D(cand.points[c_n - 2].x - cand.points[c_n - 1].x, cand.points[c_n - 2].y - cand.points[c_n - 1].y));

        auto consider = [&](const Point2D& cand_end, const Point2D& cand_tangent, bool reverse) {
            const float d = dist2d(tip, cand_end);
            if (d > options.max_gap_distance) {
                return;
            }
            const Point2D gap_v =
                (d > 1e-2f) ? normalizeVec(Point2D(cand_end.x - tip.x, cand_end.y - tip.y)) : tip_tangent;
            const float dot1 = dotVec(tip_tangent, gap_v);
            const float dot2 = dotVec(gap_v, cand_tangent);
            if (dot1 < options.min_collinearity || dot2 < options.min_collinearity) {
                return;
            }
            const float score = d - 20.0f * ((dot1 + dot2) * 0.5f);
            if (!hit.ok || score < hit.score) {
                hit.score = score;
                hit.reverse = reverse;
                hit.ok = true;
            }
        };

        // Candidate front at tip: reverse when prepending so growth continues outward
        consider(cand.points.front(), head_t, extend_front);
        // Candidate back at tip: reverse when appending
        consider(cand.points.back(), tail_t, !extend_front);
        return hit;
    };

    for (size_t i = 0; i < valid_segs.size(); ++i) {
        if (used[i]) continue;

        std::vector<Point2D> chain = valid_segs[i]->points;
        std::vector<int> chain_ids = {valid_segs[i]->id};
        float total_len = valid_segs[i]->total_length;
        used[i] = true;

        // Bidirectional greedy growth: mid-palm seeds must extend both ways or lines truncate.
        bool grew = true;
        while (grew) {
            grew = false;
            size_t best_j = valid_segs.size();
            float best_score = std::numeric_limits<float>::max();
            bool best_front = false;
            bool best_reverse = false;

            for (size_t j = 0; j < valid_segs.size(); ++j) {
                if (used[j]) continue;
                const AttachHit back_hit = evaluateAttach(chain, j, false);
                if (back_hit.ok && back_hit.score < best_score) {
                    best_score = back_hit.score;
                    best_j = j;
                    best_front = false;
                    best_reverse = back_hit.reverse;
                }
                const AttachHit front_hit = evaluateAttach(chain, j, true);
                if (front_hit.ok && front_hit.score < best_score) {
                    best_score = front_hit.score;
                    best_j = j;
                    best_front = true;
                    best_reverse = front_hit.reverse;
                }
            }

            if (best_j < valid_segs.size()) {
                used[best_j] = true;
                chain_ids.push_back(valid_segs[best_j]->id);
                total_len += valid_segs[best_j]->total_length;
                const auto& cand_pts = valid_segs[best_j]->points;
                std::vector<Point2D> ordered;
                ordered.reserve(cand_pts.size());
                if (best_reverse) {
                    ordered.assign(cand_pts.rbegin(), cand_pts.rend());
                } else {
                    ordered.assign(cand_pts.begin(), cand_pts.end());
                }
                if (best_front) {
                    chain.insert(chain.begin(), ordered.begin(), ordered.end());
                } else {
                    chain.insert(chain.end(), ordered.begin(), ordered.end());
                }
                grew = true;
            }
        }

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

size_t CreaseChainLinker::filterGraphByPalmDisc(RidgeGraph& graph, const std::vector<Point2D>& palm_disc,
                                                float min_inside_ratio) {
    if (palm_disc.size() < 3 || graph.segments.empty()) {
        return 0;
    }

    // Inflate disc slightly so thenar / Life-line fragments near the border survive.
    float cx = 0.0f;
    float cy = 0.0f;
    for (const auto& p : palm_disc) {
        cx += p.x;
        cy += p.y;
    }
    cx /= static_cast<float>(palm_disc.size());
    cy /= static_cast<float>(palm_disc.size());
    std::vector<Point2D> inflated;
    inflated.reserve(palm_disc.size());
    constexpr float kInflate = 1.12f;
    for (const auto& p : palm_disc) {
        inflated.emplace_back(cx + (p.x - cx) * kInflate, cy + (p.y - cy) * kInflate);
    }

    std::vector<CurveSegment> kept;
    kept.reserve(graph.segments.size());
    size_t removed = 0;

    for (auto& seg : graph.segments) {
        if (seg.points.size() < 2) {
            ++removed;
            continue;
        }
        size_t inside = 0;
        for (const auto& pt : seg.points) {
            if (HandAnatomyDecomposer::isInsidePalmDisc(pt, inflated)) {
                ++inside;
            }
        }
        const float ratio = static_cast<float>(inside) / static_cast<float>(seg.points.size());
        if (ratio >= min_inside_ratio) {
            kept.push_back(std::move(seg));
        } else {
            ++removed;
        }
    }

    graph.segments = std::move(kept);
    return removed;
}

namespace {

struct SegStats {
    const CurveSegment* seg{nullptr};
    float mean_x{0.0f};
    float mean_y{0.0f};
    float dx{0.0f};
    float dy{0.0f};
    float angle{0.0f}; // atan2(|sin|,|cos|) of PCA axis: 0=horizontal, ~1.57=vertical
    float span_x{0.0f};
    float span_y{0.0f};
    float min_y{0.0f};
    float max_y{0.0f};
};

SegStats makeStats(const CurveSegment& s) {
    SegStats st;
    st.seg = &s;
    float sum_x = 0.0f;
    float sum_y = 0.0f;
    float min_x = s.points.front().x;
    float max_x = s.points.front().x;
    st.min_y = s.points.front().y;
    st.max_y = s.points.front().y;
    for (const auto& pt : s.points) {
        sum_x += pt.x;
        sum_y += pt.y;
        min_x = std::min(min_x, pt.x);
        max_x = std::max(max_x, pt.x);
        st.min_y = std::min(st.min_y, pt.y);
        st.max_y = std::max(st.max_y, pt.y);
    }
    const float n = static_cast<float>(s.points.size());
    st.mean_x = sum_x / n;
    st.mean_y = sum_y / n;
    st.span_x = max_x - min_x;
    st.span_y = st.max_y - st.min_y;
    st.dx = std::abs(s.points.back().x - s.points.front().x);
    st.dy = std::abs(s.points.back().y - s.points.front().y);

    // PCA principal axis — curved life-line fragments are mis-scored by endpoint bbox slope.
    float cxx = 0.0f;
    float cyy = 0.0f;
    float cxy = 0.0f;
    for (const auto& pt : s.points) {
        const float dx = pt.x - st.mean_x;
        const float dy = pt.y - st.mean_y;
        cxx += dx * dx;
        cyy += dy * dy;
        cxy += dx * dy;
    }
    const float theta = 0.5f * std::atan2(2.0f * cxy, cxx - cyy + 1e-6f);
    const float horiz = std::abs(std::cos(theta));
    const float vert = std::abs(std::sin(theta));
    st.angle = std::atan2(vert, horiz + 1e-6f);
    return st;
}

struct PalmAxis {
    float mcp_y{0.0f};
    float wrist_y{0.0f};
    float mid_x{0.0f};
    float index_x{0.0f};
    float pinky_x{0.0f};
    float thumb_x{0.0f};
    float palm_height{1.0f};
    bool valid{false};
};

PalmAxis buildAxis(const MajorLineLandmarks& priors, int patch_dim) {
    PalmAxis axis;
    if (!priors.valid) {
        return axis;
    }
    const auto& lms = priors.landmarks;
    const float sx = lms.is_normalized ? static_cast<float>(patch_dim) : 1.0f;
    const float sy = lms.is_normalized ? static_cast<float>(patch_dim) : 1.0f;

    auto pt = [&](int idx) -> Point2D {
        return Point2D(lms.points[static_cast<size_t>(idx)].x * sx, lms.points[static_cast<size_t>(idx)].y * sy);
    };

    const Point2D wrist = pt(0);
    const Point2D index_mcp = pt(5);
    const Point2D middle_mcp = pt(9);
    const Point2D ring_mcp = pt(13);
    const Point2D pinky_mcp = pt(17);
    const Point2D thumb_mcp = pt(2);

    axis.mcp_y = 0.25f * (index_mcp.y + middle_mcp.y + ring_mcp.y + pinky_mcp.y);
    axis.wrist_y = wrist.y;
    axis.mid_x = middle_mcp.x;
    axis.index_x = index_mcp.x;
    axis.pinky_x = pinky_mcp.x;
    axis.thumb_x = thumb_mcp.x;
    axis.palm_height = std::max(40.0f, std::abs(axis.wrist_y - axis.mcp_y));
    axis.valid = true;
    return axis;
}

float palmDepth(float y, const PalmAxis& axis) {
    if (!axis.valid || axis.palm_height < 1.0f) {
        return 0.5f;
    }
    if (axis.wrist_y >= axis.mcp_y) {
        return std::clamp((y - axis.mcp_y) / axis.palm_height, 0.0f, 1.5f);
    }
    return std::clamp((axis.mcp_y - y) / axis.palm_height, 0.0f, 1.5f);
}

float scoreHeart(const SegStats& st, const PalmAxis& axis, float dim, bool /*is_left*/) {
    if (st.seg->total_length < 18.0f || st.angle > 0.75f) {
        return -1.0f;
    }
    if (axis.valid) {
        const float d = palmDepth(st.mean_y, axis);
        // Classical heart sits below MCP knuckles, not on the basal finger crease.
        // Keep a gap vs Head band to reduce exclusive-assignment theft.
        if (d < 0.06f || d > 0.30f) {
            return -1.0f;
        }
        const float palm_width = std::max(40.0f, std::abs(axis.pinky_x - axis.index_x));
        const float depth_score = 1.0f - std::abs(d - 0.16f) / 0.18f;
        const float horiz = 1.0f - std::min(1.0f, st.angle / 0.75f);
        const float len = std::min(1.0f, st.seg->total_length / 100.0f);
        const float span = std::min(1.0f, st.span_x / (0.55f * palm_width));
        return 0.35f * depth_score + 0.25f * horiz + 0.20f * len + 0.20f * span;
    }
    const float norm_y = st.mean_y / dim;
    if (norm_y < 0.14f || norm_y > 0.42f) {
        return -1.0f;
    }
    return (1.0f - std::abs(norm_y - 0.28f) / 0.24f) * 0.5f + (1.0f - st.angle / 0.75f) * 0.5f;
}

float scoreHead(const SegStats& st, const PalmAxis& axis, float dim, bool /*is_left*/) {
    if (st.seg->total_length < 14.0f || st.angle > 0.92f) {
        return -1.0f;
    }
    if (axis.valid) {
        const float d = palmDepth(st.mean_y, axis);
        if (d < 0.28f || d > 0.65f) {
            return -1.0f;
        }
        const float palm_width = std::max(40.0f, std::abs(axis.pinky_x - axis.index_x));
        const float depth_score = 1.0f - std::abs(d - 0.44f) / 0.22f;
        const float horiz = 1.0f - std::min(1.0f, st.angle / 0.90f);
        const float len = std::min(1.0f, st.seg->total_length / 100.0f);
        const float span = std::min(1.0f, st.span_x / (0.55f * palm_width));
        return 0.35f * depth_score + 0.25f * horiz + 0.20f * len + 0.20f * span;
    }
    const float norm_y = st.mean_y / dim;
    if (norm_y < 0.36f || norm_y > 0.68f) {
        return -1.0f;
    }
    return (1.0f - std::abs(norm_y - 0.50f) / 0.28f) * 0.5f + (1.0f - st.angle / 0.90f) * 0.5f;
}

float scoreLife(const SegStats& st, const PalmAxis& axis, float dim, bool is_left) {
    if (st.seg->total_length < 15.0f) {
        return -1.0f;
    }
    if (axis.valid) {
        const float d = palmDepth(st.mean_y, axis);
        if (d < 0.05f || d > 0.98f) {
            return -1.0f;
        }
        const float radial_center = 0.55f * axis.thumb_x + 0.45f * axis.index_x;
        const float palm_width = std::max(40.0f, std::abs(axis.pinky_x - axis.index_x));
        const float radial_dist = std::abs(st.mean_x - radial_center) / palm_width;
        if (radial_dist > 0.65f) {
            return -1.0f;
        }
        // Life is curved around thenar — reject flat transverse fragments near MCP row.
        if (st.angle < 0.20f && d < 0.35f) {
            return -1.0f;
        }
        // Prefer fragments that cover depth (arc down toward wrist), not just a short radial stub.
        const float d0 = palmDepth(st.min_y, axis);
        const float d1 = palmDepth(st.max_y, axis);
        const float depth_cover = std::min(1.0f, std::abs(d1 - d0) / 0.35f);
        const float curve_bonus = std::clamp(st.angle / 0.85f, 0.2f, 1.0f);
        const float len = std::min(1.0f, st.seg->total_length / 90.0f);
        return 0.30f * (1.0f - radial_dist) + 0.25f * curve_bonus + 0.25f * depth_cover + 0.20f * len;
    }
    const float norm_x = st.mean_x / dim;
    const float norm_y = st.mean_y / dim;
    const bool radial_side = is_left ? (norm_x > 0.30f) : (norm_x < 0.70f);
    if (!radial_side || norm_y < 0.25f || norm_y > 0.90f) {
        return -1.0f;
    }
    return 0.6f;
}

float scoreFate(const SegStats& st, const PalmAxis& axis, float dim, bool /*is_left*/) {
    if (st.seg->total_length < 20.0f || st.angle < 0.70f) {
        return -1.0f;
    }
    if (axis.valid) {
        const float d = palmDepth(st.mean_y, axis);
        if (d < 0.20f || d > 0.95f) {
            return -1.0f;
        }
        const float palm_width = std::max(40.0f, std::abs(axis.pinky_x - axis.index_x));
        const float center_dist = std::abs(st.mean_x - axis.mid_x) / palm_width;
        if (center_dist > 0.35f) {
            return -1.0f;
        }
        const float vert = std::min(1.0f, st.angle / 1.20f);
        const float len = std::min(1.0f, st.seg->total_length / 90.0f);
        return 0.45f * (1.0f - center_dist) + 0.35f * vert + 0.20f * len;
    }
    const float norm_x = st.mean_x / dim;
    const float norm_y = st.mean_y / dim;
    if (norm_x < 0.30f || norm_x > 0.70f || norm_y < 0.30f || norm_y > 0.90f) {
        return -1.0f;
    }
    return 0.5f * (st.angle / 1.20f);
}

} // anonymous namespace

std::unordered_map<std::string, ChainedCreaseLine>
CreaseChainLinker::extractMajorLines(const RidgeGraph& graph, bool is_left_hand, int patch_dim,
                                     const MajorLineLandmarks& priors, const CreaseLinkOptions& link_options) {
    std::unordered_map<std::string, ChainedCreaseLine> major_lines;
    if (graph.segments.empty()) {
        return major_lines;
    }

    const float dim = static_cast<float>(patch_dim);
    const PalmAxis axis = buildAxis(priors, patch_dim);

    struct Cand {
        SegStats stats;
        float heart{-1.0f};
        float head{-1.0f};
        float life{-1.0f};
        float fate{-1.0f};
        int assigned{-1};
    };

    std::vector<Cand> cands;
    cands.reserve(graph.segments.size());
    for (const auto& s : graph.segments) {
        if (s.points.size() < 2) continue;
        Cand c;
        c.stats = makeStats(s);
        c.heart = scoreHeart(c.stats, axis, dim, is_left_hand);
        c.head = scoreHead(c.stats, axis, dim, is_left_hand);
        c.life = scoreLife(c.stats, axis, dim, is_left_hand);
        c.fate = scoreFate(c.stats, axis, dim, is_left_hand);
        if (c.heart > 0.0f || c.head > 0.0f || c.life > 0.0f || c.fate > 0.0f) {
            cands.push_back(c);
        }
    }

    for (auto& c : cands) {
        float best = -1.0f;
        int best_i = -1;
        const float scores[4] = {c.heart, c.head, c.life, c.fate};
        for (int i = 0; i < 4; ++i) {
            if (scores[i] > best) {
                best = scores[i];
                best_i = i;
            }
        }
        if (best_i >= 0 && (best_i == 0 || best_i == 1) && c.heart > 0.0f && c.head > 0.0f &&
            std::abs(c.heart - c.head) < 0.08f) {
            best_i = (axis.valid && palmDepth(c.stats.mean_y, axis) < 0.28f) ? 0 : 1;
        }
        c.assigned = (best > 0.15f) ? best_i : -1;
    }

    CreaseLinkOptions opts = link_options;
    // Robust: 35->60 for broken head/life (user head gap ~30px).
    if (opts.max_gap_distance < 1.0f) {
        opts.max_gap_distance = 60.0F;
    }
    // Robust: 0.70->0.60 to allow life arc.
    if (opts.min_collinearity < 0.1f) {
        opts.min_collinearity = 0.60F;
    }
    if (opts.spline_step < 0.1f) {
        opts.spline_step = 2.0F;
    }

    auto chainGeometryScore = [&](const ChainedCreaseLine& chain, int label, float mean_seg_score) -> float {
        if (chain.points.size() < 2) {
            return -1.0f;
        }
        float min_x = chain.points.front().x;
        float max_x = chain.points.front().x;
        float min_y = chain.points.front().y;
        float max_y = chain.points.front().y;
        for (const auto& pt : chain.points) {
            min_x = std::min(min_x, pt.x);
            max_x = std::max(max_x, pt.x);
            min_y = std::min(min_y, pt.y);
            max_y = std::max(max_y, pt.y);
        }
        const float span_x = max_x - min_x;
        const float span_y = max_y - min_y;
        const float palm_width =
            axis.valid ? std::max(40.0f, std::abs(axis.pinky_x - axis.index_x)) : static_cast<float>(patch_dim) * 0.45f;
        const float len_n = std::min(1.0f, chain.total_length / 160.0f);
        if (label == 0 || label == 1) {
            // Heart/Head: prefer full transverse span, not a long but short-span scribble.
            const float span_n = std::min(1.0f, span_x / (0.55f * palm_width));
            return 0.50f * span_n + 0.30f * len_n + 0.20f * mean_seg_score;
        }
        if (label == 2) {
            // Life: prefer thenar depth coverage + radial mean.
            float depth_cover = std::min(1.0f, span_y / (0.40f * (axis.valid ? axis.palm_height : dim)));
            float mean_x = 0.5f * (min_x + max_x);
            float radial = 0.5f;
            if (axis.valid) {
                const float radial_center = 0.55f * axis.thumb_x + 0.45f * axis.index_x;
                radial = 1.0f - std::min(1.0f, std::abs(mean_x - radial_center) / palm_width);
                depth_cover = std::min(1.0f, std::abs(palmDepth(max_y, axis) - palmDepth(min_y, axis)) / 0.40f);
            }
            return 0.40f * depth_cover + 0.35f * radial + 0.25f * len_n;
        }
        // Fate: prefer vertical center column length.
        const float vert_n = std::min(1.0f, span_y / (0.35f * (axis.valid ? axis.palm_height : dim)));
        return 0.55f * vert_n + 0.25f * len_n + 0.20f * mean_seg_score;
    };

    auto chain_for = [&](const std::string& name, int label) {
        std::vector<CurveSegment> segs;
        float score_sum = 0.0f;
        int score_n = 0;
        for (const auto& c : cands) {
            if (c.assigned != label || c.stats.seg == nullptr) continue;
            segs.push_back(*c.stats.seg);
            const float sc = (label == 0) ? c.heart : (label == 1) ? c.head : (label == 2) ? c.life : c.fate;
            score_sum += sc;
            ++score_n;
        }
        auto chains = CreaseChainLinker::linkSegments(segs, opts);
        if (chains.empty()) {
            return;
        }
        const float mean_seg_score = score_n > 0 ? score_sum / static_cast<float>(score_n) : 0.5f;
        std::ranges::sort(chains, [&](const ChainedCreaseLine& a, const ChainedCreaseLine& b) {
            return chainGeometryScore(a, label, mean_seg_score) > chainGeometryScore(b, label, mean_seg_score);
        });
        if (label == 3 && (chains.front().total_length < 40.0f || mean_seg_score < 0.35f)) {
            return;
        }
        if (chains.front().total_length < 15.0f) {
            return;
        }
        // Heart/Head need meaningful transverse span; reject stub chains.
        if ((label == 0 || label == 1) && chains.front().points.size() >= 2) {
            float min_x = chains.front().points.front().x;
            float max_x = min_x;
            for (const auto& pt : chains.front().points) {
                min_x = std::min(min_x, pt.x);
                max_x = std::max(max_x, pt.x);
            }
            const float palm_width = axis.valid ? std::max(40.0f, std::abs(axis.pinky_x - axis.index_x)) : dim * 0.45f;
            if ((max_x - min_x) < 0.22f * palm_width && chains.front().total_length < 60.0f) {
                return;
            }
        }
        chains.front().name = name;
        chains.front().confidence = std::clamp(mean_seg_score, 0.0f, 0.98f);
        major_lines[name] = chains.front();
    };

    chain_for("heart_line", 0);
    chain_for("head_line", 1);
    chain_for("life_line", 2);
    chain_for("fate_line", 3);

    return major_lines;
}

} // namespace CurvEngine::domain
