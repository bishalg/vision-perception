#include <gtest/gtest.h>
#include "curv/geometry/CurveChunker.hpp"
#include <cmath>

using namespace CurvEngine;
using namespace CurvEngine::geometry;

TEST(CurveChunkerTest, EmptyOrSinglePointReturnsEmpty) {
    std::vector<Point2D> empty_pts;
    auto res_empty = CurveChunker::divideUniform(empty_pts, 5, 0.0f, 100.0f);
    EXPECT_TRUE(res_empty.chunks.empty());
    EXPECT_FLOAT_EQ(res_empty.total_arc_length, 0.0f);

    std::vector<Point2D> single_pt = {{10.0f, 10.0f}};
    auto res_single = CurveChunker::divideUniform(single_pt, 5, 0.0f, 100.0f);
    EXPECT_TRUE(res_single.chunks.empty());
    EXPECT_FLOAT_EQ(res_single.total_arc_length, 0.0f);
}

TEST(CurveChunkerTest, UniformDivisionStraightLine) {
    // Horizontal line from (0, 0) to (100, 0)
    std::vector<Point2D> pts = {
        {0.0f, 0.0f},
        {50.0f, 0.0f},
        {100.0f, 0.0f}
    };

    const int num_chunks = 4;
    auto partitioned = CurveChunker::divideUniform(pts, num_chunks, 0.0f, 100.0f);

    EXPECT_NEAR(partitioned.total_arc_length, 100.0f, 1e-3f);
    ASSERT_EQ(partitioned.chunks.size(), 4u);

    // Check chunks
    for (size_t i = 0; i < partitioned.chunks.size(); ++i) {
        const auto& c = partitioned.chunks[i];
        EXPECT_EQ(c.index, static_cast<int>(i));
        EXPECT_NEAR(c.arc_length_px, 25.0f, 1e-2f);
        float fi = static_cast<float>(i);
        EXPECT_NEAR(c.s_start, fi * 0.25f, 1e-3f);
        EXPECT_NEAR(c.s_end, (fi + 1.0f) * 0.25f, 1e-3f);

        // Value progression
        float expected_val = (fi + 0.5f) * 25.0f;
        EXPECT_NEAR(c.scalar_value, expected_val, 1e-2f);

        // Tangent should be (1, 0), normal should be orthogonal
        EXPECT_NEAR(c.tangent.x, 1.0f, 1e-3f);
        EXPECT_NEAR(c.tangent.y, 0.0f, 1e-3f);
        EXPECT_NEAR(c.normal.x * c.tangent.x + c.normal.y * c.tangent.y, 0.0f, 1e-3f);
    }
}

TEST(CurveChunkerTest, SampleAtInterpolation) {
    std::vector<Point2D> pts = {
        {0.0f, 0.0f},
        {100.0f, 100.0f}
    };

    auto partitioned = CurveChunker::divideUniform(pts, 2, 0.0f, 10.0f);
    EXPECT_NEAR(partitioned.total_arc_length, std::sqrt(20000.0f), 1e-2f);

    Point2D mid = partitioned.sampleAt(0.5f);
    EXPECT_NEAR(mid.x, 50.0f, 1e-2f);
    EXPECT_NEAR(mid.y, 50.0f, 1e-2f);

    Point2D quarter = partitioned.sampleAt(0.25f);
    EXPECT_NEAR(quarter.x, 25.0f, 1e-2f);
    EXPECT_NEAR(quarter.y, 25.0f, 1e-2f);
}

TEST(CurveChunkerTest, DivideByAnchors) {
    // Vertical line y from 0 to 100
    std::vector<Point2D> pts = {
        {50.0f, 0.0f},
        {50.0f, 50.0f},
        {50.0f, 100.0f}
    };

    // Anchors: s=0.0 -> age 10, s=0.5 -> age 35, s=1.0 -> age 70
    std::vector<CalibrationAnchor> anchors = {
        {0.0f, 10.0f},
        {0.5f, 35.0f},
        {1.0f, 70.0f}
    };

    auto partitioned = CurveChunker::divideByAnchors(pts, anchors, /*sub_chunks=*/2);
    // 2 intervals * 2 sub_chunks = 4 chunks
    ASSERT_EQ(partitioned.chunks.size(), 4u);

    // Chunk 0: s in [0.0, 0.25] -> age in [10, 22.5] -> mid ~16.25
    EXPECT_NEAR(partitioned.chunks[0].s_start, 0.0f, 1e-3f);
    EXPECT_NEAR(partitioned.chunks[0].s_end, 0.25f, 1e-3f);
    EXPECT_NEAR(partitioned.chunks[0].scalar_value, 16.25f, 0.5f);

    // Chunk 1: s in [0.25, 0.5] -> age in [22.5, 35] -> mid ~28.75
    EXPECT_NEAR(partitioned.chunks[1].scalar_value, 28.75f, 0.5f);

    // Chunk 2: s in [0.5, 0.75] -> age in [35, 52.5] -> mid ~43.75
    EXPECT_NEAR(partitioned.chunks[2].scalar_value, 43.75f, 0.5f);

    // Chunk 3: s in [0.75, 1.0] -> age in [52.5, 70] -> mid ~61.25
    EXPECT_NEAR(partitioned.chunks[3].scalar_value, 61.25f, 0.5f);
}
