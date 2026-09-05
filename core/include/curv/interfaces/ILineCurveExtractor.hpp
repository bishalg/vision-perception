#pragma once

#include "curv/Evidence.hpp"
#include "curv/Frame.hpp"
#include <vector>

namespace CurvEngine::interfaces {

/**
 * @brief Abstract interface for extracting curvilinear structures and ridges as perceptual evidence.
 */
class ILineCurveExtractor {
public:
    virtual ~ILineCurveExtractor() = default;

    [[nodiscard]] virtual std::vector<Evidence> extract(const Frame& frame) = 0;
};

} // namespace CurvEngine::interfaces
