#pragma once

#include <ostream>

namespace CurvEngine::tools {

/**
 * @brief CLI runner for the perception pipeline verification tool (curv_cli).
 * @return process-style exit code (0 = success, 1 = IO failure).
 */
int runCurvCli(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
