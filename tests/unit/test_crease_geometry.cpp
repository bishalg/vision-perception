#include "curv/domain/CreaseGeometry.hpp"
#include "curv/domain/PalmZones.hpp"
#include <gtest/gtest.h>
#include <cmath>

using namespace CurvEngine;
using namespace CurvEngine::domain;

TEST(CreaseGeometryTest, ArcLengthAndChord_StraightLine) {
    std::vector<Point2D> pts = {
        Point2D(10.0f, 20.0f),
        Point2D(40.0f, 20.0f),
        Point2D(70.0f, 20.0f),
        Point2D(110.0f, 20.0f)
    };

    const float arc = CreaseGeometry::computeArcLength(pts);
    const float chord = CreaseGeometry::computeChordLength(pts);
    const float tortuosity = CreaseGeometry::computeTortuosity(arc, chord);
    const float angle = CreaseGeometry::computeAngleDeg(pts.front(), pts.back());

    EXPECT_NEAR(arc, 100.0f, 1e-3f);
    EXPECT_NEAR(chord, 100.0f, 1e-3f);
    EXPECT_NEAR(tortuosity, 1.0f, 1e-3f);
    EXPECT_NEAR(angle, 0.0f, 1e-3f);
}

TEST(CreaseGeometryTest, Tortuosity_CurvedArc) {
    // Semi-circle of radius R = 50: arc length = pi * R ~ 157.08, chord = 2 * R = 100
    // Expected tortuosity = 157.08 / 100 = 1.5708
    std::vector<Point2D> pts;
    const int num_steps = 50;
    for (int i = 0; i <= num_steps; ++i) {
        const float theta = 3.14159265f * (static_cast<float>(i) / static_cast<float>(num_steps));
        pts.emplace_back(50.0f - 50.0f * std::cos(theta), 50.0f * std::sin(theta));
    }

    const float arc = CreaseGeometry::computeArcLength(pts);
    const float chord = CreaseGeometry::computeChordLength(pts);
    const float tortuosity = CreaseGeometry::computeTortuosity(arc, chord);

    EXPECT_NEAR(chord, 100.0f, 1e-2f);
    EXPECT_NEAR(arc, 50.0f * 3.14159265f, 0.5f);
    EXPECT_NEAR(tortuosity, 3.14159265f / 2.0f, 1e-2f);
    EXPECT_GT(tortuosity, 1.50f);
}

TEST(CreaseGeometryTest, AngleDeg_Calculations) {
    // Horizontal
    EXPECT_NEAR(CreaseGeometry::computeAngleDeg(Point2D(0.0f, 0.0f), Point2D(100.0f, 0.0f)), 0.0f, 1e-3f);
    // Vertical
    EXPECT_NEAR(CreaseGeometry::computeAngleDeg(Point2D(0.0f, 0.0f), Point2D(0.0f, 100.0f)), 90.0f, 1e-3f);
    // 45 degrees diagonal
    EXPECT_NEAR(CreaseGeometry::computeAngleDeg(Point2D(0.0f, 0.0f), Point2D(50.0f, 50.0f)), 45.0f, 1e-3f);
}

TEST(CreaseGeometryTest, MinDistance_ParallelLines) {
    std::vector<Point2D> line1 = {Point2D(0.0f, 50.0f), Point2D(100.0f, 50.0f)};
    std::vector<Point2D> line2 = {Point2D(0.0f, 85.0f), Point2D(100.0f, 85.0f)};

    const float min_d = CreaseGeometry::computeMinDistance(line1, line2);
    EXPECT_NEAR(min_d, 35.0f, 1e-3f);
}

TEST(CreaseGeometryTest, Intersections_CrossingSegments) {
    // Cross shape intersecting at (50, 50)
    std::vector<Point2D> h_line = {Point2D(0.0f, 50.0f), Point2D(100.0f, 50.0f)};
    std::vector<Point2D> v_line = {Point2D(50.0f, 0.0f), Point2D(50.0f, 100.0f)};
    std::vector<Point2D> parallel = {Point2D(0.0f, 10.0f), Point2D(100.0f, 10.0f)};

    EXPECT_EQ(CreaseGeometry::countIntersections(h_line, v_line), 1);
    EXPECT_EQ(CreaseGeometry::countIntersections(h_line, parallel), 0);
}

TEST(CreaseGeometryTest, ForkCount_MatchesJunctions) {
    std::vector<Junction> junctions;
    Junction j1;
    j1.connected_segment_ids = {10, 11, 12};
    junctions.push_back(j1);

    Junction j2;
    j2.connected_segment_ids = {20, 21};
    junctions.push_back(j2);

    Junction j3;
    j3.connected_segment_ids = {30, 31};
    junctions.push_back(j3);

    std::vector<int> chain_ids = {10, 15};
    EXPECT_EQ(CreaseGeometry::countForks(chain_ids, junctions), 1);

    std::vector<int> multi_ids = {10, 20};
    EXPECT_EQ(CreaseGeometry::countForks(multi_ids, junctions), 2);

    std::vector<int> no_ids = {99};
    EXPECT_EQ(CreaseGeometry::countForks(no_ids, junctions), 0);
}

TEST(CreaseGeometryTest, ComputeMetrics_FullChain) {
    ChainedCreaseLine chain;
    chain.name = "heart_line";
    chain.constituent_segment_ids = {101, 102};
    chain.total_length = 150.0f;
    chain.points = {Point2D(50.0f, 100.0f), Point2D(120.0f, 105.0f), Point2D(190.0f, 100.0f)};
    chain.is_valid = true;

    RidgeGraph graph;
    CurveSegment seg1;
    seg1.id = 101;
    seg1.points = {Point2D(50.0f, 100.0f, 180.0f), Point2D(120.0f, 105.0f, 170.0f)};
    seg1.total_length = 70.0f;
    graph.segments.push_back(seg1);

    CurveSegment seg2;
    seg2.id = 102;
    seg2.points = {Point2D(120.0f, 105.0f, 160.0f), Point2D(190.0f, 100.0f, 150.0f)};
    seg2.total_length = 80.0f;
    graph.segments.push_back(seg2);

    auto const zones = PalmZones::getCanonicalPalmZones(512, false);
    auto const metrics = CreaseGeometry::computeMetrics(chain, graph, zones);

    EXPECT_NEAR(metrics.arc_length_px, 150.0f, 1e-2f);
    EXPECT_NEAR(metrics.chord_length_px, 140.0f, 1e-2f);
    EXPECT_GT(metrics.tortuosity, 1.0f);
    EXPECT_EQ(metrics.gap_count, 1); // 2 constituent segments = 1 gap bridged
    EXPECT_GT(metrics.mean_intensity, 100.0f);
}
