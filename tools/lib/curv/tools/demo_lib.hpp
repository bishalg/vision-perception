#pragma once

namespace CurvEngine::tools {

/**
 * @brief Runs the desktop pipeline demo sequence (banner, frame ingest,
 *        quality report, evidence serialization via spdlog).
 * @return process-style exit code (0 = success).
 */
int runDesktopDemo();

} // namespace CurvEngine::tools
