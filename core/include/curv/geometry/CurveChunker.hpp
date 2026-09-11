#pragma once

#include "curv/GeometryTypes.hpp"
#include <string>
#include <vector>

namespace CurvEngine::geometry {

/**
 * @brief Calibration anchor mapping a relative arc position s in [0, 1] to a domain scalar.
 */
struct CalibrationAnchor {
    float s{0.0f};      // Normalized position [0.0, 1.0] along the curve
    float scalar{0.0f}; // Value at this anchor (e.g., age 35.0, distance 100.0)
};

/**
 * @brief Represents a discrete segment/chunk partitioned along a continuous curve.
 */
struct CurveChunk {
    int index{0};              // 0-indexed order
    float s_start{0.0f};       // Normalized start in [0.0, 1.0]
    float s_end{0.0f};         // Normalized end in [0.0, 1.0]
    float arc_length_px{0.0f}; // Arc length in pixels

    Point2D start_pt; // Coordinates at s_start
    Point2D mid_pt;   // Coordinates at midpoint (s_start + s_end) / 2
    Point2D end_pt;   // Coordinates at s_end

    Point2D tangent{1.0f, 0.0f}; // Unit direction along the curve
    Point2D normal{0.0f, 1.0f};  // Perpendicular unit vector (for tick lines)

    float scalar_value{0.0f}; // Assigned value at midpoint (e.g. age)
    std::string label;        // Formatted display label (e.g. "35")
};

/**
 * @brief Represents a partitioned curve discretized into an array of chunks.
 */
struct PartitionedCurve {
    int curve_id{0};
    float total_arc_length{0.0f};
    std::vector<Point2D> cumulative_points; // Resampled polyline
    std::vector<float> cumulative_lengths;  // Arc-length at each point
    std::vector<CurveChunk> chunks;

    [[nodiscard]] Point2D sampleAt(float s) const;
    [[nodiscard]] Point2D tangentAt(float s) const;
    [[nodiscard]] Point2D normalAt(float s) const;
    [[nodiscard]] int chunkIndexForArc(float s) const;
};

/**
 * @brief Generic engine for dividing curves into discrete chunks and assigning scalars.
 */
class CurveChunker {
public:
    /**
     * @brief Divides a curve into K uniform arc-length chunks.
     * @param points Ordered polyline points.
     * @param num_chunks Number of discrete segments.
     * @param val_start Scalar value mapped to s=0.
     * @param val_end Scalar value mapped to s=1.
     */
    [[nodiscard]] static PartitionedCurve divideUniform(const std::vector<Point2D>& points, int num_chunks,
                                                        float val_start = 0.0f, float val_end = 100.0f);

    /**
     * @brief Divides a curve using piecewise calibration anchors.
     * @param points Ordered polyline points.
     * @param anchors Sequence of (s, value) calibration anchors (must be sorted by s).
     * @param sub_chunks_per_interval Number of sub-segments per anchor interval.
     */
    [[nodiscard]] static PartitionedCurve divideByAnchors(const std::vector<Point2D>& points,
                                                          const std::vector<CalibrationAnchor>& anchors,
                                                          int sub_chunks_per_interval = 5);
};

} // namespace CurvEngine::geometry
