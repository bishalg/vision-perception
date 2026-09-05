#pragma once

#include <cstdint>
#include <opencv2/core.hpp>
#include <vector>

namespace CurvEngine {

/**
 * @brief Sub-pixel 2D point representation with radiometric intensity and confidence metric.
 */
struct Point2D {
    float x{0.0f};
    float y{0.0f};
    float intensity{0.0f};
    float confidence{1.0f};

    Point2D() = default;
    constexpr Point2D(float in_x, float in_y, float in_intensity = 0.0f, float in_confidence = 1.0f) noexcept
        : x(in_x), y(in_y), intensity(in_intensity), confidence(in_confidence) {}
};

/**
 * @brief Continuous curvilinear ridge segment represented as an ordered sequence of sub-pixel points.
 */
struct CurveSegment {
    int id{0};
    std::vector<Point2D> points;
    float total_length{0.0f};
    float average_curvature{0.0f};
    bool is_closed{false};
};

/**
 * @brief Topological junction or bifurcation point connecting multiple curvilinear segments.
 */
struct Junction {
    Point2D position;
    std::vector<int> connected_segment_ids;
    float branch_angle{0.0f};
};

/**
 * @brief Complete topological graph representation of extracted curvilinear structures.
 */
struct RidgeGraph {
    std::vector<CurveSegment> segments;
    std::vector<Junction> junctions;
    cv::Mat debug_mask;
};

} // namespace CurvEngine
