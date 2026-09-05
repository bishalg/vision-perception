#pragma once

#include "curv/Frame.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include <array>

namespace CurvEngine::roi {

/**
 * @brief High-level canonical ROI warper that warps source frames into canonical square frames.
 */
class CanonicalWarper {
public:
    explicit CanonicalWarper(int target_dimension = 512);

    /**
     * @brief Warps source frame quadrilateral region into canonical upright square frame.
     * @param source_frame Original input frame.
     * @param corners 4 corner points (TL, TR, BR, BL) in source coordinates.
     * @param output_size Output square resolution (default: 512).
     * @return Frame containing the warped canonical image.
     */
    [[nodiscard]] Frame warpQuad(const Frame& source_frame, const std::array<Point2D, 4>& corners,
                                 int output_size = 512) const;

    /**
     * @brief Warps source frame palm region into canonical upright square frame using hand landmarks.
     * @param source_frame Original input frame.
     * @param landmarks 21 hand landmarks.
     * @param output_size Output square resolution (default: 512).
     * @return Frame containing the warped canonical image and transform cached in metadata.
     */
    [[nodiscard]] Frame warpPalm(const Frame& source_frame, const geometry::HandLandmarks& landmarks,
                                 int output_size = 512) const;

private:
    int target_dim_;
};

} // namespace CurvEngine::roi
