#pragma once

#include "curv/GeometryTypes.hpp"
#include <string>
#include <vector>

namespace CurvEngine::pattern {

/**
 * @brief Categorization of morphological signs & micro-patterns on crease curves.
 */
enum class PatternType {
    Square,
    Triangle,
    Star,
    Circle,
    Island,
    Cross,
    Trident,
    SupportingLine,
    ChainedLine,
    Tassels,
    Net,
    Fish
};

/**
 * @brief Detected geometric sign or micro-pattern.
 */
struct DetectedPattern {
    PatternType type{PatternType::Square};
    std::string name;
    float confidence{1.0f};
    Point2D centroid;
    std::vector<Point2D> points;
    std::vector<int> source_segment_ids;

    // Optional host line attachment
    int host_curve_id{-1};
    float host_arc_s{0.0f};       // Relative position [0.0, 1.0] along host curve
    float estimated_scalar{0.0f}; // e.g. age at pattern location
};

/**
 * @brief Topological graph analyzer that detects micro-patterns from RidgeGraph primitives.
 */
class MicroPatternDetector {
public:
    [[nodiscard]] static std::vector<DetectedPattern> detectAll(const RidgeGraph& graph);

    [[nodiscard]] static std::vector<DetectedPattern> findTriangles(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findSquares(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findIslands(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findCrosses(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findStars(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findTridents(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findNets(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findFish(const RidgeGraph& graph);
};

} // namespace CurvEngine::pattern
