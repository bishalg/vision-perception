#pragma once

#include "curv/GeometryTypes.hpp"
#include <nlohmann/json.hpp>
#include <vector>

namespace CurvEngine::domain {

/**
 * @brief Extracts minor crease segments (islands, breaks) excluding major named lines.
 */
class MinorRidgeExtractor {
public:
    struct MinorSegment {
        int segment_id{0};
        float arc_length_px{0.0f};
        float mean_angle_deg{0.0f};
        std::string primary_zone;
        std::vector<std::pair<float, float>> points_norm;
    };

    [[nodiscard]] static std::vector<MinorSegment>
    extract(const RidgeGraph& fine_ridges, const RidgeGraph& major_ridge_ids_filter, int zone_dim, bool is_left_hand);

    [[nodiscard]] static nlohmann::json toJson(const std::vector<MinorSegment>& segments);
};

} // namespace CurvEngine::domain
