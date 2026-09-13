#pragma once

#include "curv/GeometryTypes.hpp"
#include <nlohmann/json.hpp>
#include <string>

namespace CurvEngine::domain {

enum class DermatoglyphicPattern { Arch, Loop, Whorl, Unknown };

struct FingertipPatternResult {
    std::string digit;
    DermatoglyphicPattern pattern{DermatoglyphicPattern::Unknown};
    float ridge_frequency{0.0f};
    float quality{0.0f};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Classifies dermatoglyphic pattern from ridge orientation histogram.
 * Pattern class only — never emits a biometric template.
 */
class FingertipPatternAnalyzer {
public:
    [[nodiscard]] static FingertipPatternResult classify(const RidgeGraph& ridges, const std::string& digit_name);
};

} // namespace CurvEngine::domain
