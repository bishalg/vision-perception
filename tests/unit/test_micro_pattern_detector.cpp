#include <gtest/gtest.h>
#include "curv/pattern/MicroPatternDetector.hpp"
#include "curv/pattern/PatternValidation.hpp"
#include "curv/pattern/HostAttribution.hpp"
#include "curv/geometry/JunctionDeriver.hpp"

#include <algorithm>
#include <cmath>
#include <random>
#include <vector>

using namespace CurvEngine;
using namespace CurvEngine::pattern;
using CurvEngine::geometry::deriveJunctions;

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

// ---------------------------------------------------------------------------
// Geometry-validated detection (P2): fixtures, soft filters, false-positive gate
// ---------------------------------------------------------------------------

namespace {

constexpr float kTestPalmWidth = 282.0F; // canonical 512 patch => 0.55 * 512

Point2D P(float x, float y) { return Point2D{x, y, 0.0F, 1.0F}; }

CurveSegment makeSeg(int id, std::vector<Point2D> pts) {
    CurveSegment seg;
    seg.id = id;
    seg.points = std::move(pts);
    float length = 0.0F;
    for (size_t i = 0; i + 1 < seg.points.size(); ++i) {
        const float dx = seg.points[i + 1].x - seg.points[i].x;
        const float dy = seg.points[i + 1].y - seg.points[i].y;
        length += std::sqrt(dx * dx + dy * dy);
    }
    seg.total_length = length;
    return seg;
}

Junction makeJunc(float x, float y, std::vector<int> ids) {
    Junction j;
    j.position = P(x, y);
    j.connected_segment_ids = std::move(ids);
    return j;
}

MicroSignConfig cfgFor(float palm_width = kTestPalmWidth) {
    MicroSignConfig cfg;
    cfg.scale.palm_width_px = palm_width;
    return cfg;
}

int countType(const std::vector<DetectedPattern>& patterns, PatternType type) {
    int n = 0;
    for (const auto& p : patterns) {
        if (p.type == type) {
            ++n;
        }
    }
    return n;
}

std::vector<Point2D> arcPoints(float x0, float y0, float x1, float y1, float bulge, bool upward, int steps = 8) {
    std::vector<Point2D> pts;
    for (int i = 0; i <= steps; ++i) {
        const float t = static_cast<float>(i) / static_cast<float>(steps);
        const float x = x0 + (x1 - x0) * t;
        const float y = y0 + (y1 - y0) * t + ((upward ? -1.0F : 1.0F) * bulge * std::sin(3.14159265F * t));
        pts.push_back(P(x, y));
    }
    return pts;
}

// Island: theta loop between (100,100) and (124,100), bulge 4, plus approach/exit lines.
RidgeGraph islandGraph(float scale = 1.0F) {
    RidgeGraph g;
    g.segments.push_back(makeSeg(0, {P(84 * scale, 100 * scale), P(100 * scale, 100 * scale)}));
    g.segments.push_back(makeSeg(1, arcPoints(100 * scale, 100 * scale, 124 * scale, 100 * scale, 4 * scale, true)));
    g.segments.push_back(makeSeg(2, arcPoints(100 * scale, 100 * scale, 124 * scale, 100 * scale, 4 * scale, false)));
    g.segments.push_back(makeSeg(3, {P(124 * scale, 100 * scale), P(140 * scale, 100 * scale)}));
    g.junctions.push_back(makeJunc(100 * scale, 100 * scale, {0, 1, 2}));
    g.junctions.push_back(makeJunc(124 * scale, 100 * scale, {1, 2, 3}));
    return g;
}

// Isosceles triangle with base 30 and height 26 (per vertex_scale).
RidgeGraph triangleGraph(float vertex_scale = 1.0F) {
    const float ax = 100.0F;
    const float ay = 100.0F;
    const float bx = ax + 30.0F * vertex_scale;
    const float by = ay;
    const float cx = ax + 15.0F * vertex_scale;
    const float cy = ay + 26.0F * vertex_scale;
    RidgeGraph g;
    g.segments.push_back(makeSeg(10, {P(ax, ay), P(bx, by)}));
    g.segments.push_back(makeSeg(11, {P(bx, by), P(cx, cy)}));
    g.segments.push_back(makeSeg(12, {P(cx, cy), P(ax, ay)}));
    g.junctions.push_back(makeJunc(ax, ay, {10, 12}));
    g.junctions.push_back(makeJunc(bx, by, {10, 11}));
    g.junctions.push_back(makeJunc(cx, cy, {11, 12}));
    return g;
}

// Two straight strokes crossing at (110,110); second stroke rotated by stroke2_deg.
RidgeGraph crossGraph(float stroke2_deg, float arm = 20.0F) {
    RidgeGraph g;
    const float cx = 110.0F;
    const float cy = 110.0F;
    const float rad = stroke2_deg * 3.14159265F / 180.0F;
    const float dx2 = std::cos(rad);
    const float dy2 = std::sin(rad);
    g.segments.push_back(makeSeg(0, {P(cx - arm, cy), P(cx, cy)}));
    g.segments.push_back(makeSeg(1, {P(cx, cy), P(cx + arm, cy)}));
    g.segments.push_back(makeSeg(2, {P(cx - arm * dx2, cy - arm * dy2), P(cx, cy)}));
    g.segments.push_back(makeSeg(3, {P(cx, cy), P(cx + arm * dx2, cy + arm * dy2)}));
    g.junctions.push_back(makeJunc(cx, cy, {0, 1, 2, 3}));
    return g;
}

RidgeGraph starGraph(int arm_count, float arm_len = 15.0F) {
    RidgeGraph g;
    const float cx = 110.0F;
    const float cy = 110.0F;
    std::vector<int> ids;
    for (int i = 0; i < arm_count; ++i) {
        const float a = 2.0F * 3.14159265F * static_cast<float>(i) / static_cast<float>(arm_count);
        g.segments.push_back(makeSeg(100 + i, {P(cx, cy), P(cx + arm_len * std::cos(a), cy + arm_len * std::sin(a))}));
        ids.push_back(100 + i);
    }
    g.junctions.push_back(makeJunc(cx, cy, ids));
    return g;
}

RidgeGraph tridentGraph() {
    RidgeGraph g;
    const float cx = 110.0F;
    const float cy = 120.0F;
    g.segments.push_back(makeSeg(0, {P(cx, cy), P(cx - 12.0F, cy - 22.0F)}));
    g.segments.push_back(makeSeg(1, {P(cx, cy), P(cx, cy - 25.0F)}));
    g.segments.push_back(makeSeg(2, {P(cx, cy), P(cx + 12.0F, cy - 22.0F)}));
    g.junctions.push_back(makeJunc(cx, cy, {0, 1, 2}));
    return g;
}

// 2x2 grid of crossing lines; with_diagonal adds a cell diagonal (grid cells must be
// suppressed as nets, not reported as triangles/squares).
RidgeGraph gridGraph(bool with_diagonal) {
    RidgeGraph g;
    int next_id = 0;
    const float h_y[2] = {100.0F, 120.0F};
    const float v_x[2] = {100.0F, 120.0F};
    for (int i = 0; i < 2; ++i) {
        g.segments.push_back(makeSeg(next_id++, {P(90.0F, h_y[i]), P(v_x[0], h_y[i])}));
        g.segments.push_back(makeSeg(next_id++, {P(v_x[0], h_y[i]), P(v_x[1], h_y[i])}));
        g.segments.push_back(makeSeg(next_id++, {P(v_x[1], h_y[i]), P(130.0F, h_y[i])}));
    }
    for (int i = 0; i < 2; ++i) {
        g.segments.push_back(makeSeg(next_id++, {P(v_x[i], 90.0F), P(v_x[i], h_y[0])}));
        g.segments.push_back(makeSeg(next_id++, {P(v_x[i], h_y[0]), P(v_x[i], h_y[1])}));
        g.segments.push_back(makeSeg(next_id++, {P(v_x[i], h_y[1]), P(v_x[i], 130.0F)}));
    }
    int diag_id = -1;
    if (with_diagonal) {
        diag_id = next_id++;
        g.segments.push_back(makeSeg(diag_id, {P(v_x[0], h_y[0]), P(v_x[1], h_y[1])}));
    }
    for (int hx = 0; hx < 2; ++hx) {
        for (int vy = 0; vy < 2; ++vy) {
            const int h_mid = 1 + hx * 3;
            const int v_mid = 7 + vy * 3;
            // Split-line junction: 4 adjacent pieces (h-left, h-mid, v-top, v-mid).
            std::vector<int> ids = {h_mid - 1, h_mid, v_mid - 1, v_mid};
            if (with_diagonal && ((hx == 0 && vy == 0) || (hx == 1 && vy == 1))) {
                ids.push_back(diag_id);
            }
            g.junctions.push_back(makeJunc(v_x[vy], h_y[hx], ids));
        }
    }
    return g;
}

// Island body plus a degree-4 tail junction within fish-tail range of the centroid.
RidgeGraph fishGraph() {
    RidgeGraph g = islandGraph();
    const float tx = 148.0F;
    const float ty = 105.0F;
    g.segments.push_back(makeSeg(4, {P(tx, ty), P(tx + 14.0F, ty - 8.0F)}));
    g.segments.push_back(makeSeg(5, {P(tx, ty), P(tx + 16.0F, ty + 4.0F)}));
    g.segments.push_back(makeSeg(6, {P(tx, ty), P(tx + 6.0F, ty + 16.0F)}));
    g.segments.push_back(makeSeg(7, {P(tx, ty), P(tx - 10.0F, ty + 14.0F)}));
    g.junctions.push_back(makeJunc(tx, ty, {4, 5, 6, 7}));
    return g;
}

// Straight-line arrangement graph: every pair of lines is split at its crossing, junctions
// carry the 4 adjacent piece ids. Mimics crossing creases for the false-positive gate.
RidgeGraph lineArrangementGraph(const std::vector<std::pair<Point2D, Point2D>>& lines) {
    RidgeGraph g;
    const size_t n = lines.size();
    std::vector<std::vector<std::pair<float, size_t>>> inter_sorted(n);
    for (size_t i = 0; i < n; ++i) {
        for (size_t j = i + 1; j < n; ++j) {
            const float dx1 = lines[i].second.x - lines[i].first.x;
            const float dy1 = lines[i].second.y - lines[i].first.y;
            const float dx2 = lines[j].second.x - lines[j].first.x;
            const float dy2 = lines[j].second.y - lines[j].first.y;
            const float den = dx1 * dy2 - dy1 * dx2;
            if (std::fabs(den) < 1e-6F) {
                continue;
            }
            const float t =
                ((lines[j].first.x - lines[i].first.x) * dy2 - (lines[j].first.y - lines[i].first.y) * dx2) / den;
            const float s =
                ((lines[j].first.x - lines[i].first.x) * dy1 - (lines[j].first.y - lines[i].first.y) * dx1) / den;
            if (t > 1e-3F && t < 1.0F - 1e-3F && s > 1e-3F && s < 1.0F - 1e-3F) {
                inter_sorted[i].emplace_back(t, j);
                inter_sorted[j].emplace_back(s, i);
            }
        }
    }
    for (auto& list : inter_sorted) {
        std::sort(list.begin(), list.end(),
                  [](const std::pair<float, size_t>& a, const std::pair<float, size_t>& b) { return a.first < b.first; });
    }

    int next_id = 0;
    std::vector<std::vector<int>> pieces(n);
    auto pointAt = [&](size_t i, float t) {
        return P(lines[i].first.x + (lines[i].second.x - lines[i].first.x) * t,
                 lines[i].first.y + (lines[i].second.y - lines[i].first.y) * t);
    };
    for (size_t i = 0; i < n; ++i) {
        std::vector<float> ts{0.0F};
        for (const auto& [t, j] : inter_sorted[i]) {
            (void)j;
            ts.push_back(t);
        }
        ts.push_back(1.0F);
        for (size_t k = 0; k + 1 < ts.size(); ++k) {
            g.segments.push_back(makeSeg(next_id, {pointAt(i, ts[k]), pointAt(i, ts[k + 1])}));
            pieces[i].push_back(next_id++);
        }
    }
    for (size_t i = 0; i < n; ++i) {
        for (size_t pos_i = 0; pos_i < inter_sorted[i].size(); ++pos_i) {
            const size_t j = inter_sorted[i][pos_i].second;
            size_t pos_j = 0;
            for (size_t k = 0; k < inter_sorted[j].size(); ++k) {
                if (inter_sorted[j][k].second == i) {
                    pos_j = k;
                    break;
                }
            }
            if (pos_i == 0 || pos_j == 0) {
                continue; // cannot happen for interior intersections, kept defensive
            }
            const Point2D cross_pt = pointAt(i, inter_sorted[i][pos_i].first);
            g.junctions.push_back(makeJunc(cross_pt.x, cross_pt.y, {pieces[i][pos_i - 1], pieces[i][pos_i],
                                                                    pieces[j][pos_j - 1], pieces[j][pos_j]}));
        }
    }
    return g;
}

} // namespace

