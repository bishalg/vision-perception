#include <gtest/gtest.h>

#include "curv/domain/CreaseGeometry.hpp"
#include "curv/domain/FateLineAnalyzer.hpp"

using namespace CurvEngine;
using namespace CurvEngine::domain;

namespace {

ChainedCreaseLine makeLine(const std::string& name, int id, const std::vector<Point2D>& pts, float conf = 0.8F) {
    ChainedCreaseLine line;
    line.name = name;
    line.constituent_segment_ids = {id};
    line.points = pts;
    line.total_length = CreaseGeometry::computeArcLength(pts);
    line.confidence = conf;
    line.is_valid = true;
    return line;
}

std::vector<Point2D> vertical(float x, float y0, float y1, float step = 10.0F) {
    std::vector<Point2D> pts;
    for (float y = y0; y <= y1; y += step) {
        pts.push_back(Point2D{x, y});
    }
    return pts;
}

} // namespace

TEST(FateLineAnalyzerTest, AbsentIsValidNoFate) {
    ChainedCreaseLine fate;
    fate.is_valid = false;
    RidgeGraph graph;
    const FateReport rep = FateLineAnalyzer::analyze(fate, nullptr, nullptr, nullptr, graph, 512);
    EXPECT_FALSE(rep.present);
    EXPECT_TRUE(rep.no_fate);
    EXPECT_FALSE(rep.broken);
}

TEST(FateLineAnalyzerTest, BrokenFlagFromGaps) {
    ChainedCreaseLine fate = makeLine("fate_line", 7, vertical(256.0F, 200.0F, 400.0F));
    fate.constituent_segment_ids = {7, 8};
    RidgeGraph graph;
    const FateReport rep = FateLineAnalyzer::analyze(fate, nullptr, nullptr, nullptr, graph, 512);
    EXPECT_TRUE(rep.present);
    EXPECT_EQ(rep.gap_count, 1);
    EXPECT_TRUE(rep.broken);
    EXPECT_GT(rep.age_at_break, 0.0F);
}

TEST(FateLineAnalyzerTest, StopsAtHeadAndAge35) {
    // Head horizontal at y=300; fate vertical crossing it.
    std::vector<Point2D> head_pts = {Point2D{100.0F, 300.0F}, Point2D{400.0F, 300.0F}};
    std::vector<Point2D> heart_pts = {Point2D{100.0F, 150.0F}, Point2D{400.0F, 150.0F}};
    ChainedCreaseLine head = makeLine("head_line", 20, head_pts);
    ChainedCreaseLine heart = makeLine("heart_line", 10, heart_pts);
    ChainedCreaseLine fate = makeLine("fate_line", 30, vertical(256.0F, 100.0F, 400.0F));
    RidgeGraph graph;
    const FateReport rep = FateLineAnalyzer::analyze(fate, &heart, &head, nullptr, graph, 512);
    EXPECT_TRUE(rep.stops_at_head);
    EXPECT_TRUE(rep.stops_at_heart);
    // End y=400/512=0.78 below head (~0.59): age should be <35, break age set.
    EXPECT_GT(rep.age_at_end, 15.0F);
    EXPECT_LT(rep.age_at_end, 80.0F);
}

TEST(FateLineAnalyzerTest, DoubleLineParallel) {
    ChainedCreaseLine fate = makeLine("fate_line", 30, vertical(256.0F, 200.0F, 400.0F));
    RidgeGraph graph;
    CurveSegment other;
    other.id = 31;
    other.total_length = 200.0F;
    for (float y = 200.0F; y <= 400.0F; y += 10.0F) {
        other.points.emplace_back(270.0F, y);
    }
    graph.segments = {other};
    const FateReport rep = FateLineAnalyzer::analyze(fate, nullptr, nullptr, nullptr, graph, 512);
    EXPECT_TRUE(rep.double_line);
}

TEST(FateLineAnalyzerTest, OriginMoonVsLifeJoint) {
    // Life near thenar; fate starting on it -> life_joint.
    std::vector<Point2D> life_pts = {Point2D{150.0F, 400.0F}, Point2D{160.0F, 450.0F}};
    ChainedCreaseLine life = makeLine("life_line", 40, life_pts);
    std::vector<Point2D> fate_pts = {Point2D{152.0F, 402.0F}, Point2D{256.0F, 200.0F}};
    ChainedCreaseLine fate = makeLine("fate_line", 30, fate_pts);
    RidgeGraph graph;
    const FateReport rep = FateLineAnalyzer::analyze(fate, nullptr, nullptr, &life, graph, 512);
    EXPECT_TRUE(rep.joined_life_start);
    EXPECT_EQ(rep.origin, "life_joint");
}

TEST(FateLineAnalyzerTest, AgeMappingAnchors) {
    // head y=0.6, heart y=0.3 in norm space.
    EXPECT_NEAR(FateLineAnalyzer::ageAtY(0.6F, 0.6F, 0.3F), 35.0F, 1.0F);
    EXPECT_NEAR(FateLineAnalyzer::ageAtY(0.3F, 0.6F, 0.3F), 53.0F, 1.0F);
    const float mid = FateLineAnalyzer::ageAtY(0.45F, 0.6F, 0.3F);
    EXPECT_GT(mid, 35.0F);
    EXPECT_LT(mid, 53.0F);
}
