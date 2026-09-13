#include "curv/domain/LifeLineAnalyzer.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

#include "curv/domain/CreaseGeometry.hpp"
#include "curv/domain/LineLengthThresholds.hpp"

namespace CurvEngine::domain {

namespace {

constexpr float kPresentMinLength = 40.0F;
constexpr float kPresentMinConf = 0.55F;
constexpr float kHeadJointDistPx = 10.0F;
constexpr float kMaxLifeAge = 90.0F;
constexpr float kHeadAnchorAge = 35.0F; // PLACEHOLDER, mirrors fate convention; see header.

// Arc fraction [0,1] of the point on `line` nearest to `query`.
float nearestArcFraction(const std::vector<Point2D>& line, const Point2D& query) {
    if (line.empty()) {
        return -1.0F;
    }
    size_t best = 0;
    float best_d2 = std::numeric_limits<float>::max();
    for (size_t i = 0; i < line.size(); ++i) {
        const float dx = line[i].x - query.x;
        const float dy = line[i].y - query.y;
        const float d2 = dx * dx + dy * dy;
        if (d2 < best_d2) {
            best_d2 = d2;
            best = i;
        }
    }
    if (line.size() < 2) {
        return 0.0F;
    }
    return static_cast<float>(best) / static_cast<float>(line.size() - 1);
}

} // namespace

nlohmann::json LifeReport::toJson() const {
    return {
        {"present", present},           {"is_short", is_short},
        {"is_long", is_long},           {"broken", broken},
        {"gap_count", gap_count},       {"age_at_end", age_at_end},
        {"age_at_break", age_at_break}, {"age_calibrated", age_calibrated},
    };
}

float LifeLineAnalyzer::ageAtLifeArc(float t, float head_t) {
    const float tc = std::clamp(t, 0.0F, 1.0F);
    if (head_t < 0.0F || head_t >= 1.0F) {
        return tc * kMaxLifeAge;
    }
    const float hc = std::clamp(head_t, 0.0F, 1.0F);
    if (tc <= hc) {
        const float denom = (hc > 1e-6F) ? hc : 1.0F;
        return kHeadAnchorAge * (tc / denom);
    }
    const float denom = (1.0F - hc > 1e-6F) ? (1.0F - hc) : 1.0F;
    return kHeadAnchorAge + (kMaxLifeAge - kHeadAnchorAge) * ((tc - hc) / denom);
}

LifeReport LifeLineAnalyzer::analyze(const ChainedCreaseLine& life, const ChainedCreaseLine* heart,
                                     const ChainedCreaseLine* head, int patch_dim) {
    LifeReport rep;
    const float dim = static_cast<float>(std::max(1, patch_dim));
    (void)heart;

    if (!life.is_valid || life.points.size() < 2 || life.total_length < kPresentMinLength ||
        life.confidence < kPresentMinConf) {
        return rep;
    }
    rep.present = true;

    const int gaps =
        (life.constituent_segment_ids.size() > 1) ? static_cast<int>(life.constituent_segment_ids.size() - 1) : 0;
    rep.gap_count = gaps;
    rep.broken = gaps >= 1;

    const float arc = CreaseGeometry::computeArcLength(life.points);
    const auto table = LineLengthTable::defaults();
    if (const auto* row = LineLengthTable::find(table, "life_line")) {
        const auto cls = LineLengthTable::classify(*row, arc, dim);
        rep.is_short = cls.is_short;
        rep.is_long = cls.is_long;
    }

    // Head-crossing anchor: nearest life point to the head line, if close.
    float head_t = -1.0F;
    if (head != nullptr && head->is_valid && !head->points.empty()) {
        float best_d2 = std::numeric_limits<float>::max();
        Point2D best_pt{0.0F, 0.0F};
        for (const auto& hp : head->points) {
            for (const auto& lp : life.points) {
                const float dx = lp.x - hp.x;
                const float dy = lp.y - hp.y;
                const float d2 = dx * dx + dy * dy;
                if (d2 < best_d2) {
                    best_d2 = d2;
                    best_pt = lp;
                }
            }
        }
        if (best_d2 < kHeadJointDistPx * kHeadJointDistPx) {
            head_t = nearestArcFraction(life.points, best_pt);
        }
    }

    rep.age_at_end = ageAtLifeArc(1.0F, head_t);
    if (rep.broken && life.points.size() >= 3) {
        const float tb = static_cast<float>(life.points.size() / 2) / static_cast<float>(life.points.size() - 1);
        rep.age_at_break = ageAtLifeArc(tb, head_t);
    }
    rep.age_calibrated = false;
    return rep;
}

} // namespace CurvEngine::domain
