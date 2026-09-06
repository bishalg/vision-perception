#include <iostream>

#include "curv/tools/demo_lib.hpp"

int main() {
    try {
        return CurvEngine::tools::runDesktopDemo();
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unhandled exception: " << e.what() << '\n';
        return 1;
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception" << '\n';
        return 1;
    }
}
