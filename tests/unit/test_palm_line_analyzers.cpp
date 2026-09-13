#include <gtest/gtest.h>

#include "curv/domain/CreaseGeometry.hpp"
#include "curv/domain/HeartHeadAnalyzer.hpp"
#include "curv/domain/LifeLineAnalyzer.hpp"
#include "curv/domain/LineCaseTable.hpp"
#include "curv/domain/MountMarkDetector.hpp"
#include "curv/domain/PalmZones.hpp"

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

std::vector<Point2D> horizontal(float y, float x0, float x1, float step = 10.0F) {
    std::vector<Point2D> pts;
    for (float x = x0; x <= x1; x += step) {
        pts.push_back(Point2D{x, y});
    }
    return pts;
}

std::vector<Point2D> vertical(float x, float y0, float y1, float step = 10.0F) {
    std::vector<Point2D> pts;
    for (float y = y0; y <= y1; y += step) {
        pts.push_back(Point2D{x, y});
    }
    return pts;
}

} // namespace

TEST(LifeLineAnalyzerTest, AbsentLineIsNotPresent) {
    ChainedCreaseLine life;
    life.is_valid = false;
    const LifeReport rep = LifeLineAnalyzer::analyze(life, nullptr, nullptr, 512);
    EXPECT_FALSE(rep.present);
    EXPECT_FLOAT_EQ(rep.age_at_end, 0.0F);
    EXPECT_FALSE(rep.age_calibrated);
}

TEST(LifeLineAnalyzerTest, AgeAtLifeArcEndpointsAndMonotonic) {
    EXPECT_FLOAT_EQ(LifeLineAnalyzer::ageAtLifeArc(0.0F), 0.0F);
    EXPECT_FLOAT_EQ(LifeLineAnalyzer::ageAtLifeArc(1.0F), 90.0F);
    EXPECT_LT(LifeLineAnalyzer::ageAtLifeArc(0.25F), LifeLineAnalyzer::ageAtLifeArc(0.75F));
    // Head anchor remaps through 35 at the anchor fraction.
    EXPECT_FLOAT_EQ(LifeLineAnalyzer::ageAtLifeArc(0.5F, 0.5F), 35.0F);
    EXPECT_GT(LifeLineAnalyzer::ageAtLifeArc(0.9F, 0.5F), 35.0F);
}

TEST(LifeLineAnalyzerTest, PresentLifeGetsAgeAndLengthClass) {
    // Long C arc: 400px tall in 512 space -> is_long via 0.70 ratio.
    ChainedCreaseLine life = makeLine("life_line", 3, vertical(150.0F, 100.0F, 480.0F));
    const LifeReport rep = LifeLineAnalyzer::analyze(life, nullptr, nullptr, 512);
    EXPECT_TRUE(rep.present);
    EXPECT_TRUE(rep.is_long);
    EXPECT_FALSE(rep.is_short);
    EXPECT_FLOAT_EQ(rep.age_at_end, 90.0F);
}

TEST(HeartHeadAnalyzerTest, AbsentLinesAreNotPresent) {
    ChainedCreaseLine heart;
    heart.is_valid = false;
    ChainedCreaseLine head;
    head.is_valid = false;
    EXPECT_FALSE(HeartLineAnalyzer::analyze(heart, nullptr, nullptr, 512, 400.0F).present);
    EXPECT_FALSE(HeadLineAnalyzer::analyze(head, nullptr, nullptr, 512, 400.0F).present);
}

TEST(HeartHeadAnalyzerTest, SimianMergeDetectedBothSides) {
    // Heart and head share the same transverse polyline -> fused.
    const auto pts = horizontal(200.0F, 100.0F, 400.0F);
    ChainedCreaseLine heart = makeLine("heart_line", 10, pts);
    ChainedCreaseLine head = makeLine("head_line", 20, pts);
    const HeartReport hr = HeartLineAnalyzer::analyze(heart, &head, nullptr, 512, 400.0F);
    const HeadReport dr = HeadLineAnalyzer::analyze(head, &heart, nullptr, 512, 400.0F);
    EXPECT_TRUE(hr.simian_merge);
    EXPECT_TRUE(dr.merged_heart);
}

TEST(HeartHeadAnalyzerTest, HeadJoinedLifeOrigin) {
    ChainedCreaseLine life = makeLine("life_line", 3, vertical(120.0F, 150.0F, 450.0F));
    std::vector<Point2D> head_pts = {Point2D{122.0F, 152.0F}, Point2D{300.0F, 180.0F}};
    ChainedCreaseLine head = makeLine("head_line", 20, head_pts);
    const HeadReport dr = HeadLineAnalyzer::analyze(head, nullptr, &life, 512, 400.0F);
    EXPECT_TRUE(dr.joined_life_origin);
}

TEST(HeartHeadAnalyzerTest, HeartUsesWidthBasis) {
    // 300px heart on 400px palm width: 0.75 -> medium (between 0.55 and 0.80).
    ChainedCreaseLine heart = makeLine("heart_line", 10, horizontal(200.0F, 100.0F, 400.0F));
    const HeartReport rep = HeartLineAnalyzer::analyze(heart, nullptr, nullptr, 512, 400.0F);
    EXPECT_TRUE(rep.present);
    EXPECT_FALSE(rep.is_short);
    EXPECT_FALSE(rep.is_long);
}

TEST(LineCaseTableTest, TablesCoverThreeLinesWithP0Implemented) {
    int p0_count = 0;
    for (const auto& table : {HeartLineCases(), HeadLineCases(), LifeLineCases()}) {
        EXPECT_FALSE(table.empty());
        for (const auto& row : table) {
            EXPECT_FALSE(row.id.empty());
            EXPECT_FALSE(row.primitive.empty());
            if (row.priority == 0) {
                EXPECT_TRUE(row.implemented) << row.id << " P0 must be implemented";
                ++p0_count;
            }
            if (!row.implemented) {
                EXPECT_NE(row.classical_note.find("RESEARCH NEEDED"), std::string::npos) << row.id;
            }
        }
    }
    EXPECT_GE(p0_count, 3);
}

TEST(MountMarkDetectorTest, EmptyGraphHasNoBadMounts) {
    RidgeGraph graph;
    const auto zones = domains::PalmZones::getCanonicalPalmZones(512, false);
    const MountMarks marks = MountMarkDetector::detect(graph, zones);
    EXPECT_FALSE(marks.saturn_bad);
    EXPECT_FALSE(marks.mercury_bad);
}

TEST(MountMarkDetectorTest, JunctionClusterFlagsSaturnGrillProxy) {
    // Six junctions inside the Saturn zone (200..300 x, 80..200 y) trip the
    // grill-density proxy without any detected pattern.
    RidgeGraph graph;
    for (int i = 0; i < 6; ++i) {
        Junction j;
        j.position = Point2D{220.0F + static_cast<float>(i) * 10.0F, 120.0F};
        graph.junctions.push_back(j);
    }
    const auto zones = domains::PalmZones::getCanonicalPalmZones(512, false);
    const MountMarks marks = MountMarkDetector::detect(graph, zones);
    EXPECT_TRUE(marks.saturn_bad);
    EXPECT_FALSE(marks.mercury_bad);
}
