#include <gtest/gtest.h>
#include "curv/geometry/ZoneIntersector.hpp"
#include "curv/domain/PalmZones.hpp"

using namespace CurvEngine;
using namespace CurvEngine::geometry;
using namespace CurvEngine::domains;

TEST(ZoneIntersectorTest, Test_PointInZone) {
    Zone box;
    box.id = "test_box";
    box.polygon = {
        Point2D{100.0f, 100.0f},
        Point2D{200.0f, 100.0f},
        Point2D{200.0f, 200.0f},
        Point2D{100.0f, 200.0f}
    };

    EXPECT_TRUE(ZoneIntersector::isPointInZone(Point2D{150.0f, 150.0f}, box));
    EXPECT_TRUE(ZoneIntersector::isPointInZone(Point2D{100.0f, 100.0f}, box)); // on boundary
    EXPECT_FALSE(ZoneIntersector::isPointInZone(Point2D{50.0f, 50.0f}, box));
    EXPECT_FALSE(ZoneIntersector::isPointInZone(Point2D{250.0f, 150.0f}, box));
}

TEST(ZoneIntersectorTest, Test_FindPrimaryZone) {
    Zone zone_a;
    zone_a.id = "zone_alpha";
    zone_a.polygon = {
        Point2D{0.0f, 0.0f},
        Point2D{100.0f, 0.0f},
        Point2D{100.0f, 100.0f},
        Point2D{0.0f, 100.0f}
    };

    Zone zone_b;
    zone_b.id = "zone_beta";
    zone_b.polygon = {
        Point2D{200.0f, 0.0f},
        Point2D{300.0f, 0.0f},
        Point2D{300.0f, 100.0f},
        Point2D{200.0f, 100.0f}
    };

    std::vector<Zone> zones = {zone_a, zone_b};

    // Segment with 80% points in zone_alpha
    CurveSegment seg_a;
    seg_a.id = 1;
    for (int x = 20; x <= 100; x += 10) {
        seg_a.points.emplace_back(static_cast<float>(x), 50.0f);
    }
    // and 2 points outside
    seg_a.points.emplace_back(120.0f, 50.0f);
    seg_a.points.emplace_back(140.0f, 50.0f);

    EXPECT_EQ(ZoneIntersector::findPrimaryZone(seg_a, zones), "zone_alpha");

    // Segment completely outside
    CurveSegment seg_out;
    seg_out.id = 2;
    seg_out.points.emplace_back(500.0f, 500.0f);
    seg_out.points.emplace_back(510.0f, 510.0f);

    EXPECT_EQ(ZoneIntersector::findPrimaryZone(seg_out, zones), "unknown");
}

TEST(ZoneIntersectorTest, Test_PalmZones_CanonicalMapping) {
    const auto zones = PalmZones::getCanonicalPalmZones(512);
    EXPECT_GE(zones.size(), 8);

    // Test a curve across the Jupiter Mount (under index finger: x ~ 150, y ~ 160)
    CurveSegment jupiter_curve;
    jupiter_curve.id = 10;
    for (int x = 120; x <= 180; x += 10) {
        jupiter_curve.points.emplace_back(static_cast<float>(x), 160.0f);
    }

    const std::string primary = ZoneIntersector::findPrimaryZone(jupiter_curve, zones);
    EXPECT_EQ(primary, "jupiter_zone");
}

// === Step 4 coverage-gap tests (characterization) ===

TEST(ZoneIntersectorTest, Test_DegeneratePolygonsAreNeverContaining) {
    Zone empty_zone;
    EXPECT_FALSE(ZoneIntersector::isPointInZone(Point2D{150.0f, 150.0f}, empty_zone));

    Zone one_point;
    one_point.polygon = {Point2D{150.0f, 150.0f}};
    EXPECT_FALSE(ZoneIntersector::isPointInZone(Point2D{150.0f, 150.0f}, one_point));

    Zone two_points;
    two_points.polygon = {Point2D{0.0f, 0.0f}, Point2D{100.0f, 100.0f}};
    EXPECT_FALSE(ZoneIntersector::isPointInZone(Point2D{50.0f, 50.0f}, two_points));
}

TEST(ZoneIntersectorTest, Test_FindPrimaryZone_EdgeCases) {
    Zone zone;
    zone.id = "zone";
    zone.polygon = {Point2D{0.0f, 0.0f}, Point2D{100.0f, 0.0f}, Point2D{100.0f, 100.0f},
                    Point2D{0.0f, 100.0f}};

    CurveSegment empty_segment;
    empty_segment.id = 1;
    EXPECT_EQ(ZoneIntersector::findPrimaryZone(empty_segment, {zone}), "unknown");

    CurveSegment seg;
    seg.id = 2;
    seg.points.emplace_back(50.0f, 50.0f);
    EXPECT_EQ(ZoneIntersector::findPrimaryZone(seg, {}), "unknown");

    // A demanding overlap ratio that no zone satisfies yields "unknown".
    std::vector<Zone> zones = {zone};
    CurveSegment mixed;
    mixed.id = 3;
    mixed.points.emplace_back(50.0f, 50.0f);
    mixed.points.emplace_back(500.0f, 500.0f);
    EXPECT_EQ(ZoneIntersector::findPrimaryZone(mixed, zones, 2.0f), "unknown");
}

TEST(ZoneIntersectorTest, Test_ComputeZoneOverlap_EdgeCases) {
    CurveSegment empty_segment;
    EXPECT_TRUE(ZoneIntersector::computeZoneOverlap(empty_segment, {}).empty());

    Zone zone;
    zone.id = "zone";
    zone.polygon = {Point2D{0.0f, 0.0f}, Point2D{100.0f, 0.0f}, Point2D{100.0f, 100.0f},
                    Point2D{0.0f, 100.0f}};

    CurveSegment seg;
    seg.id = 1;
    seg.points.emplace_back(50.0f, 50.0f);

    // Zones with degenerate polygons are skipped entirely.
    Zone degenerate;
    degenerate.id = "degenerate";
    degenerate.polygon = {Point2D{50.0f, 50.0f}};
    const auto overlaps = ZoneIntersector::computeZoneOverlap(seg, {degenerate, zone});
    ASSERT_EQ(overlaps.size(), 1u);
    EXPECT_EQ(overlaps[0].first, "zone");
    EXPECT_NEAR(overlaps[0].second, 1.0f, 1e-6);

    // Non-empty segment with empty zones returns empty
    EXPECT_TRUE(ZoneIntersector::computeZoneOverlap(seg, {}).empty());
}
