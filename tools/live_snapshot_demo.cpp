#include <iostream>

#include "curv/tools/live_snapshot_demo_lib.hpp"

int main(int argc, char* argv[]) {
    try {
        return CurvEngine::tools::runLiveSnapshotDemo(argc, argv, std::cout, std::cerr);
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unhandled exception: " << e.what() << '\n';
        return 1;
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception" << '\n';
        return 1;
    }
}
