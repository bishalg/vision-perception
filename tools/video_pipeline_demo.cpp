#include <iostream>

#include "curv/tools/video_pipeline_demo_lib.hpp"

int main(int argc, char* argv[]) {
    try {
        return CurvEngine::tools::runVideoPipelineDemo(argc, argv, std::cout, std::cerr);
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unhandled exception: " << e.what() << '\n';
        return 1;
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception" << '\n';
        return 1;
    }
}
