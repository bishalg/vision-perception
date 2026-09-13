#include "curv/domain/HeartHeadAnalyzer.hpp"

#include <algorithm>
#include <cmath>

#include "curv/domain/CreaseGeometry.hpp"
#include "curv/domain/FateLineAnalyzer.hpp"
#include "curv/domain/LineLengthThresholds.hpp"

namespace CurvEngine::domain {

namespace {

constexpr float kPresentMinLength = 40.0F;
constexpr float kPresentMinConf = 0.55F;
constexpr float kSimianMergePx = 6.0F;
constexpr float kJointDistPx = 10.0F;
constexpr float kSimianOverlapPx = 30.0F;

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

bool mergedPair(const ChainedCreaseLine& a, const ChainedCreaseLine& b) {
    if (a.points.empty() || b.points.empty()) {
        return false;
    }
    // Dominant-axis overlap: vertical pairs (fate) overlap in Y, horizontal
    // pairs (heart/head) in X. Either suffices for a merge.
    const float d = CreaseGeometry::computeMinDistance(a.points, b.points);
    if (d >= kSimianMergePx) {
        return false;
    }
    float a_min_x = a.points.front().x;
    float a_max_x = a.points.front().x;
    float b_min_x = b.points.front().x;
    float b_max_x = b.points.front().x;
    for (const auto& p : a.points) {
        a_min_x = std::min(a_min_x, p.x);
        a_max_x = std::max(a_max_x, p.x);
    }
    for (const auto& p : b.points) {
        b_min_x = std::min(b_min_x, p.x);
        b_max_x = std::max(b_max_x, p.x);
    }
    const float overlap_x = std::max(0.0F, std::min(a_max_x, b_max_x) - std::max(a_min_x, b_min_x));
    return (overlapY(a.points, b.points) > kSimianOverlapPx) || (overlap_x > kSimianOverlapPx);
}

int gapCountOf(const ChainedCreaseLine& line) {
    return (line.constituent_segment_ids.size() > 1) ? static_cast<int>(line.constituent_segment_ids.size() - 1) : 0;
}

bool validLine(const ChainedCreaseLine& line) {
    return line.is_valid && line.points.size() >= 2 && line.total_length >= kPresentMinLength &&
           line.confidence >= kPresentMinConf;
}

} // namespace

nlohmann::json HeartReport::toJson() const {
    return {
        {"present", present},
        {"is_short", is_short},
        {"is_long", is_long},
        {"broken", broken},
        {"gap_count", gap_count},
        {"simian_merge", simian_merge},
        {"terminates_fate", terminates_fate},
    };
}

nlohmann::json HeadReport::toJson() const {
    return {
        {"present", present},
        {"is_short", is_short},
        {"is_long", is_long},
        {"broken", broken},
        {"gap_count", gap_count},
        {"merged_heart", merged_heart},
        {"joined_life_origin", joined_life_origin},
    };
}

HeartReport HeartLineAnalyzer::analyze(const ChainedCreaseLine& heart, const ChainedCreaseLine* head,
                                       const ChainedCreaseLine* fate, int patch_dim, float palm_width_px) {
    HeartReport rep;
    (void)patch_dim;
    if (!validLine(heart)) {
        return rep;
    }
    rep.present = true;
    rep.gap_count = gapCountOf(heart);
    rep.broken = rep.gap_count >= 1;

    const float arc = CreaseGeometry::computeArcLength(heart.points);
    const auto table = LineLengthTable::defaults();
    if (const auto* row = LineLengthTable::find(table, "heart_line")) {
        const auto cls = LineLengthTable::classify(*row, arc, palm_width_px);
        rep.is_short = cls.is_short;
        rep.is_long = cls.is_long;
    }

    rep.simian_merge = (head != nullptr && head->is_valid) && mergedPair(heart, *head);
    rep.terminates_fate = (fate != nullptr && fate->is_valid) && FateLineAnalyzer::stopsAt(fate->points, heart.points);
    return rep;
}

HeadReport HeadLineAnalyzer::analyze(const ChainedCreaseLine& head, const ChainedCreaseLine* heart,
                                     const ChainedCreaseLine* life, int patch_dim, float palm_width_px) {
    HeadReport rep;
    (void)patch_dim;
    if (!validLine(head)) {
        return rep;
    }
    rep.present = true;
    rep.gap_count = gapCountOf(head);
    rep.broken = rep.gap_count >= 1;

    const float arc = CreaseGeometry::computeArcLength(head.points);
    const auto table = LineLengthTable::defaults();
    if (const auto* row = LineLengthTable::find(table, "head_line")) {
        const auto cls = LineLengthTable::classify(*row, arc, palm_width_px);
        rep.is_short = cls.is_short;
        rep.is_long = cls.is_long;
    }

    rep.merged_heart = (heart != nullptr && heart->is_valid) && mergedPair(head, *heart);
    rep.joined_life_origin = false;
    if (life != nullptr && life->is_valid && !life->points.empty() && !head.points.empty()) {
        const float d = CreaseGeometry::computeMinDistance({head.points.front()}, life->points);
        rep.joined_life_origin = d < kJointDistPx;
    }
    return rep;
}

} // namespace CurvEngine::domain
