#include "curv/config/DynamicParameterResolver.hpp"
#include <algorithm>
#include <cmath>

namespace CurvEngine::config {

DynamicParameterResolver::DynamicParameterResolver() = default;

DynamicParameterResolver::DynamicParameterResolver(BaselineConfig baseline) : baseline_(baseline) {}

ridge::StegerConfig DynamicParameterResolver::resolve(float roi_diagonal_pixels) const {
    ridge::StegerConfig cfg;
    cfg.extract_dark_lines = baseline_.extract_dark_lines;

    const float diag = std::max(10.0f, roi_diagonal_pixels);
    const float ref_diag = std::max(10.0f, baseline_.reference_diagonal);

    // Scale factor based on diagonal ratio
    const float scale_ratio = diag / ref_diag;

    // Sigma scales with square root to prevent excessive blurring on 4K resolutions
    // while scaling up adequately on larger ROIs
    const float sigma_scale = std::clamp(std::sqrt(scale_ratio), 0.5f, 3.5f);
    cfg.sigma = baseline_.base_sigma * sigma_scale;

    // Thresholds scale moderately with scale ratio
    const float thresh_scale = std::clamp(scale_ratio, 0.5f, 2.5f);
    cfg.low_threshold = baseline_.base_low_thresh * thresh_scale;
    cfg.high_threshold = baseline_.base_high_thresh * thresh_scale;

    cfg.min_segment_length = baseline_.min_segment_length * sigma_scale;

    return cfg;
}

ridge::StegerConfig DynamicParameterResolver::resolve(int roi_width, int roi_height) const {
    const float w = static_cast<float>(std::max(1, roi_width));
    const float h = static_cast<float>(std::max(1, roi_height));
    const float diagonal = std::sqrt(w * w + h * h);
    return resolve(diagonal);
}

} // namespace CurvEngine::config
