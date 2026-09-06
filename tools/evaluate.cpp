#include <iostream>

#include "curv/Version.hpp"
#include "curv/tools/evaluate_lib.hpp"

int main(int argc, char* argv[]) {
    std::cout << "CurvEngine Extraction Accuracy Evaluator (v" << CurvEngine::getVersion() << ")\n";
    try {
        return CurvEngine::tools::runEvaluator(argc, argv, std::cout, std::cerr);
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unhandled exception: " << e.what() << '\n';
        return 1;
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception\n";
        return 1;
    }
}
