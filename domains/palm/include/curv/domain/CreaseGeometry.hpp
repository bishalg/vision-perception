#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/domain/CreaseChainLinker.hpp"
#include "curv/geometry/ZoneIntersector.hpp"
#include <string>
#include <vector>

namespace CurvEngine::domain {

/**
 * @brief Geometric and topological measurements for a chained crease.
 */
struct CreaseMetrics {
    float arc_length_px{0.0f};
    float chord_length_px{0.0f};
    float tortuosity{1.0f};
    float mean_angle_deg{0.0f};
    int gap_count{0};
    int fork_count{0};
    float mean_width_px{0.0f};
    float mean_intensity{0.0f};
    std::string start_zone{"unknown"};
    std::string end_zone{"unknown"};
};

/**
 * @brief Polyline geometry calculator for palm creases (arc length, tortuosity, angle, distances).
 */
class CreaseGeometry {
public:
    [[nodiscard]] static float computeArcLength(const std::vector<Point2D>& points);
    [[nodiscard]] static float computeChordLength(const std::vector<Point2D>& points);
    [[nodiscard]] static float computeTortuosity(float arc_length, float chord_length);
    [[nodiscard]] static float computeAngleDeg(const Point2D& start, const Point2D& end);
    [[nodiscard]] static float computeMinDistance(const std::vector<Point2D>& a, const std::vector<Point2D>& b);
    [[nodiscard]] static int countForks(const std::vector<int>& segment_ids, const std::vector<Junction>& junctions);
    [[nodiscard]] static int countIntersections(const std::vector<Point2D>& a, const std::vector<Point2D>& b);

    [[nodiscard]] static CreaseMetrics computeMetrics(const ChainedCreaseLine& chain, const RidgeGraph& graph,
                                                      const std::vector<geometry::Zone>& zones);
};

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using CreaseMetrics = CurvEngine::domain::CreaseMetrics;
using CreaseGeometry = CurvEngine::domain::CreaseGeometry;
} // namespace CurvEngine::domains
