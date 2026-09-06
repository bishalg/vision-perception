#pragma once

#include <opencv2/core.hpp>
#include <string>
#include <utility>
#include <vector>

namespace CurvEngine::tools {

/**
 * @brief Options for the sequential-vs-parallel extraction throughput benchmark.
 */
struct BenchmarkOptions {
    int iterations{5};
    double sigma{1.5f};
    /// Resolution table; empty -> built-in 720p/1080p defaults.
    std::vector<std::pair<std::string, cv::Size>> resolutions{};
};

/**
 * @brief Times Steger extraction single-threaded vs. cv::parallel_for_ on synthetic
 *        horizontal-ridge scenes and writes a formatted report to out.
 */
void runThroughputBenchmark(std::ostream& out, const BenchmarkOptions& opts = BenchmarkOptions{});

} // namespace CurvEngine::tools
