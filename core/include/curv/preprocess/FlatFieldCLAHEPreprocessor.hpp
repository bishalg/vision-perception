#pragma once

#include "curv/Frame.hpp"
#include "curv/interfaces/IPreprocessor.hpp"

#include <opencv2/core.hpp>

namespace CurvEngine::preprocess {

/**
 * @brief Configuration for FlatFieldCLAHEPreprocessor. All sizes derive from the palm
 *        width so they survive device/resolution changes.
 */
struct FlatFieldCLAHEConfig {
    bool enabled{true};
    float close_kernel_frac{0.20F}; ///< Background-estimate closing kernel as a fraction of palm width
    int close_kernel_min_px{31};    ///< Floor for the closing kernel (px) at small palm widths
    int background_floor{30};       ///< Clamp for the estimated background (protects the division)
    double clahe_clip{2.0};         ///< CLAHE clip limit
    int clahe_tiles{8};             ///< CLAHE tile grid (clahe_tiles x clahe_tiles)
};

/**
 * @brief First concrete IPreprocessor: illumination flattening + local contrast lift.
 *
 * Palm captures are washed out in the center and dark at the edges, so faint creases
 * vanish in bright zones. Stage 1 estimates the low-frequency illumination with a
 * morphological closing (kernel ~20% of palm width) and divides it out; stage 2 applies
 * CLAHE to pop faint secondary creases without amplifying pore noise.
 *
 * Runs BEFORE the quality gate and ridge extraction so both judge the flattened image.
 */
class FlatFieldCLAHEPreprocessor final : public CurvEngine::interfaces::IPreprocessor {
public:
    explicit FlatFieldCLAHEPreprocessor(FlatFieldCLAHEConfig config = {});

    /**
     * @brief Flattens + contrast-lifts a canonical patch. palm_width_px drives the
     *        closing-kernel size; a non-positive value falls back to 20% of the image.
     * @return Single-channel CV_8U image (input untouched when disabled).
     */
    [[nodiscard]] cv::Mat apply(const cv::Mat& image, float palm_width_px) const;

    [[nodiscard]] Frame process(const Frame& frame) override;

    [[nodiscard]] const FlatFieldCLAHEConfig& config() const noexcept { return config_; }

private:
    FlatFieldCLAHEConfig config_;
};

} // namespace CurvEngine::preprocess
