#include "curv/domain/CreaseChainLinker.hpp"
#include <gtest/gtest.h>
#include <cmath>

using namespace CurvEngine;
using namespace CurvEngine::domain;

TEST(CreaseChainLinkerTest, EmptyInputReturnsEmpty) {
    std::vector<CurveSegment> const empty;
    auto const res = CreaseChainLinker::linkSegments(empty);
    EXPECT_TRUE(res.empty());
}

TEST(CreaseChainLinkerTest, LinksCollinearBrokenSegments) {
    // Segment 1: (10, 100) -> (30, 100) -> (50, 100)
    CurveSegment seg1;
    seg1.id = 1;
    seg1.points = {Point2D(10.0f, 100.0f), Point2D(30.0f, 100.0f), Point2D(50.0f, 100.0f)};
    seg1.total_length = 40.0f;

    // Segment 2: (65, 100) -> (85, 100) -> (105, 100) [Gap of 15px along same line]
    CurveSegment seg2;
    seg2.id = 2;
    seg2.points = {Point2D(65.0f, 100.0f), Point2D(85.0f, 100.0f), Point2D(105.0f, 100.0f)};
    seg2.total_length = 40.0f;

    CreaseLinkOptions opts;
    opts.max_gap_distance = 25.0f;
    opts.min_collinearity = 0.80f;
    opts.spline_step = 5.0f;

    auto const chains = CreaseChainLinker::linkSegments({seg1, seg2}, opts);
    ASSERT_EQ(chains.size(), 1);
    EXPECT_TRUE(chains[0].is_valid);
    EXPECT_EQ(chains[0].constituent_segment_ids.size(), 2);

    // Continuous spline should span from ~10 to ~105
    EXPECT_NEAR(chains[0].points.front().x, 10.0f, 2.0f);
    EXPECT_NEAR(chains[0].points.back().x, 105.0f, 2.0f);
    EXPECT_GT(chains[0].points.size(), 15);
}

TEST(CreaseChainLinkerTest, DoesNotLinkDistantOrPerpendicularSegments) {
    // Horizontal segment
    CurveSegment seg1;
    seg1.id = 1;
    seg1.points = {Point2D(10.0f, 100.0f), Point2D(50.0f, 100.0f)};
    seg1.total_length = 40.0f;

    // Perpendicular vertical segment
    CurveSegment seg2;
    seg2.id = 2;
    seg2.points = {Point2D(60.0f, 100.0f), Point2D(60.0f, 140.0f)};
    seg2.total_length = 40.0f;

    CreaseLinkOptions opts;
    opts.max_gap_distance = 25.0f;
    opts.min_collinearity = 0.85f;

    auto const chains = CreaseChainLinker::linkSegments({seg1, seg2}, opts);
    // Should remain two separate chains because of 90-degree angle
    EXPECT_EQ(chains.size(), 2);
}

TEST(CreaseChainLinkerTest, CatmullRomSplineInterpolation) {
    std::vector<Point2D> control_points = {
        Point2D(0.0f, 0.0f),
        Point2D(20.0f, 10.0f),
        Point2D(40.0f, 0.0f),
        Point2D(60.0f, -10.0f),
        Point2D(80.0f, 0.0f)
    };

    auto const smooth = CreaseChainLinker::fitCatmullRomSpline(control_points, 2.0f);
    EXPECT_GT(smooth.size(), 35);
    EXPECT_NEAR(smooth.front().x, 0.0f, 1e-3f);
    EXPECT_NEAR(smooth.front().y, 0.0f, 1e-3f);
    EXPECT_NEAR(smooth.back().x, 80.0f, 1e-3f);
    EXPECT_NEAR(smooth.back().y, 0.0f, 1e-3f);
}

TEST(CreaseChainLinkerTest, ExtractMajorLinesFromSyntheticGraph) {
    RidgeGraph graph;

    // Heart line fragments (y ~ 150)
    CurveSegment h1;
    h1.id = 101;
    h1.points = {Point2D(150.0f, 150.0f), Point2D(250.0f, 155.0f)};
    h1.total_length = 100.0f;
    graph.segments.push_back(h1);

    CurveSegment h2;
    h2.id = 102;
    h2.points = {Point2D(265.0f, 156.0f), Point2D(380.0f, 160.0f)};
    h2.total_length = 115.0f;
    graph.segments.push_back(h2);

    // Life line fragments (arc around x~180, y~220-380)
    CurveSegment l1;
    l1.id = 201;
    l1.points = {Point2D(160.0f, 220.0f), Point2D(180.0f, 280.0f)};
    l1.total_length = 65.0f;
    graph.segments.push_back(l1);

    CurveSegment l2;
    l2.id = 202;
    l2.points = {Point2D(185.0f, 295.0f), Point2D(210.0f, 380.0f)};
    l2.total_length = 90.0f;
    graph.segments.push_back(l2);

    auto const major_lines = CreaseChainLinker::extractMajorLines(graph, false, 512);

    EXPECT_TRUE(major_lines.contains("heart_line"));
    EXPECT_TRUE(major_lines.at("heart_line").is_valid);
    EXPECT_GT(major_lines.at("heart_line").points.size(), 20);

    EXPECT_TRUE(major_lines.contains("life_line"));
    EXPECT_TRUE(major_lines.at("life_line").is_valid);
    EXPECT_GT(major_lines.at("life_line").points.size(), 20);
}