// --- soft range / validation math ---

TEST(PatternValidationTest, SoftRangeScores) {
    EXPECT_NEAR(softRange(0.5F, 0.0F, 1.0F, 0.1F), 1.0F, 1e-6F);
    EXPECT_NEAR(softRange(1.05F, 0.0F, 1.0F, 0.1F), 0.5F, 1e-6F); // linear falloff
    EXPECT_NEAR(softRange(1.2F, 0.0F, 1.0F, 0.1F), 0.0F, 1e-6F);  // beyond margin
    EXPECT_NEAR(softRange(-0.05F, 0.0F, 1.0F, 0.1F), 0.5F, 1e-6F);
    EXPECT_NEAR(softRange(5.0F, 0.0F, 1.0F, 0.0F), 0.0F, 1e-6F); // zero margin safe
}

TEST(PatternValidationTest, LoopStatsShape) {
    // Axis-aligned rectangle 30x20 => area 600, perimeter 100, elongation 1.5.
    std::vector<Point2D> rect = {P(0, 0), P(30, 0), P(30, 20), P(0, 20)};
    const auto stats = computeLoopStats(rect);
    EXPECT_NEAR(stats.area_px, 600.0F, 1e-3F);
    EXPECT_NEAR(stats.perimeter_px, 100.0F, 1e-3F);
    EXPECT_NEAR(stats.circularity, 4.0F * 3.14159265F * 600.0F / (100.0F * 100.0F), 1e-3F);
    EXPECT_NEAR(stats.elongation, 1.5F, 0.05F);
}

