#pragma once

#include <string>

namespace CurvEngine {

/**
 * @brief Returns the semver string of the CurvEngine core library.
 */
[[nodiscard]] std::string getVersion();

} // namespace CurvEngine
