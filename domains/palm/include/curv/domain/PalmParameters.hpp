#pragma once

#include "curv/geometry/RoiWarper.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include <algorithm>
#include <cmath>

namespace CurvEngine::domains {

/**
 * @brief Configuration baseline for palm scale adaptation.
 */
struct PalmScaleConfig {
    float reference_palm_radius_px{150.0f}; // Reference palm radius in canonical/patch space
    float base_sigma{1.6f};                 // Base Gaussian sigma for palm creases
    float base_low_thresh{0.4f};            // Base low hysteresis threshold
    float base_high_thresh{1.2f};           // Base high hysteresis threshold
    float base_min_length{15.0f};           // Base minimum crease length
};

/**
 * @brief Palm domain specific scale-adaptive parameter calculations.
 */
class PalmParameters {
public:
    /**
     * @brief Computes tuned StegerConfig based on palm disc radius.
     * @param palm_radius_px Radius of the palm disc in pixels.
     * @param cfg Palm scale configuration baseline.
     * @return Tuned StegerConfig.
     */
    static ridge::StegerConfig computeStegerConfig(float palm_radius_px,
                                                   const PalmScaleConfig& cfg = PalmScaleConfig{}) {
        ridge::StegerConfig result;
        result.extract_dark_lines = true;

        const float r = std::max(10.0f, palm_radius_px);
        const float ref_r = std::max(10.0f, cfg.reference_palm_radius_px);

        // Linear scaling with clamped ratio
        const float ratio = std::clamp(r / ref_r, 0.4f, 3.0f);

        result.sigma = cfg.base_sigma * ratio;
        result.low_threshold = cfg.base_low_thresh * ratio;
        result.high_threshold = cfg.base_high_thresh * ratio;
        result.min_segment_length = cfg.base_min_length * ratio;

        return result;
    }

    /**
     * @brief Extracts palm disc radius from 21 hand landmarks and computes tuned parameters.
     * @param target_canonical_dim If > 0, computes parameter scale for canonical warped patch.
     */
    static ridge::StegerConfig computeFromLandmarks(const geometry::HandLandmarks& lms, int img_width, int img_height,
                                                    int target_canonical_dim = 512,
                                                    const PalmScaleConfig& cfg = PalmScaleConfig{}) {
        const float w_f = static_cast<float>(std::max(1, img_width));
        const float h_f = static_cast<float>(std::max(1, img_height));

        auto get_pt = [&](int idx) -> cv::Point2f {
            const auto& pt = lms.points[static_cast<size_t>(idx)];
            if (lms.is_normalized) {
                return cv::Point2f(pt.x * w_f, pt.y * h_f);
            }
            return cv::Point2f(pt.x, pt.y);
        };

        const cv::Point2f p5 = get_pt(5);   // Index MCP
        const cv::Point2f p17 = get_pt(17); // Pinky MCP

        const float palm_w = static_cast<float>(cv::norm(p17 - p5));
        float palm_radius = palm_w * 0.5f;

        if (target_canonical_dim > 0 && palm_w > 1e-4f) {
            const float target_palm_width = static_cast<float>(target_canonical_dim) * 0.55f;
            palm_radius = target_palm_width * 0.5f;
        }

        return computeStegerConfig(palm_radius, cfg);
    }
};

} // namespace CurvEngine::domains

// Compatibility namespace alias for curv/domain/PalmParameters.hpp
namespace CurvEngine::domain {
using PalmScaleConfig = CurvEngine::domains::PalmScaleConfig;
using PalmParameters = CurvEngine::domains::PalmParameters;
} // namespace CurvEngine::domain
