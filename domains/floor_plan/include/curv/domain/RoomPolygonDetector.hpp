#pragma once

#include "curv/domain/WallTypes.hpp"
#include <vector>

namespace CurvEngine::domain {

struct RoomDetectionConfig {
    float min_room_area_px{400.0f};   // Discard tiny polygons (e.g. wall junction artifacts)
    float max_room_area_ratio{0.95f}; // Discard exterior envelope if it covers almost the whole canvas
    float pixels_per_meter{0.0f};     // 0.0 means uncalibrated
};

class RoomPolygonDetector {
public:
    explicit RoomPolygonDetector(RoomDetectionConfig config = RoomDetectionConfig{}) : config_(config) {}

    /**
     * @brief Traverses planar cycles in the wall junction graph to delineate closed room polygons.
     * Computes metric dimensions, perimeter, centroid, and area.
     */
    [[nodiscard]] std::vector<RoomPolygon> detectRooms(const std::vector<WallSegment>& walls,
                                                       const std::vector<WallJunction>& junctions) const;

private:
    RoomDetectionConfig config_;
};

} // namespace CurvEngine::domain
