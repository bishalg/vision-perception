#include <iostream>

#include "curv/tools/palm_inspect_lib.hpp"

int main(int argc, char* argv[]) {
    try {
        return CurvEngine::tools::runPalmOverlayDebug(argc, argv, std::cout, std::cerr);
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unhandled exception: " << e.what() << '\n';
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception" << '\n';
    }
    return 1;
}
