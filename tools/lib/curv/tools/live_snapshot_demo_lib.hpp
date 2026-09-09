#pragma once

#include <ostream>

namespace CurvEngine::tools {

/**
 * @brief Phase 3 demo: poll HTTP/mock CCTV snapshots and print live quality HUD.
 */
int runLiveSnapshotDemo(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
