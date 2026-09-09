#pragma once

#include "curv/domain/WallTypes.hpp"
#include <vector>

namespace CurvEngine::domain {

struct JunctionGraphConfig {
    float snap_distance{12.0f};            // Tolerance to snap nearby wall endpoints into a single junction
    float orthogonal_tolerance_deg{15.0f}; // Angular tolerance for L-corners
};

class WallJunctionGraph {
public:
    explicit WallJunctionGraph(JunctionGraphConfig config = JunctionGraphConfig{}) : config_(config) {}

    /**
     * @brief Constructs a topological junction graph from rectified wall segments.
     * Snaps endpoints, detects T-junctions on wall bodies, and classifies junction types.
     */
    void build(std::vector<WallSegment>& walls);

    [[nodiscard]] const std::vector<WallJunction>& junctions() const noexcept { return junctions_; }
    [[nodiscard]] const std::vector<WallSegment>& snappedWalls() const noexcept { return walls_; }

private:
    JunctionGraphConfig config_;
    std::vector<WallJunction> junctions_;
    std::vector<WallSegment> walls_;

    int findOrCreateJunction(const Point2D& pt, int wall_id);
};

} // namespace CurvEngine::domain
