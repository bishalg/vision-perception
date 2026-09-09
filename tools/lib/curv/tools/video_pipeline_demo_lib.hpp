#pragma once

#include <ostream>

namespace CurvEngine::tools {

/**
 * @brief Phase 2 demo: recorded CCTV video → quality + Steger → annotated output video.
 * @return 0 on success, non-zero on failure.
 */
int runVideoPipelineDemo(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
