#include <gtest/gtest.h>

#include "curv/Evidence.hpp"
#include "curv/GeometryTypes.hpp"

namespace {

using CurvEngine::CurveSegment;
using CurvEngine::Junction;
using CurvEngine::Point2D;
using CurvEngine::RidgeGraph;

TEST(GeometryTypes, Point2DStoresPositionIntensityAndConfidence) {
    const Point2D pt{10.5f, 20.25f, 0.85f, 0.99f};
    EXPECT_FLOAT_EQ(pt.x, 10.5f);
    EXPECT_FLOAT_EQ(pt.y, 20.25f);
    EXPECT_FLOAT_EQ(pt.intensity, 0.85f);
    EXPECT_FLOAT_EQ(pt.confidence, 0.99f);
}

TEST(GeometryTypes, Point2DDefaultsAreSane) {
    const Point2D pt;
    EXPECT_FLOAT_EQ(pt.x, 0.0f);
    EXPECT_FLOAT_EQ(pt.y, 0.0f);
    EXPECT_FLOAT_EQ(pt.intensity, 0.0f);
    EXPECT_FLOAT_EQ(pt.confidence, 1.0f);
}

TEST(GeometryTypes, CurveSegmentHoldsOrderedPointsAndMetrics) {
    CurveSegment seg;
    seg.id = 1;
    seg.points.emplace_back(10.5f, 20.25f, 0.85f, 0.99f);
    seg.total_length = 15.0f;
    seg.average_curvature = 0.05f;
    seg.is_closed = false;

    EXPECT_EQ(seg.id, 1);
    ASSERT_EQ(seg.points.size(), 1u);
    EXPECT_FLOAT_EQ(seg.points[0].confidence, 0.99f);
    EXPECT_FLOAT_EQ(seg.total_length, 15.0f);
    EXPECT_FLOAT_EQ(seg.average_curvature, 0.05f);
    EXPECT_FALSE(seg.is_closed);
}

TEST(GeometryTypes, JunctionConnectsMultipleSegments) {
    Junction junc;
    junc.position = Point2D{10.5f, 20.25f, 0.85f, 0.99f};
    junc.connected_segment_ids = {1, 2};
    junc.branch_angle = 1.57f;

    EXPECT_FLOAT_EQ(junc.position.x, 10.5f);
    ASSERT_EQ(junc.connected_segment_ids.size(), 2u);
    EXPECT_EQ(junc.connected_segment_ids[0], 1);
    EXPECT_EQ(junc.connected_segment_ids[1], 2);
    EXPECT_FLOAT_EQ(junc.branch_angle, 1.57f);
}

TEST(GeometryTypes, RidgeGraphAggregatesSegmentsAndJunctions) {
    RidgeGraph graph;

    CurveSegment seg;
    seg.id = 1;
    seg.points.emplace_back(1.0f, 2.0f, 0.5f, 0.9f);
    graph.segments.push_back(seg);

    Junction junc;
    junc.position = Point2D{1.0f, 2.0f, 0.5f, 0.9f};
    junc.connected_segment_ids = {1};
    graph.junctions.push_back(junc);

    ASSERT_EQ(graph.segments.size(), 1u);
    ASSERT_EQ(graph.junctions.size(), 1u);
    EXPECT_EQ(graph.segments[0].id, 1);
    EXPECT_EQ(graph.junctions[0].connected_segment_ids[0], 1);
    EXPECT_TRUE(graph.debug_mask.empty());
}

} // namespace

// === Step 4 coverage-gap tests (characterization) ===

TEST(EvidenceContractTest, Test_DefaultEvidenceSerializesNullPayloadsAsObjects) {
    // A default-constructed Evidence carries null JSON members; to_json must
    // normalize both to empty objects so downstream consumers see a stable shape.
    const CurvEngine::Evidence ev;
    EXPECT_TRUE(ev.evidence_id.empty());
    EXPECT_TRUE(ev.type.empty());
    EXPECT_NEAR(ev.confidence, 1.0, 1e-9);

    const auto doc = ev.to_json();
    EXPECT_TRUE(doc["geometry"].is_object());
    EXPECT_TRUE(doc["geometry"].empty());
    EXPECT_TRUE(doc["metadata"].is_object());
    EXPECT_TRUE(doc["metadata"].empty());
    EXPECT_TRUE(doc["evidence_id"].is_string());
    EXPECT_TRUE(doc["confidence"].is_number());
}
