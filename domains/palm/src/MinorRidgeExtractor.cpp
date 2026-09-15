#include "curv/domain/MinorRidgeExtractor.hpp"

#include "curv/domain/PalmZones.hpp"
#include "curv/geometry/ZoneIntersector.hpp"

#include <cmath>

namespace CurvEngine::domain {

std::vector<MinorRidgeExtractor::MinorSegment>
MinorRidgeExtractor::extract(const RidgeGraph& fine_ridges, const RidgeGraph& /*major_ridge_ids_filter*/, int zone_dim,
                             bool is_left_hand) {
    const auto zones = PalmZones::getCanonicalPalmZones(zone_dim, is_left_hand);
    const float inv_dim = 1.0f / static_cast<float>(std::max(1, zone_dim));

    std::vector<MinorSegment> out;
    for (const auto& seg : fine_ridges.segments) {
        if (seg.points.size() < 2 || seg.total_length < 8.0f || seg.total_length > 120.0f) {
            continue;
        }
        MinorSegment minor;
        minor.segment_id = seg.id;
        minor.arc_length_px = seg.total_length;
        minor.primary_zone = geometry::ZoneIntersector::findPrimaryZone(seg, zones);

        const auto& a = seg.points.front();
        const auto& b = seg.points.back();
        minor.mean_angle_deg = std::atan2(b.y - a.y, b.x - a.x) * 180.0f / static_cast<float>(M_PI);

        const size_t step = std::max<size_t>(1, seg.points.size() / 12);
        for (size_t i = 0; i < seg.points.size(); i += step) {
            minor.points_norm.emplace_back(seg.points[i].x * inv_dim, seg.points[i].y * inv_dim);
        }
        out.push_back(minor);
        if (out.size() >= 32) {
            break;
        }
    }
    return out;
}

nlohmann::json MinorRidgeExtractor::toJson(const std::vector<MinorSegment>& segments) {
    nlohmann::json arr = nlohmann::json::array();
    for (const auto& s : segments) {
        nlohmann::json pts = nlohmann::json::array();
        for (const auto& p : s.points_norm) {
            pts.push_back({p.first, p.second});
        }
        arr.push_back({{"segment_id", s.segment_id},
                       {"arc_length_px", s.arc_length_px},
                       {"mean_angle_deg", s.mean_angle_deg},
                       {"primary_zone", s.primary_zone},
                       {"points_canonical_norm", pts}});
    }
    return {{"minor_ridges", arr}, {"count", segments.size()}};
}

} // namespace CurvEngine::domain
