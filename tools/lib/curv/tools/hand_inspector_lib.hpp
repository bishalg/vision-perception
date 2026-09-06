#pragma once

#include <ostream>

namespace CurvEngine::tools {

/**
 * @brief CLI runner for the hand inspector & VLM grounding tool (hand_inspector_cli).
 * @return process-style exit code (0 = success, 1 = IO failure).
 */
int runHandInspector(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
