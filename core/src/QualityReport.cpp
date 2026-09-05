#include "curv/QualityReport.hpp"

namespace CurvEngine {

QualityReport::QualityReport(double in_blur_score, double in_brightness_score, double in_contrast_score,
                             bool in_is_usable, std::string in_recommendation)
    : blur_score(in_blur_score), brightness_score(in_brightness_score), contrast_score(in_contrast_score),
      is_usable(in_is_usable), recommendation(std::move(in_recommendation)) {}

} // namespace CurvEngine
