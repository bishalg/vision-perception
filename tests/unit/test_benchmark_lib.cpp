#include <sstream>

#include <gtest/gtest.h>

#include "curv/tools/benchmark_lib.hpp"

namespace {

using CurvEngine::tools::runThroughputBenchmark;

TEST(BenchmarkLibTest, ReportsSequentialAndParallelTimings) {
    std::stringstream out;
    CurvEngine::tools::BenchmarkOptions opts;
    opts.iterations = 1;
    opts.resolutions = {{"Tiny", {64, 64}}};
    runThroughputBenchmark(out, opts);
    const auto text = out.str();
    EXPECT_NE(text.find("Sequential"), std::string::npos);
    EXPECT_NE(text.find("Multi-threaded"), std::string::npos);
    EXPECT_NE(text.find("Tiny"), std::string::npos);
    EXPECT_NE(text.find("Speedup"), std::string::npos);
    EXPECT_NE(text.find("ms"), std::string::npos);
}

TEST(BenchmarkLibTest, UsesDefaultResolutionsWhenUnset) {
    std::stringstream out;
    CurvEngine::tools::BenchmarkOptions opts;
    opts.iterations = 1;
    opts.resolutions.clear(); // signal: use built-in 720p/1080p table
    runThroughputBenchmark(out, opts);
    EXPECT_NE(out.str().find("720p HD"), std::string::npos);
    EXPECT_NE(out.str().find("1080p FHD"), std::string::npos);
}

} // namespace
