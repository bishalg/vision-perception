#pragma once

#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"

namespace CurvEngine::interfaces {

/**
 * @brief Abstract interface for evaluating input frame quality (focus, exposure, contrast).
 */
class IQualityAnalyzer {
public:
    virtual ~IQualityAnalyzer() = default;

    [[nodiscard]] virtual QualityReport analyze(const Frame& frame) = 0;
};

} // namespace CurvEngine::interfaces
