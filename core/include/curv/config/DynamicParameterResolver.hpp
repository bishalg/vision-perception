#pragma once

#include "curv/ridge/StegerRidgeExtractor.hpp"

namespace CurvEngine::config {

/**
 * @brief Dynamic parameter resolver that adapts Steger extraction parameters
 * to the physical pixel scale of the image or ROI.
 */
class DynamicParameterResolver {
public:
    struct BaselineConfig {
        float reference_diagonal{500.0f}; // Baseline diagonal length in pixels
        float base_sigma{1.5f};           // Baseline Gaussian filter scale
        float base_low_thresh{0.5f};      // Baseline low hysteresis threshold
        float base_high_thresh{1.5f};     // Baseline high hysteresis threshold
        float min_segment_length{5.0f};   // Baseline minimum polyline length
        bool extract_dark_lines{false};
    };

    DynamicParameterResolver();
    explicit DynamicParameterResolver(BaselineConfig baseline);

    /**
     * @brief Resolves tuned StegerConfig based on the ROI diagonal length in pixels.
     * @param roi_diagonal_pixels Measured diagonal of ROI bounding box or quad.
     * @return Tuned StegerConfig.
     */
    [[nodiscard]] ridge::StegerConfig resolve(float roi_diagonal_pixels) const;

    /**
     * @brief Resolves tuned StegerConfig based on ROI width and height.
     * @param roi_width Width in pixels.
     * @param roi_height Height in pixels.
     * @return Tuned StegerConfig.
     */
    [[nodiscard]] ridge::StegerConfig resolve(int roi_width, int roi_height) const;

private:
    BaselineConfig baseline_{};
};

} // namespace CurvEngine::config