TEST(PatternValidationTest, PatternTypeNameStable) {
    EXPECT_STREQ(patternTypeName(PatternType::Island), "Island");
    EXPECT_STREQ(patternTypeName(PatternType::Fish), "Fish");
    EXPECT_STREQ(patternTypeName(PatternType::SupportingLine), "SupportingLine");
}

// --- validated per-sign detection ---

TEST(MicroSignValidation, DetectsValidatedIsland) {
    const RidgeGraph g = islandGraph();
    auto islands = MicroPatternDetector::findIslands(g, cfgFor());
    ASSERT_EQ(islands.size(), 1u);
    EXPECT_GE(islands[0].confidence, 0.6F);
    EXPECT_NEAR(islands[0].centroid.x, 112.0F, 1.0F);
    // No other sign should fire on this fixture.
    const auto all = MicroPatternDetector::detectAll(g, cfgFor());
    EXPECT_EQ(countType(all, PatternType::Trident), 0);
    EXPECT_EQ(countType(all, PatternType::Cross), 0);
}

TEST(MicroSignValidation, IslandDegradesGracefullyWithoutGeometry) {
    // Junction-only graph (legacy fixture style): filters neutral, prior confidence kept.
    RidgeGraph g;
    Junction j1;
    j1.position = {10.0F, 10.0F};
    j1.connected_segment_ids = {0, 1, 2};
    Junction j2;
    j2.position = {30.0F, 10.0F};
    j2.connected_segment_ids = {1, 2, 3};
    g.junctions = {j1, j2};
    auto islands = MicroPatternDetector::findIslands(g, cfgFor());
    ASSERT_EQ(islands.size(), 1u);
    EXPECT_NEAR(islands[0].confidence, 0.95F, 1e-6F);
}

