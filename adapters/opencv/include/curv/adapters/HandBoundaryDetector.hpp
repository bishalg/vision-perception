#pragma once

#include "curv/GeometryTypes.hpp"
#include <opencv2/core.hpp>
#include <vector>

namespace CurvEngine::adapters {

/**
 * @brief Output result of hand boundary segmentation.
 */
struct HandBoundaryResult {
    cv::Mat mask;                                 ///< 8-bit binary mask (255 inside hand, 0 outside)
    std::vector<cv::Point> contour;               ///< Primary outer boundary polygon vertices
    std::vector<std::vector<cv::Point>> contours; ///< Array of contours ready for cv::drawContours
    double area{0.0};                             ///< Pixel area enclosed by the boundary
    bool is_valid{false};                         ///< True if a valid boundary was extracted
};

/**
 * @brief Anatomical hand and palm boundary detector based on chrominance segmentation
 * and morphological contour extraction.
 */
class HandBoundaryDetector {
public:
    /**
     * @brief Detects the anatomical hand/palm boundary mask and contour from color chrominance.
     * @param image Input image (BGR format).
     * @param downscale_factor Downscale factor for rapid mask processing on high-res photos (e.g. 0.25f).
     * @return HandBoundaryResult containing mask, contour, and geometric metrics.
     */
    [[nodiscard]] static HandBoundaryResult detectBoundary(const cv::Mat& image, float downscale_factor = 1.0f);

    /**
     * @brief Filters a RidgeGraph in place, pruning segments that fall outside the hand mask.
     * @param graph The RidgeGraph to filter.
     * @param mask 8-bit binary mask where non-zero indicates the hand interior.
     * @param min_inside_ratio Minimum ratio of segment points required to lie inside the mask (0.0 to 1.0).
     */
    static void filterGraphByMask(RidgeGraph& graph, const cv::Mat& mask, float min_inside_ratio = 0.70f);
};

} // namespace CurvEngine::adapters
