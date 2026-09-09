#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/domain/WallTypes.hpp"
#include <vector>

namespace CurvEngine::domain {

struct WallExtractionConfig {
    float min_wall_length{15.0f};         // Minimum length to consider a ridge as a wall
    float manhattan_tolerance_deg{12.0f}; // Angular tolerance to snap to 0 or 90 degrees
    float collinear_dist_tolerance{8.0f}; // Max perpendicular distance to consider collinear
    float merge_gap_threshold{25.0f};     // Max gap between endpoints to merge into continuous wall
    float default_wall_thickness{12.0f};  // Default thickness if radiometric width unavailable
};

class WallSegmentExtractor {
public:
    explicit WallSegmentExtractor(WallExtractionConfig config = WallExtractionConfig{}) : config_(config) {}

    /**
     * @brief Extracts and rectifies straight wall segments from extracted sub-pixel curvilinear ridges.
     */
    [[nodiscard]] std::vector<WallSegment> extractWalls(const RidgeGraph& ridge_graph) const;

private:
    WallExtractionConfig config_;
};

} // namespace CurvEngine::domain
