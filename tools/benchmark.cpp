#include <iostream>

#include "curv/Version.hpp"
#include "curv/tools/benchmark_lib.hpp"

int main(int argc, char* argv[]) {
    try {
        std::cout << "CurvEngine Benchmark Tool (v" << CurvEngine::getVersion() << ")\n";
        CurvEngine::tools::BenchmarkOptions opts;
        for (int i = 1; i < argc; ++i) {
            const std::string arg = argv[i];
            if (arg == "--iterations" && i + 1 < argc) {
                opts.iterations = std::max(1, std::stoi(argv[++i]));
            } else if (arg == "--sigma" && i + 1 < argc) {
                opts.sigma = std::stod(argv[++i]);
            } else {
                std::cout << "Usage: " << argv[0] << " [--iterations n] [--sigma s]\n";
                return 1;
            }
        }
        CurvEngine::tools::runThroughputBenchmark(std::cout, opts);
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] " << e.what() << '\n';
        return 1;
    } catch (...) {
        std::cerr << "[ERROR] Unhandled unknown exception\n";
        return 1;
    }
}
