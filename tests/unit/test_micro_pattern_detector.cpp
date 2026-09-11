#include <gtest/gtest.h>
#include "curv/pattern/MicroPatternDetector.hpp"

using namespace CurvEngine;
using namespace CurvEngine::pattern;

TEST(MicroPatternDetectorTest, DetectsCross) {
    RidgeGraph graph;
    // Single junction with degree 4
    Junction j;
    j.position = {50.0f, 50.0f};
    j.connected_segment_ids = {1, 2, 3, 4};
    graph.junctions.push_back(j);

    auto crosses = MicroPatternDetector::findCrosses(graph);
    ASSERT_EQ(crosses.size(), 1u);
    EXPECT_EQ(crosses[0].type, PatternType::Cross);
    EXPECT_NEAR(crosses[0].centroid.x, 50.0f, 1e-3f);
    EXPECT_NEAR(crosses[0].centroid.y, 50.0f, 1e-3f);
}

TEST(MicroPatternDetectorTest, DetectsStar) {
    RidgeGraph graph;
    // Junction with degree 6 (multi-ray star)
    Junction j;
    j.position = {30.0f, 40.0f};
    j.connected_segment_ids = {1, 2, 3, 4, 5, 6};
    graph.junctions.push_back(j);

    auto stars = MicroPatternDetector::findStars(graph);
    ASSERT_EQ(stars.size(), 1u);
    EXPECT_EQ(stars[0].type, PatternType::Star);
    EXPECT_NEAR(stars[0].centroid.x, 30.0f, 1e-3f);
    EXPECT_NEAR(stars[0].centroid.y, 40.0f, 1e-3f);
}

TEST(MicroPatternDetectorTest, DetectsIsland) {
    RidgeGraph graph;
    // Two junctions sharing two segments (1 and 2)
    Junction j1;
    j1.position = {10.0f, 10.0f};
    j1.connected_segment_ids = {0, 1, 2};

    Junction j2;
    j2.position = {30.0f, 10.0f};
    j2.connected_segment_ids = {1, 2, 3};

    graph.junctions.push_back(j1);
    graph.junctions.push_back(j2);

    auto islands = MicroPatternDetector::findIslands(graph);
    ASSERT_EQ(islands.size(), 1u);
    EXPECT_EQ(islands[0].type, PatternType::Island);
    EXPECT_NEAR(islands[0].centroid.x, 20.0f, 1e-3f);
    EXPECT_NEAR(islands[0].centroid.y, 10.0f, 1e-3f);
}

TEST(MicroPatternDetectorTest, DetectsTriangleCycle) {
    RidgeGraph graph;
    // 3 segments 1, 2, 3 forming a triangle via 3 junctions
    Junction j1;
    j1.position = {0.0f, 0.0f};
    j1.connected_segment_ids = {1, 2};

    Junction j2;
    j2.position = {10.0f, 0.0f};
    j2.connected_segment_ids = {2, 3};

    Junction j3;
    j3.position = {5.0f, 10.0f};
    j3.connected_segment_ids = {3, 1};

    graph.junctions.push_back(j1);
    graph.junctions.push_back(j2);
    graph.junctions.push_back(j3);

    auto triangles = MicroPatternDetector::findTriangles(graph);
    ASSERT_EQ(triangles.size(), 1u);
    EXPECT_EQ(triangles[0].type, PatternType::Triangle);
    EXPECT_NEAR(triangles[0].centroid.x, 5.0f, 1e-3f);
}
