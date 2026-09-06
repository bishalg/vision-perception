#include <iostream>

#include "curv/Version.hpp"
#include "curv/tools/synth_lib.hpp"

int main(int argc, char* argv[]) {
    std::cout << "CurvEngine Synthetic Ground-Truth Generator (v" << CurvEngine::getVersion() << ")\n";
    try {
        return CurvEngine::tools::runSynthGenerator(argc, argv, std::cout, std::cerr);
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unhandled exception: " << e.what() << '\n';
        return 1;
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception\n";
        return 1;
    }
}
