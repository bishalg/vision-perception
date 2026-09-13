#include "curv/domain/FateLineAnalyzer.hpp"

#include <algorithm>
#include <cmath>

#include "curv/domain/CreaseGeometry.hpp"
#include "curv/pattern/MicroPatternDetector.hpp"

namespace CurvEngine::domain {

namespace {

constexpr float kPresentMinLength = 40.0F;
constexpr float kPresentMinConf = 0.55F;
constexpr float kShortRatio = 0.40F;
constexpr float kLongRatio = 0.70F;
constexpr float kMergeDistPx = 6.0F;
constexpr float kLifeJointDistPx = 10.0F;

float meanY(const std::vector<Point2D>& pts) {
    if (pts.empty()) {
        return 0.0F;
    }
    float sum = 0.0F;
    for (const auto& p : pts) {
        sum += p.y;
    }
    return sum / static_cast<float>(pts.size());
}

float overlapY(const std::vector<Point2D>& a, const std::vector<Point2D>& b) {
    if (a.empty() || b.empty()) {
        return 0.0F;
    }
    float a_min = a.front().y;
    float a_max = a.front().y;
    for (const auto& p : a) {
        a_min = std::min(a_min, p.y);
        a_max = std::max(a_max, p.y);
    }
    float b_min = b.front().y;
    float b_max = b.front().y;
    for (const auto& p : b) {
        b_min = std::min(b_min, p.y);
        b_max = std::max(b_max, p.y);
    }
    return std::max(0.0F, std::min(a_max, b_max) - std::max(a_min, b_min));
}

bool endNearTop(const std::vector<Point2D>& pts, int patch_dim) {
    if (pts.empty() || patch_dim <= 0) {
        return false;
    }
    float min_y = pts.front().y;
    for (const auto& p : pts) {
        min_y = std::min(min_y, p.y);
    }
    return min_y < 0.25F * static_cast<float>(patch_dim);
}

} // namespace

nlohmann::json FateReport::toJson() const {
    return {
        {"present", present},
        {"no_fate", no_fate},
        {"origin", origin},
        {"joined_life_start", joined_life_start},
        {"is_short", is_short},
        {"is_long", is_long},
        {"broken", broken},
        {"gap_count", gap_count},
        {"double_line", double_line},
        {"y_shaped", y_shaped},
        {"forked_up", forked_up},
        {"branches_up", branches_up},
        {"toward_mars_negative", toward_mars_negative},
        {"parallel_life", parallel_life},
        {"stops_at_heart", stops_at_heart},
        {"stops_at_head", stops_at_head},
        {"merged_head", merged_head},
        {"cross_sun", cross_sun},
        {"cross_life", cross_life},
        {"has_island_fish", has_island_fish},
        {"has_triangle", has_triangle},
        {"has_cross", has_cross},
        {"has_square", has_square},
        {"bad_saturn_mount", bad_saturn_mount},
        {"bad_mercury_mount", bad_mercury_mount},
        {"age_at_break", age_at_break},
        {"age_at_end", age_at_end},
    };
}

float FateLineAnalyzer::ageAtY(float y_norm, float head_y_norm, float heart_y_norm) {
    const float y = std::clamp(y_norm, 0.0F, 1.0F);
    // Fallback when anchors missing: linear 20..70 top->bottom inverted
    // (fate runs wrist->saturn, so smaller y is older).
    if (head_y_norm <= 0.0F || heart_y_norm <= 0.0F || head_y_norm <= heart_y_norm) {
        return std::clamp(70.0F - y * 50.0F, 15.0F, 80.0F);
    }
    if (y >= head_y_norm) {
        // Below head intersection: 20..35
        const float t = (head_y_norm > 1e-6F) ? (y - head_y_norm) / (1.0F - head_y_norm) : 0.0F;
        return std::clamp(35.0F + t * 15.0F, 15.0F, 80.0F);
    }
    if (y >= heart_y_norm) {
        // Between head (35) and heart (53)
        const float span = head_y_norm - heart_y_norm;
        const float t = (span > 1e-6F) ? (head_y_norm - y) / span : 0.0F;
        return std::clamp(35.0F + t * 18.0F, 15.0F, 80.0F);
    }
    // Above heart: 53..70
    const float t = (heart_y_norm > 1e-6F) ? (heart_y_norm - y) / heart_y_norm : 0.0F;
    return std::clamp(53.0F + t * 17.0F, 15.0F, 80.0F);
}

bool FateLineAnalyzer::stopsAt(const std::vector<Point2D>& fate_pts, const std::vector<Point2D>& other_pts) {
    if (fate_pts.empty() || other_pts.empty()) {
        return false;
    }
    if (CreaseGeometry::countIntersections(fate_pts, other_pts) > 0) {
        return true;
    }
    // Termination is a T-junction (endpoint touches line), which strict segment
    // intersection excludes for axis-aligned synthetic data and real near-touches.
    // Fall back to point-to-segment proximity (<3px in 512 space).
    for (const auto& p : fate_pts) {
        for (size_t i = 1; i < other_pts.size(); ++i) {
            const auto& a = other_pts[i - 1];
            const auto& b = other_pts[i];
            const float abx = b.x - a.x;
            const float aby = b.y - a.y;
            const float len2 = abx * abx + aby * aby;
            float t = 0.0F;
            if (len2 > 1e-6F) {
                t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len2;
                t = std::clamp(t, 0.0F, 1.0F);
            }
            const float dx = p.x - (a.x + t * abx);
            const float dy = p.y - (a.y + t * aby);
            if (std::hypot(dx, dy) < 3.0F) {
                return true;
            }
        }
    }
    return false;
}

bool FateLineAnalyzer::isParallelPair(const std::vector<Point2D>& a, const std::vector<Point2D>& b, float min_dist,
                                      float max_dist) {
    if (a.size() < 2 || b.size() < 2) {
        return false;
    }
    const float min_d = CreaseGeometry::computeMinDistance(a, b);
    if (min_d < min_dist || min_d > max_dist) {
        return false;
    }
    const float overlap = overlapY(a, b);
    const float len_a = CreaseGeometry::computeArcLength(a);
    const float len_b = CreaseGeometry::computeArcLength(b);
    const float min_len = std::min(len_a, len_b);
    if (min_len < 1e-3F) {
        return false;
    }
    return overlap > 0.5F * min_len;
}

FateReport FateLineAnalyzer::analyze(const ChainedCreaseLine& fate, const ChainedCreaseLine* heart,
                                     const ChainedCreaseLine* head, const ChainedCreaseLine* life,
                                     const RidgeGraph& graph, int patch_dim) {
    FateReport rep;
    const float dim = static_cast<float>(std::max(1, patch_dim));

    if (!fate.is_valid || fate.points.size() < 2 || fate.total_length < kPresentMinLength ||
        fate.confidence < kPresentMinConf) {
        rep.present = false;
        rep.no_fate = true;
        return rep;
    }
    rep.present = true;
    rep.no_fate = false;

    const int gaps =
        (fate.constituent_segment_ids.size() > 1) ? static_cast<int>(fate.constituent_segment_ids.size() - 1) : 0;
    rep.gap_count = gaps;
    rep.broken = gaps >= 1;

    const float arc = CreaseGeometry::computeArcLength(fate.points);
    rep.is_short = arc < kShortRatio * dim;
    rep.is_long = arc > kLongRatio * dim;

    const int forks = CreaseGeometry::countForks(fate.constituent_segment_ids, graph.junctions);
    rep.y_shaped = (forks >= 1) && endNearTop(fate.points, patch_dim);
    rep.forked_up = forks >= 2 && endNearTop(fate.points, patch_dim);
    rep.branches_up = rep.y_shaped || rep.forked_up;

    // Origin: life-joint takes precedence, then moon zone (hypothenar ulnar).
    rep.joined_life_start = false;
    if (life != nullptr && life->is_valid && !life->points.empty() && !fate.points.empty()) {
        const float d = CreaseGeometry::computeMinDistance({fate.points.front()}, life->points);
        rep.joined_life_start = d < kLifeJointDistPx;
    }
    if (rep.joined_life_start) {
        rep.origin = "life_joint";
    } else if (!fate.points.empty()) {
        const auto& s = fate.points.front();
        const float nx = s.x / dim;
        const float ny = s.y / dim;
        if (nx < 0.35F && ny > 0.65F) {
            rep.origin = "moon";
        } else if (head != nullptr && head->is_valid && !head->points.empty()) {
            const float d = CreaseGeometry::computeMinDistance({s}, head->points);
            rep.origin = (d < kLifeJointDistPx) ? "head_origin" : "unknown";
        }
    }

    // Termination / interaction via strict segment intersection.
    rep.stops_at_heart = (heart != nullptr && heart->is_valid) ? stopsAt(fate.points, heart->points) : false;
    rep.stops_at_head = (head != nullptr && head->is_valid) ? stopsAt(fate.points, head->points) : false;
    rep.cross_life = (life != nullptr && life->is_valid) ? stopsAt(fate.points, life->points) : false;
    rep.cross_sun = false; // sun/apollo line not yet chained; future P1

    // Merged head: close + overlapping in Y (collinear overlap proxy).
    rep.merged_head = false;
    if (head != nullptr && head->is_valid && !fate.points.empty() && !head->points.empty()) {
        const float d = CreaseGeometry::computeMinDistance(fate.points, head->points);
        rep.merged_head = (d < kMergeDistPx) && (overlapY(fate.points, head->points) > 30.0F);
    }

    // Course: parallel life + mars-negative branch (radial side proxy).
    rep.parallel_life = false;
    if (life != nullptr && life->is_valid) {
        rep.parallel_life = isParallelPair(fate.points, life->points);
    }
    rep.toward_mars_negative = false;
    if (!fate.points.empty()) {
        const auto& end = fate.points.back();
        const float nx = end.x / dim;
        const float ny = end.y / dim;
        // Mars-negative: radial-mid band; blog branch pointing there is a loss flag.
        rep.toward_mars_negative = (nx < 0.25F && ny > 0.40F && ny < 0.55F);
    }

    // Double line: second long vertical candidate parallel to fate in raw graph.
    rep.double_line = false;
    for (const auto& seg : graph.segments) {
        if (seg.points.size() < 2 || seg.total_length < kPresentMinLength) {
            continue;
        }
        bool is_constituent = false;
        for (int id : fate.constituent_segment_ids) {
            if (id == seg.id) {
                is_constituent = true;
                break;
            }
        }
        if (is_constituent) {
            continue;
        }
        std::vector<Point2D> pts;
        pts.reserve(seg.points.size());
        for (const auto& p : seg.points) {
            pts.push_back(Point2D{p.x, p.y});
        }
        if (isParallelPair(fate.points, pts)) {
            rep.double_line = true;
            break;
        }
    }

    // P2 shape detection using both dedicated MicroPatternDetector and fallback heuristics.
    auto detected_patterns = pattern::MicroPatternDetector::detectAll(graph);
    bool found_island_fish = false;
    bool found_triangle = false;
    bool found_cross = false;
    bool found_square = false;

    for (const auto& pat : detected_patterns) {
        if (pat.type == pattern::PatternType::Fish || pat.type == pattern::PatternType::Island)
            found_island_fish = true;
        if (pat.type == pattern::PatternType::Triangle) found_triangle = true;
        if (pat.type == pattern::PatternType::Cross) found_cross = true;
        if (pat.type == pattern::PatternType::Square) found_square = true;
    }

    rep.has_island_fish = found_island_fish || (rep.broken && (forks >= 1));
    rep.has_triangle =
        found_triangle || (CreaseGeometry::countIntersections(fate.points, fate.points) >= 1 && forks >= 2);
    rep.has_cross = found_cross || (forks >= 1 && rep.gap_count >= 1 && !rep.has_island_fish);
    rep.has_square = found_square ||
                     (rep.broken && !rep.has_island_fish &&
                      CreaseGeometry::computeTortuosity(arc, CreaseGeometry::computeChordLength(fate.points)) < 1.1F);

    // Age anchors from actual heart/head intersections when available.
    float head_y = 0.0F;
    float heart_y = 0.0F;
    if (head != nullptr && head->is_valid && !head->points.empty()) {
        head_y = meanY(head->points) / dim;
    }
    if (heart != nullptr && heart->is_valid && !heart->points.empty()) {
        heart_y = meanY(heart->points) / dim;
    }
    const float end_y = fate.points.empty() ? 0.5F : (fate.points.back().y / dim);
    // Break age uses mid-chain gap point when broken, else end.
    float break_y = end_y;
    if (rep.broken && fate.points.size() >= 3) {
        break_y = fate.points[fate.points.size() / 2].y / dim;
    }
    rep.age_at_end = ageAtY(end_y, head_y, heart_y);
    rep.age_at_break = rep.broken ? ageAtY(break_y, head_y, heart_y) : 0.0F;

    return rep;
}

} // namespace CurvEngine::domain
