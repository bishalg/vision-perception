#pragma once

#include "curv/GeometryTypes.hpp"
#include <string>
#include <utility>
#include <vector>

namespace CurvEngine::geometry {

/**
 * @brief Represents a 2D spatial polygonal zone for semantic topological mapping.
 */
struct Zone {
    std::string id;
    std::vector<Point2D> polygon; // Ordered vertices defining the polygon boundary
};

/**
 * @brief Domain-agnostic geometric utility to determine intersection and
 * containment of curve segments within 2D polygonal zones.
 */
class ZoneIntersector {
public:
    /**
     * @brief Checks if a single Point2D is inside or on the boundary of a Zone polygon.
     */
    static bool isPointInZone(const Point2D& point, const Zone& zone);

    /**
     * @brief Finds the primary zone containing the majority of points in a curve segment.
     * @param segment Curve segment containing sub-pixel points.
     * @param zones List of candidate zones.
     * @param min_overlap_ratio Minimum fraction of points [0.0, 1.0] required (default: 0.20f).
     * @return Zone ID with greatest point overlap, or "unknown" if no zone meets min_overlap_ratio.
     */
    static std::string findPrimaryZone(const CurveSegment& segment, const std::vector<Zone>& zones,
                                       float min_overlap_ratio = 0.20f);

    /**
     * @brief Computes detailed overlap ratios across all zones for a curve segment.
     * @return Vector of pairs (zone_id, overlap_fraction in [0.0, 1.0]).
     */
    static std::vector<std::pair<std::string, float>> computeZoneOverlap(const CurveSegment& segment,
                                                                         const std::vector<Zone>& zones);
};

} // namespace CurvEngine::geometry
