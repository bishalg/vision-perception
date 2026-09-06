#include "curv/tools/benchmark_lib.hpp"

#include <chrono>
#include <iomanip>
#include <opencv2/imgproc.hpp>
#include <vector>

#include "curv/ridge/StegerRidgeExtractor.hpp"

namespace CurvEngine::tools {

void runThroughputBenchmark(std::ostream& out, const BenchmarkOptions& opts) {
    out << "\n==================================================" << '\n';
    out << "CurvEngine Performance Benchmark (cv::parallel_for_)" << '\n';
    out << "Platform Threads: " << cv::getNumThreads() << '\n';
    out << "==================================================" << '\n';

    ridge::StegerConfig cfg;
    cfg.sigma = static_cast<float>(opts.sigma);
    cfg.low_threshold = 0.5f;
    cfg.high_threshold = 1.5f;
    cfg.min_segment_length = 5.0f;
    const ridge::StegerRidgeExtractor extractor(cfg);

    const std::vector<std::pair<std::string, cv::Size>> resolutions =
        opts.resolutions.empty() ? std::vector<std::pair<std::string, cv::Size>>{{"720p HD", cv::Size(1280, 720)},
                                                                                 {"1080p FHD", cv::Size(1920, 1080)},
                                                                                 {"4K UHD", cv::Size(3840, 2160)}}
                                 : opts.resolutions;

    for (const auto& [name, sz] : resolutions) {
        cv::Mat synth(sz, CV_8UC1, cv::Scalar(30));
        for (int r = 100; r < sz.height - 100; r += 120) {
            cv::ellipse(synth, cv::Point(sz.width / 2, r), cv::Size(sz.width / 3, 60), 0, 0, 180, cv::Scalar(220), 3,
                        cv::LINE_AA);
        }

        // Warmup run
        cv::setNumThreads(0);
        (void)extractor.extractRidgeGraph(synth);

        const int iterations = std::max(1, opts.iterations);

        // 1. Sequential execution (single-threaded)
        cv::setNumThreads(1);
        auto t0 = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < iterations; ++i) {
            (void)extractor.extractRidgeGraph(synth);
        }
        auto t1 = std::chrono::high_resolution_clock::now();
        const double seq_ms = std::chrono::duration<double, std::milli>(t1 - t0).count() / iterations;

        // 2. Parallel execution (cv::parallel_for_)
        cv::setNumThreads(0);
        t0 = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < iterations; ++i) {
            (void)extractor.extractRidgeGraph(synth);
        }
        t1 = std::chrono::high_resolution_clock::now();
        const double par_ms = std::chrono::duration<double, std::milli>(t1 - t0).count() / iterations;

        const double speedup = seq_ms / std::max(1e-6, par_ms);

        out << "\nResolution: " << name << " (" << sz.width << "x" << sz.height << ")" << '\n';
        out << "  - Sequential (1 thread):         " << seq_ms << " ms" << '\n';
        out << "  - Multi-threaded (parallel_for): " << par_ms << " ms" << '\n';
        out << "  - Speedup Factor:                " << speedup << "x (" << (1000.0 / par_ms) << " FPS)" << '\n';
    }
    out << "\n==================================================\n" << '\n';
}

} // namespace CurvEngine::tools
