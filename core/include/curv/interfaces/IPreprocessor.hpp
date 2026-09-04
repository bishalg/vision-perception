#pragma once

#include "curv/Frame.hpp"

namespace CurvEngine::interfaces {

/**
 * @brief Abstract interface for geometric and radiometric frame preprocessing.
 */
class IPreprocessor {
public:
    virtual ~IPreprocessor() = default;

    [[nodiscard]] virtual Frame process(const Frame& frame) = 0;
};

} // namespace CurvEngine::interfaces