TEST(MicroSignValidation, RejectsOversizedTriangle) {
    // 3x vertex scale => 9x area: far outside the soft size range, dropped entirely.
    const RidgeGraph big = triangleGraph(3.0F);
    EXPECT_EQ(MicroPatternDetector::findTriangles(big, cfgFor()).size(), 0u);
    // 1x stays.
    const RidgeGraph ok = triangleGraph(1.0F);
    auto tris = MicroPatternDetector::findTriangles(ok, cfgFor());
    ASSERT_EQ(tris.size(), 1u);
    EXPECT_GE(tris[0].confidence, 0.6F);
}

TEST(MicroSignValidation, RejectsNeedleCross) {
    // 20-degree stroke separation is far outside [70,90]+20: dropped.
    EXPECT_EQ(MicroPatternDetector::findCrosses(crossGraph(20.0F), cfgFor()).size(), 0u);
    // 90-degree cross with 20px arms inside a 282px palm: kept.
    auto crosses = MicroPatternDetector::findCrosses(crossGraph(90.0F), cfgFor());
    ASSERT_EQ(crosses.size(), 1u);
    EXPECT_GE(crosses[0].confidence, 0.6F);
}

TEST(MicroSignValidation, RejectsOversizedCrossArms) {
    // 60px arms => 0.21 PW, far above the 0.08 PW upper bound: long crease crossings
    // must not become sign candidates.
    EXPECT_EQ(MicroPatternDetector::findCrosses(crossGraph(90.0F, 60.0F), cfgFor()).size(), 0u);
}

