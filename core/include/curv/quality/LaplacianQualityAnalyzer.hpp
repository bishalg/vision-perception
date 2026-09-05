#pragma once

#include "curv/QualityReport.hpp"
#include "curv/interfaces/IQualityAnalyzer.hpp"

namespace CurvEngine::quality {

/**
 * @brief Configurable quality thresholds for input frame validation.
 */
struct QualityThresholds {
    double min_blur_score{80.0};  ///< Minimum Laplacian variance
    double min_brightness{20.0};  ///< Minimum mean pixel intensity [0, 255]
    double max_brightness{240.0}; ///< Maximum mean pixel intensity [0, 255]
    double min_contrast{15.0};    ///< Minimum standard deviation of intensity
};

/**
 * @brief Concrete quality analyzer utilizing Laplacian variance and luminance statistics.
 */
class LaplacianQualityAnalyzer final : public interfaces::IQualityAnalyzer {
public:
    explicit LaplacianQualityAnalyzer(QualityThresholds thresholds = QualityThresholds{});
    ~LaplacianQualityAnalyzer() override = default;

    [[nodiscard]] QualityReport analyze(const Frame& frame) override;

    [[nodiscard]] const QualityThresholds& getThresholds() const noexcept { return thresholds_; }
    void setThresholds(const QualityThresholds& thresholds) noexcept { thresholds_ = thresholds; }

private:
    QualityThresholds thresholds_;
};

} // namespace CurvEngine::quality
