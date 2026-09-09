#pragma once

#include "curv/GeometryTypes.hpp"
#include <string>
#include <unordered_map>
#include <vector>

namespace CurvEngine::domain {

/**
 * @brief Continuous chained crease polyline with Catmull-Rom spline points.
 */
struct ChainedCreaseLine {
    std::string name;
    std::vector<int> constituent_segment_ids{};
    std::vector<Point2D> points{}; // Smooth spline points
    float total_length{0.0f};
    float mean_intensity{0.0f};
    bool is_valid{false};
};

/**
 * @brief Configuration parameters for topological segment chaining.
 */
struct CreaseLinkOptions {
    float max_gap_distance{35.0f}; ///< Maximum pixel gap between endpoints to bridge
    float min_collinearity{0.70f}; ///< Minimum cos(theta) between segment heading and gap vector
    float spline_step{2.0f};       ///< Arc-length step for Catmull-Rom spline sampling
};

/**
 * @brief Topological ridge segment linker and smooth Catmull-Rom spline generator.
 *
 * Bridges fragmented Steger sub-pixel segments across skin pores and interruptions,
 * synthesizing continuous, unbroken curves for the major palmistry lines.
 */
class CreaseChainLinker {
public:
    /**
     * @brief Greedily chains collinear, proximate curve segments into continuous polylines.
     */
    [[nodiscard]] static std::vector<ChainedCreaseLine>
    linkSegments(const std::vector<CurveSegment>& segments, const CreaseLinkOptions& options = CreaseLinkOptions{});

    /**
     * @brief Fits a centripetal Catmull-Rom spline through control points.
     */
    [[nodiscard]] static std::vector<Point2D> fitCatmullRomSpline(const std::vector<Point2D>& control_points,
                                                                  float step = 2.0f);

    /**
     * @brief Extracts and chains the 4 major palm lines (Heart, Head, Life, Fate).
     * @param graph Sub-pixel ridge graph in canonical or image space.
     * @param is_left_hand True if left hand, false if right hand.
     * @param patch_dim Dimension of canonical patch (e.g. 512).
     * @return Map of line name ("heart_line", "head_line", "life_line", "fate_line") to chained curve.
     */
    [[nodiscard]] static std::unordered_map<std::string, ChainedCreaseLine>
    extractMajorLines(const RidgeGraph& graph, bool is_left_hand = false, int patch_dim = 512);
};

} // namespace CurvEngine::domain