TEST(MicroSignValidation, DetectsStarAndDegradesFiveArm) {
    auto six = MicroPatternDetector::findStars(starGraph(6), cfgFor());
    ASSERT_EQ(six.size(), 1u);
    EXPECT_GE(six[0].confidence, 0.5F);
    auto five = MicroPatternDetector::findStars(starGraph(5), cfgFor());
    ASSERT_EQ(five.size(), 1u); // soft: below 6 arms still scores, just lower
    EXPECT_LT(five[0].confidence, six[0].confidence);
    EXPECT_GE(five[0].confidence, cfgFor().ranges.min_confidence);
}

TEST(MicroSignValidation, DetectsTridentFan) {
    auto tridents = MicroPatternDetector::findTridents(tridentGraph(), cfgFor());
    ASSERT_EQ(tridents.size(), 1u);
    EXPECT_GE(tridents[0].confidence, 0.6F);
}

TEST(MicroSignValidation, GridGraphEmitsNetOnly) {
    const RidgeGraph plain = gridGraph(false);
    const auto all_plain = MicroPatternDetector::detectAll(plain, cfgFor());
    EXPECT_EQ(countType(all_plain, PatternType::Square), 0); // grid cells are not signs
    EXPECT_EQ(countType(all_plain, PatternType::Triangle), 0);
    // The 4-way junctions of a grille are also geometric crosses (short arms); what
    // matters is that the cell itself is not reported as a square/triangle.
    EXPECT_LE(countType(all_plain, PatternType::Cross), 4);
    EXPECT_EQ(countType(all_plain, PatternType::Net), 1);

    const RidgeGraph diag = gridGraph(true);
    const auto all_diag = MicroPatternDetector::detectAll(diag, cfgFor());
    EXPECT_EQ(countType(all_diag, PatternType::Triangle), 0); // diagonal still a grid cell
    EXPECT_EQ(countType(all_diag, PatternType::Square), 0);
}

TEST(MicroSignValidation, DetectsFishWithTail) {
    const RidgeGraph g = fishGraph();
    auto fish = MicroPatternDetector::findFish(g, cfgFor());
    ASSERT_EQ(fish.size(), 1u);
    EXPECT_GE(fish[0].confidence, 0.5F);
    EXPECT_EQ(countType(MicroPatternDetector::detectAll(g, cfgFor()), PatternType::Island), 1);
}

