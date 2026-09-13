#include "curv/domain/FingertipPatternAnalyzer.hpp"

#include <gtest/gtest.h>

TEST(FingertipPatternAnalyzer, EmptyGraphReturnsUnknown) {
    CurvEngine::RidgeGraph graph;
    const auto result = CurvEngine::domain::FingertipPatternAnalyzer::classify(graph, "index");
    EXPECT_EQ(result.digit, "index");
    EXPECT_EQ(result.quality, 0.0f);
}

TEST(FingertipPatternAnalyzer, HorizontalRidgesClassifyAsArch) {
    CurvEngine::RidgeGraph graph;
    CurvEngine::CurveSegment seg;
    seg.id = 1;
    for (int i = 0; i < 20; ++i) {
        seg.points.emplace_back(static_cast<float>(i * 3), 10.0f, 0.5f, 0.9f);
    }
    seg.total_length = 60.0f;
    graph.segments.push_back(seg);
    const auto result = CurvEngine::domain::FingertipPatternAnalyzer::classify(graph, "index");
    EXPECT_GT(result.quality, 0.0f);
}
