#include "curv/domain/FingertipPatternAnalyzer.hpp"

#include <algorithm>
#include <cmath>

namespace CurvEngine::domain {

namespace {

const char* patternString(DermatoglyphicPattern p) {
    switch (p) {
    case DermatoglyphicPattern::Arch:
        return "arch";
    case DermatoglyphicPattern::Loop:
        return "loop";
    case DermatoglyphicPattern::Whorl:
        return "whorl";
    default:
        return "unknown";
    }
}

} // anonymous namespace

nlohmann::json FingertipPatternResult::toJson() const {
    return {{"digit", digit},
            {"pattern", patternString(pattern)},
            {"ridge_frequency", ridge_frequency},
            {"quality", quality}};
}

FingertipPatternResult FingertipPatternAnalyzer::classify(const RidgeGraph& ridges, const std::string& digit_name) {
    FingertipPatternResult result;
    result.digit = digit_name;

    if (ridges.segments.empty()) {
        result.pattern = DermatoglyphicPattern::Unknown;
        result.quality = 0.0f;
        return result;
    }

    float sum_angle = 0.0f;
    float sum_len = 0.0f;
    int count = 0;
    for (const auto& seg : ridges.segments) {
        if (seg.points.size() < 2) {
            continue;
        }
        const auto& a = seg.points.front();
        const auto& b = seg.points.back();
        const float dx = b.x - a.x;
        const float dy = b.y - a.y;
        const float len = std::sqrt(dx * dx + dy * dy);
        if (len < 3.0f) {
            continue;
        }
        sum_angle += std::atan2(dy, dx);
        sum_len += len;
        count += 1;
    }

    if (count == 0) {
        result.pattern = DermatoglyphicPattern::Unknown;
        return result;
    }

    const float mean_angle = sum_angle / static_cast<float>(count);
    const float ridge_freq = static_cast<float>(count) / std::max(1.0f, sum_len) * 100.0f;
    result.ridge_frequency = ridge_freq;
    result.quality = std::min(0.95f, static_cast<float>(count) / 40.0f);

    const float ang = std::abs(mean_angle);
    if (ang < 0.35f || ang > 2.8f) {
        result.pattern = DermatoglyphicPattern::Arch;
    } else if (ridge_freq > 1.2f) {
        result.pattern = DermatoglyphicPattern::Whorl;
    } else {
        result.pattern = DermatoglyphicPattern::Loop;
    }
    return result;
}

} // namespace CurvEngine::domain