TEST(MicroSignValidation, IslandAndFishDedupeToOneSign) {
    const RidgeGraph g = fishGraph();
    const auto all = MicroPatternDetector::detectAll(g, cfgFor());
    // The island body and its fish share support: one physical mark, one report, and
    // the higher-confidence island (0.95 prior) outranks the fish.
    EXPECT_EQ(countType(all, PatternType::Island) + countType(all, PatternType::Fish), 1);
    EXPECT_EQ(countType(all, PatternType::Island), 1);
}

TEST(MicroSignValidation, FilterTraceRecordsSoftScores) {
    auto islands = MicroPatternDetector::findIslands(islandGraph(), cfgFor());
    ASSERT_EQ(islands.size(), 1u);
    bool has_size = false;
    bool has_elong = false;
    for (const auto& f : islands[0].filter_trace) {
        if (f.name == "size") {
            has_size = true;
            EXPECT_NEAR(f.score, 1.0F, 1e-3F);
        }
        if (f.name == "elong") {
            has_elong = true;
            EXPECT_NEAR(f.score, 1.0F, 1e-3F);
        }
    }
    EXPECT_TRUE(has_size);
    EXPECT_TRUE(has_elong);
}

TEST(MicroSignValidation, ScaleInvariance) {
    const auto small = MicroPatternDetector::findIslands(islandGraph(1.0F), cfgFor(282.0F));
    const auto large = MicroPatternDetector::findIslands(islandGraph(2.0F), cfgFor(564.0F));
    ASSERT_EQ(small.size(), 1u);
    ASSERT_EQ(large.size(), 1u);
    EXPECT_NEAR(small[0].confidence, large[0].confidence, 1e-3F);
}

TEST(MicroSignValidation, LegacyApiUnchangedByValidation) {
    // The legacy topology-only API keeps fixed confidences regardless of geometry.
    const RidgeGraph g = triangleGraph(3.0F); // oversized: validated drops, legacy keeps
    auto legacy = MicroPatternDetector::findTriangles(g);
    ASSERT_EQ(legacy.size(), 1u);
    EXPECT_NEAR(legacy[0].confidence, 0.90F, 1e-6F);
}

// --- false-positive gate on a random crease arrangement ---

TEST(MicroSignValidation, RandomArrangementFalsePositiveGate) {
    std::mt19937 rng(42u);
    std::uniform_real_distribution<float> dist(20.0F, 262.0F);
    std::vector<std::pair<Point2D, Point2D>> lines;
    for (int i = 0; i < 12; ++i) {
        lines.emplace_back(P(dist(rng), dist(rng)), P(dist(rng), dist(rng)));
    }
    const RidgeGraph g = lineArrangementGraph(lines);
    const auto legacy = MicroPatternDetector::detectAll(g);
    const auto validated = MicroPatternDetector::detectAll(g, cfgFor());
    // Validation may only remove candidates, never add them.
    EXPECT_LE(validated.size(), legacy.size());
    // FP gate (< 0.5 signs/image would be ideal; pinned to the measured value + slack).
    EXPECT_LE(validated.size(), 2u);
}

// --- host-line attribution ---

TEST(HostAttributionTest, ProjectsToNearestHostWithinBand) {
    std::vector<HostPolyline> hosts;
    HostPolyline heart;
    heart.curve_id = 0;
    heart.name = "heart_line";
    heart.points = {P(0.0F, 300.0F), P(200.0F, 300.0F)};
    hosts.push_back(heart);
    HostPolyline head;
    head.curve_id = 1;
    head.name = "head_line";
    head.points = {P(0.0F, 200.0F), P(200.0F, 200.0F)};
    hosts.push_back(head);

    const auto hit = attributeHost(P(120.0F, 195.0F), hosts, 10.0F);
    ASSERT_TRUE(hit.attributed());
    EXPECT_EQ(hit.host_curve_id, 1);
    EXPECT_EQ(hit.host_name, "head_line");
    EXPECT_NEAR(hit.host_arc_s, 0.6F, 0.01F);
    EXPECT_NEAR(hit.dist_px, 5.0F, 0.01F);

    const auto miss = attributeHost(P(120.0F, 250.0F), hosts, 10.0F);
    EXPECT_FALSE(miss.attributed());
    EXPECT_NEAR(miss.dist_px, 50.0F, 0.01F);
}

