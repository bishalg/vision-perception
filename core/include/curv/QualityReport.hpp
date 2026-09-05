#pragma once

#include <string>

namespace CurvEngine {

/**
 * @brief Assessment metrics and usability verdict for an analyzed frame.
 */
struct QualityReport {
    double blur_score{0.0};
    double brightness_score{0.0};
    double contrast_score{0.0};
    bool is_usable{false};
    std::string recommendation;

    QualityReport() = default;

    QualityReport(double in_blur_score, double in_brightness_score, double in_contrast_score, bool in_is_usable,
                  std::string in_recommendation = "");
};

} // namespace CurvEngine
