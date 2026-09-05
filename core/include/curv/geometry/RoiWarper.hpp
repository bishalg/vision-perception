#pragma once

#include "curv/GeometryTypes.hpp"
#include <array>
#include <opencv2/core.hpp>
#include <vector>

namespace CurvEngine::geometry {

/**
 * @brief Landmark point in normalized [0.0, 1.0] or pixel coordinate space.
 */
struct LandmarkPoint {
    float x{0.0f};
    float y{0.0f};

    LandmarkPoint() = default;
    constexpr LandmarkPoint(float x_val, float y_val) : x(x_val), y(y_val) {}
};

/**
 * @brief 21-point hand skeleton landmarks following MediaPipe Hands topology.
 * Key indices:
 *   0:  Wrist
 *   5:  Index finger MCP
 *   9:  Middle finger MCP
 *   13: Ring finger MCP
 *   17: Pinky finger MCP
 */
struct HandLandmarks {
    std::array<LandmarkPoint, 21> points{};
    bool is_normalized{true}; // true if coordinates are in [0, 1] relative to frame dimensions
};

/**
 * @brief Represents an affine transform to and from canonical ROI space.
 */
struct RoiTransform {
    cv::Mat M;                      // 2x3 affine matrix: source frame -> canonical patch
    cv::Mat M_inv;                  // 2x3 affine matrix: canonical patch -> source frame
    cv::Point2f center{0.0f, 0.0f}; // Centroid in source coordinates
    float angle_deg{0.0f};          // Rotation angle applied in degrees
    float scale{1.0f};              // Scale factor applied
    int target_dim{512};            // Canonical patch width & height
};

/**
 * @brief Computes a canonical upright affine transform for a palm region.
 *
 * - Direction: Axis from Wrist (Point 0) to Middle MCP (Point 9) is rotated to point vertically upwards.
 * - Center: Centroid of the palm disc points [0, 5, 9, 13, 17].
 * - Scale: Palm width ||P5 - P17|| is normalized to occupy a fixed portion of the target dimension.
 *
 * @param lms 21 hand landmarks.
 * @param img_width Original image width in pixels.
 * @param img_height Original image height in pixels.
 * @param target_dim Canonical output square dimension (default: 512).
 * @return RoiTransform containing forward and inverse 2x3 affine matrices.
 */
RoiTransform computeCanonicalPalmTransform(const HandLandmarks& lms, int img_width, int img_height,
                                           int target_dim = 512);

/**
 * @brief Determines if the hand landmarks correspond to a left hand or right hand.
 * For a palm facing the camera:
 * - In a right hand, the index finger (Point 5) is to the left of the pinky (Point 17) along the transverse axis.
 * - In a left hand, the index finger is to the right of the pinky along the transverse axis.
 *
 * @param lms 21-point hand landmarks.
 * @return true if left hand, false if right hand.
 */
bool detectIsLeftHand(const HandLandmarks& lms);

/**
 * @brief Computes a perspective transform from 4 arbitrary quadrilateral corner points.
 *
 * @param corners Top-Left, Top-Right, Bottom-Right, Bottom-Left corners in source image.
 * @param target_dim Canonical output square dimension (default: 512).
 * @return cv::Mat 3x3 perspective matrix.
 */
cv::Mat computeQuadPerspectiveTransform(const std::array<Point2D, 4>& corners, int target_dim = 512);

/**
 * @brief Warps an input image into the canonical coordinate frame using a 2x3 affine matrix.
 *
 * @param src Source image raster.
 * @param M 2x3 affine transformation matrix.
 * @param target_dim Output dimension (width = height = target_dim).
 * @return cv::Mat Warped canonical square image.
 */
cv::Mat warpToCanonical(const cv::Mat& src, const cv::Mat& M, int target_dim = 512);

/**
 * @brief Inverts extracted sub-pixel curves from canonical patch space back into the original frame space.
 *
 * @param graph Sub-pixel ridge graph extracted on the canonical patch (modified in-place).
 * @param M_inv 2x3 inverse affine transformation matrix.
 */
void invertGraphCoordinates(RidgeGraph& graph, const cv::Mat& M_inv);

} // namespace CurvEngine::geometry