// ---------------------------------------------------------------------------
// Junction derivation (segments-only graphs -> topology the detectors can read)
// ---------------------------------------------------------------------------

TEST(JunctionDeriverTest, EndpointClustersMergeIntoJunctions) {
    RidgeGraph g;
    // Four segments whose endpoints meet at (100,100) with sub-pixel jitter.
    g.segments.push_back(makeSeg(0, {P(80.0F, 99.0F), P(100.0F, 100.0F)}));
    g.segments.push_back(makeSeg(1, {P(100.5F, 101.0F), P(120.0F, 100.0F)}));
    g.segments.push_back(makeSeg(2, {P(100.0F, 100.5F), P(100.0F, 120.0F)}));
    g.segments.push_back(makeSeg(3, {P(99.5F, 99.5F), P(80.0F, 80.0F)}));
    EXPECT_EQ(deriveJunctions(g, 2.5F), 1u);
    ASSERT_EQ(g.junctions.size(), 1u);
    EXPECT_EQ(g.junctions[0].connected_segment_ids.size(), 4u);
    EXPECT_NEAR(g.junctions[0].position.x, 100.0F, 1.0F);
    EXPECT_NEAR(g.junctions[0].position.y, 100.0F, 1.0F);
    EXPECT_EQ(g.segments.size(), 4u); // no crossings: segments untouched
}

TEST(JunctionDeriverTest, ThroughCrossingSplitsIntoFourArms) {
    RidgeGraph g;
    // Two short creases traced straight through each other (no shared endpoints).
    g.segments.push_back(makeSeg(0, {P(95.0F, 110.0F), P(125.0F, 110.0F)}));
    g.segments.push_back(makeSeg(1, {P(110.0F, 95.0F), P(110.0F, 125.0F)}));
    EXPECT_EQ(deriveJunctions(g, 2.5F), 1u);
    ASSERT_EQ(g.junctions.size(), 1u);
    EXPECT_NEAR(g.junctions[0].position.x, 110.0F, 0.5F);
    EXPECT_NEAR(g.junctions[0].position.y, 110.0F, 0.5F);
    EXPECT_EQ(g.junctions[0].connected_segment_ids.size(), 4u); // split into 4 pieces
    EXPECT_EQ(g.segments.size(), 4u);

    // The validated detector now sees a proper cross sign.
    auto crosses = MicroPatternDetector::findCrosses(g, cfgFor());
    ASSERT_EQ(crosses.size(), 1u);
    EXPECT_GE(crosses[0].confidence, 0.6F);
}

TEST(JunctionDeriverTest, NoJunctionWithoutContact) {
    RidgeGraph g;
    g.segments.push_back(makeSeg(0, {P(50.0F, 50.0F), P(80.0F, 50.0F)}));
    g.segments.push_back(makeSeg(1, {P(120.0F, 120.0F), P(150.0F, 120.0F)}));
    EXPECT_EQ(deriveJunctions(g, 2.5F), 0u);
    EXPECT_TRUE(g.junctions.empty());
    EXPECT_EQ(g.segments.size(), 2u);
}

TEST(JunctionDeriverTest, IslandLoopDetectedFromDerivedJunctions) {
    // Pipeline reality: a segments-only graph (no junction entries). The theta loop's
    // shared endpoints must merge into two junctions and the island must surface.
    RidgeGraph g = islandGraph();
    g.junctions.clear();
    EXPECT_EQ(deriveJunctions(g, 2.5F), 2u);
    auto islands = MicroPatternDetector::findIslands(g, cfgFor());
    ASSERT_EQ(islands.size(), 1u);
    EXPECT_GE(islands[0].confidence, 0.6F);
}
