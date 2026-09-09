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
    EXPECT_TRUE(major_lines.contains("life_line"));
    EXPECT_TRUE(major_lines.at("life_line").is_valid);
}

TEST(CreaseChainLinkerTest, LandmarkPriorsPreferHeartShallowerThanHead) {
    RidgeGraph graph;

    // Shallower transverse (heart band)
    CurveSegment heart_seg;
    heart_seg.id = 1;
    heart_seg.points = {Point2D(120.0f, 180.0f), Point2D(260.0f, 185.0f), Point2D(400.0f, 190.0f)};
    heart_seg.total_length = 280.0f;
    graph.segments.push_back(heart_seg);

    // Deeper transverse (head band)
    CurveSegment head_seg;
    head_seg.id = 2;
    head_seg.points = {Point2D(140.0f, 260.0f), Point2D(270.0f, 270.0f), Point2D(390.0f, 275.0f)};
    head_seg.total_length = 250.0f;
    graph.segments.push_back(head_seg);

    MajorLineLandmarks priors;
    priors.valid = true;
    priors.landmarks.is_normalized = false;
    // Wrist near bottom, MCP row near top of palm.
    priors.landmarks.points[0] = {256.0f, 420.0f}; // wrist
    priors.landmarks.points[2] = {180.0f, 300.0f}; // thumb MCP
    priors.landmarks.points[5] = {180.0f, 140.0f}; // index MCP
    priors.landmarks.points[9] = {256.0f, 130.0f}; // middle MCP
    priors.landmarks.points[13] = {320.0f, 140.0f};
    priors.landmarks.points[17] = {380.0f, 155.0f};

    auto const major = CreaseChainLinker::extractMajorLines(graph, false, 512, priors);
    ASSERT_TRUE(major.contains("heart_line"));
    ASSERT_TRUE(major.contains("head_line"));
    EXPECT_TRUE(major.at("heart_line").is_valid);
    EXPECT_TRUE(major.at("head_line").is_valid);

    // Exclusive: different segment sets
    EXPECT_NE(major.at("heart_line").constituent_segment_ids.front(),
              major.at("head_line").constituent_segment_ids.front());
}

TEST(CreaseChainLinkerTest, FilterGraphByPalmDiscRemovesOutsideSegments) {
    RidgeGraph graph;
    CurveSegment inside;
    inside.id = 1;
    inside.points = {Point2D(250.0f, 300.0f), Point2D(260.0f, 310.0f)};
    inside.total_length = 14.0f;
    graph.segments.push_back(inside);

    CurveSegment outside;
    outside.id = 2;
    outside.points = {Point2D(10.0f, 10.0f), Point2D(20.0f, 15.0f)};
    outside.total_length = 12.0f;
    graph.segments.push_back(outside);

    const std::vector<Point2D> disc = {
        Point2D(200.0f, 200.0f), Point2D(320.0f, 200.0f), Point2D(320.0f, 400.0f), Point2D(200.0f, 400.0f)};

    const size_t removed = CreaseChainLinker::filterGraphByPalmDisc(graph, disc, 0.5f);
    EXPECT_EQ(removed, 1u);
    ASSERT_EQ(graph.segments.size(), 1u);
    EXPECT_EQ(graph.segments.front().id, 1);
}

TEST(CreaseChainLinkerTest, LinksBidirectionallyFromMidSeed) {
    // Longest seed is in the middle; growth must extend both left and right.
    CurveSegment left;
    left.id = 1;
    left.points = {Point2D(10.0f, 100.0f), Point2D(40.0f, 100.0f)};
    left.total_length = 30.0f;

    CurveSegment mid;
    mid.id = 2;
    mid.points = {Point2D(55.0f, 100.0f), Point2D(120.0f, 100.0f)};
    mid.total_length = 65.0f;

    CurveSegment right;
    right.id = 3;
    right.points = {Point2D(135.0f, 100.0f), Point2D(180.0f, 100.0f)};
    right.total_length = 45.0f;

    CreaseLinkOptions opts;
    opts.max_gap_distance = 25.0f;
    opts.min_collinearity = 0.80f;
    opts.spline_step = 5.0f;

    auto const chains = CreaseChainLinker::linkSegments({left, mid, right}, opts);
    ASSERT_EQ(chains.size(), 1u);
    EXPECT_EQ(chains[0].constituent_segment_ids.size(), 3u);
    EXPECT_NEAR(chains[0].points.front().x, 10.0f, 3.0f);
    EXPECT_NEAR(chains[0].points.back().x, 180.0f, 3.0f);
}

TEST(CreaseChainLinkerTest, LifeDoesNotClaimShallowTransverse) {
    RidgeGraph graph;

    CurveSegment heartish;
    heartish.id = 1;
    heartish.points = {Point2D(140.0f, 170.0f), Point2D(260.0f, 175.0f), Point2D(390.0f, 180.0f)};
    heartish.total_length = 250.0f;
    graph.segments.push_back(heartish);

    CurveSegment life;
    life.id = 2;
    life.points = {Point2D(170.0f, 200.0f), Point2D(185.0f, 280.0f), Point2D(205.0f, 380.0f)};
    life.total_length = 190.0f;
    graph.segments.push_back(life);

    MajorLineLandmarks priors;
    priors.valid = true;
    priors.landmarks.is_normalized = false;
    priors.landmarks.points[0] = {256.0f, 420.0f};
    priors.landmarks.points[2] = {180.0f, 300.0f};
    priors.landmarks.points[5] = {180.0f, 140.0f};
    priors.landmarks.points[9] = {256.0f, 130.0f};
    priors.landmarks.points[13] = {320.0f, 140.0f};
    priors.landmarks.points[17] = {380.0f, 155.0f};

    auto const major = CreaseChainLinker::extractMajorLines(graph, false, 512, priors);
    ASSERT_TRUE(major.contains("heart_line"));
    ASSERT_TRUE(major.contains("life_line"));
    EXPECT_EQ(major.at("heart_line").constituent_segment_ids.front(), 1);
    EXPECT_EQ(major.at("life_line").constituent_segment_ids.front(), 2);
}

TEST(CreaseChainLinkerTest, FateGateRejectsShortWeakChain) {
    RidgeGraph graph;

    CurveSegment stub;
    stub.id = 9;
    stub.points = {Point2D(250.0f, 300.0f), Point2D(252.0f, 325.0f)};
    stub.total_length = 25.0f;
    graph.segments.push_back(stub);

    MajorLineLandmarks priors;
    priors.valid = true;
    priors.landmarks.is_normalized = false;
    priors.landmarks.points[0] = {256.0f, 420.0f};
    priors.landmarks.points[2] = {180.0f, 300.0f};
    priors.landmarks.points[5] = {180.0f, 140.0f};
    priors.landmarks.points[9] = {256.0f, 130.0f};
    priors.landmarks.points[13] = {320.0f, 140.0f};
    priors.landmarks.points[17] = {380.0f, 155.0f};

    auto const major = CreaseChainLinker::extractMajorLines(graph, false, 512, priors);
    EXPECT_FALSE(major.contains("fate_line"));
}
