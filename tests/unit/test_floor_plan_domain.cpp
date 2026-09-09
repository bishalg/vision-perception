#include <gtest/gtest.h>
#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>

#include "curv/curv_capi.h"
#include "curv/domain/RoomPolygonDetector.hpp"
#include "curv/domain/VastuMandalaOverlay.hpp"
#include "curv/domain/WallJunctionGraph.hpp"
#include "curv/domain/WallSegmentExtractor.hpp"
#include "curv/domains/FloorPlanDomainPack.hpp"

using namespace CurvEngine;
using namespace CurvEngine::domain;
using namespace CurvEngine::domains;

namespace {

CurveSegment makeStraightSegment(int id, float x1, float y1, float x2, float y2, int step_count = 20) {
    CurveSegment seg;
    seg.id = id;
    seg.points.reserve(static_cast<size_t>(step_count + 1));
    const float dx = (x2 - x1) / static_cast<float>(step_count);
    const float dy = (y2 - y1) / static_cast<float>(step_count);
    for (int i = 0; i <= step_count; ++i) {
        seg.points.emplace_back(x1 + static_cast<float>(i) * dx, y1 + static_cast<float>(i) * dy, 150.0f, 1.0f);
    }
    seg.total_length = std::hypot(x2 - x1, y2 - y1);
    return seg;
}

} // namespace

TEST(FloorPlanDomainTest, Test_SingleRoom_WallAndPolygonDetection) {
    // Construct a 400x300 rectangular room: (50, 50) to (450, 350)
    RidgeGraph graph;
    graph.segments.push_back(makeStraightSegment(1, 50.0f, 50.0f, 450.0f, 50.0f));   // Top
    graph.segments.push_back(makeStraightSegment(2, 450.0f, 50.0f, 450.0f, 350.0f)); // Right
    graph.segments.push_back(makeStraightSegment(3, 450.0f, 350.0f, 50.0f, 350.0f)); // Bottom
    graph.segments.push_back(makeStraightSegment(4, 50.0f, 350.0f, 50.0f, 50.0f));   // Left

    cv::Mat blank(500, 500, CV_8UC3, cv::Scalar(255, 255, 255));
    Frame frame(blank, "floor_plan_1", 1);
    frame.metadata["scale_px_per_m"] = "20.0"; // 20 px = 1 meter
    frame.metadata["north_deg"] = "0.0";

    QualityReport quality{150.0, 200.0, 50.0, true, "OK"};
    DomainInput d_in{frame, graph, quality};

    FloorPlanDomainPack pack;
    DomainOutput d_out = pack.process(d_in);

    EXPECT_EQ(d_out.domain_name, "org.curv.domain.floor_plan");
    const auto& payload = d_out.domain_payload;

    EXPECT_EQ(payload["status"], "success");
    EXPECT_EQ(payload["wall_count"], 4);
    EXPECT_EQ(payload["junction_count"], 4);

    // Verify all 4 junctions are CORNER_L
    for (const auto& j : payload["junctions"]) {
        EXPECT_EQ(j["type"], "CORNER_L");
    }

    // Exactly 1 room detected
    EXPECT_EQ(payload["room_count"], 1);
    const auto& room = payload["rooms"][0];
    EXPECT_NEAR(static_cast<double>(room["area_px"]), 120000.0, 10.0); // 400 * 300
    EXPECT_NEAR(static_cast<double>(room["area_sqm"]), 300.0, 1.0);    // 120000 / (20*20) = 300 m^2
    EXPECT_NEAR(static_cast<double>(room["centroid"]["x"]), 250.0, 2.0);
    EXPECT_NEAR(static_cast<double>(room["centroid"]["y"]), 200.0, 2.0);

    // 9 Vastu zones generated
    EXPECT_EQ(payload["vastu_zones"].size(), 9u);
    EXPECT_FALSE(payload["grounded_prompt"].get<std::string>().empty());
}

TEST(FloorPlanDomainTest, Test_MultiRoomWithPartition_TJunctions) {
    // Construct a 600x400 area with a vertical divider wall at x = 300
    // Forms two 300x400 rooms: Room 1 (0..300, 0..400) and Room 2 (300..600, 0..400)
    RidgeGraph graph;
    graph.segments.push_back(makeStraightSegment(1, 0.0f, 0.0f, 600.0f, 0.0f));     // Top exterior
    graph.segments.push_back(makeStraightSegment(2, 600.0f, 0.0f, 600.0f, 400.0f)); // Right exterior
    graph.segments.push_back(makeStraightSegment(3, 600.0f, 400.0f, 0.0f, 400.0f)); // Bottom exterior
    graph.segments.push_back(makeStraightSegment(4, 0.0f, 400.0f, 0.0f, 0.0f));     // Left exterior
    graph.segments.push_back(makeStraightSegment(5, 300.0f, 0.0f, 300.0f, 400.0f)); // Center partition

    WallExtractionConfig w_cfg;
    WallSegmentExtractor extractor(w_cfg);
    auto walls = extractor.extractWalls(graph);

    JunctionGraphConfig j_cfg;
    WallJunctionGraph j_graph(j_cfg);
    j_graph.build(walls);

    int corner_count = 0;
    int tee_count = 0;
    for (const auto& j : j_graph.junctions()) {
        if (j.type == WallJunctionType::CORNER_L) corner_count++;
        if (j.type == WallJunctionType::TEE_T) tee_count++;
    }

    EXPECT_EQ(corner_count, 4); // 4 outer corners
    EXPECT_EQ(tee_count, 2);    // 2 T-junctions where divider meets top & bottom walls

    RoomDetectionConfig r_cfg;
    RoomPolygonDetector detector(r_cfg);
    auto rooms = detector.detectRooms(j_graph.snappedWalls(), j_graph.junctions());

    EXPECT_EQ(rooms.size(), 2u);
    for (const auto& rm : rooms) {
        EXPECT_NEAR(rm.area_px, 120000.0f, 20.0f); // 300 * 400
    }
}

TEST(FloorPlanDomainTest, Test_VastuMandala_9PadOrientation) {
    VastuConfig cfg;
    cfg.north_orientation_deg = 0.0f; // North is UP
    VastuMandalaOverlay overlay(cfg);

    auto zones = overlay.generateMandala(0.0f, 0.0f, 300.0f, 300.0f);
    ASSERT_EQ(zones.size(), 9u);

    // Grid coordinates:
    // Row 0 (Top):    VAYU(NW),    UTTARA(N),   ISHANYA(NE)
    // Row 1 (Mid):    PASHCHIMA(W),BRAHMA(C),   PURVA(E)
    // Row 2 (Bottom): NAIRUTYA(SW),DAKSHINA(S), AGNI(SE)
    EXPECT_EQ(zones[0].id, VastuZoneId::VAYU);
    EXPECT_EQ(zones[1].id, VastuZoneId::UTTARA);
    EXPECT_EQ(zones[2].id, VastuZoneId::ISHANYA);
    EXPECT_EQ(zones[3].id, VastuZoneId::PASHCHIMA);
    EXPECT_EQ(zones[4].id, VastuZoneId::BRAHMASTHAN);
    EXPECT_EQ(zones[5].id, VastuZoneId::PURVA);
    EXPECT_EQ(zones[6].id, VastuZoneId::NAIRUTYA);
    EXPECT_EQ(zones[7].id, VastuZoneId::DAKSHINA);
    EXPECT_EQ(zones[8].id, VastuZoneId::AGNI);

    // Check Brahmasthan metadata
    auto [name, elem] = VastuMandalaOverlay::getZoneMetadata(VastuZoneId::BRAHMASTHAN);
    EXPECT_NE(name.find("Brahmasthan"), std::string::npos);
    EXPECT_NE(elem.find("Akasha"), std::string::npos);
}

TEST(FloorPlanDomainTest, Test_VastuFixture_ComplianceEvaluation) {
    VastuConfig cfg;
    VastuMandalaOverlay overlay(cfg);
    auto zones = overlay.generateMandala(0.0f, 0.0f, 300.0f, 300.0f);
    std::vector<RoomPolygon> empty_rooms;

    std::vector<FixtureItem> fixtures;

    // Fixture 1: Stove in Agni (SE: x=250, y=250) -> Ideal COMPLIANT
    FixtureItem fix_stove_ok;
    fix_stove_ok.id = 1;
    fix_stove_ok.type = FixtureType::STOVE;
    fix_stove_ok.label = "Kitchen Cooktop";
    fix_stove_ok.position = Point2D{250.0f, 250.0f};
    fixtures.push_back(fix_stove_ok);

    // Fixture 2: Stove in Ishanya (NE: x=250, y=50) -> MAJOR_DEFECT
    FixtureItem fix_stove_bad;
    fix_stove_bad.id = 2;
    fix_stove_bad.type = FixtureType::STOVE;
    fix_stove_bad.label = "Misplaced Stove";
    fix_stove_bad.position = Point2D{250.0f, 50.0f};
    fixtures.push_back(fix_stove_bad);

    // Fixture 3: Bed in Nairutya (SW: x=50, y=250) -> Ideal COMPLIANT
    FixtureItem fix_bed_ok;
    fix_bed_ok.id = 3;
    fix_bed_ok.type = FixtureType::BED;
    fix_bed_ok.label = "Master Bed";
    fix_bed_ok.position = Point2D{50.0f, 250.0f};
    fixtures.push_back(fix_bed_ok);

    // Fixture 4: Toilet in Ishanya (NE: x=250, y=50) -> MAJOR_DEFECT
    FixtureItem fix_toilet_bad;
    fix_toilet_bad.id = 4;
    fix_toilet_bad.type = FixtureType::TOILET;
    fix_toilet_bad.label = "Northeast Toilet";
    fix_toilet_bad.position = Point2D{250.0f, 50.0f};
    fixtures.push_back(fix_toilet_bad);

    overlay.evaluateFixtures(fixtures, zones, empty_rooms);

    EXPECT_EQ(fixtures[0].vastu_zone, VastuZoneId::AGNI);
    EXPECT_EQ(fixtures[0].compliance, ComplianceStatus::COMPLIANT);

    EXPECT_EQ(fixtures[1].vastu_zone, VastuZoneId::ISHANYA);
    EXPECT_EQ(fixtures[1].compliance, ComplianceStatus::MAJOR_DEFECT);

    EXPECT_EQ(fixtures[2].vastu_zone, VastuZoneId::NAIRUTYA);
    EXPECT_EQ(fixtures[2].compliance, ComplianceStatus::COMPLIANT);

    EXPECT_EQ(fixtures[3].vastu_zone, VastuZoneId::ISHANYA);
    EXPECT_EQ(fixtures[3].compliance, ComplianceStatus::MAJOR_DEFECT);
}

TEST(FloorPlanDomainTest, Test_CAPI_FloorPlanPipeline) {
    CurvPipelineHandle pipeline = curv_pipeline_create("floor_plan");
    ASSERT_NE(pipeline, nullptr);

    // Create a 400x400 white image with dark walls
    cv::Mat floor_img(400, 400, CV_8UC3, cv::Scalar(255, 255, 255));
    // Draw rectangular outer walls
    cv::rectangle(floor_img, cv::Point(50, 50), cv::Point(350, 350), cv::Scalar(0, 0, 0), 4);
    // Draw a dividing interior wall
    cv::line(floor_img, cv::Point(200, 50), cv::Point(200, 350), cv::Scalar(0, 0, 0), 4);

    CurvFrameHandle frame = curv_frame_create_from_buffer(
        floor_img.data, floor_img.cols, floor_img.rows,
        static_cast<int>(floor_img.step), CURV_PIXEL_FORMAT_BGR);
    ASSERT_NE(frame, nullptr);

    std::vector<char> json_buf(1048576, 0); // 1 MB
    int status = curv_pipeline_process_frame(pipeline, frame, json_buf.data(), static_cast<int>(json_buf.size()));
    EXPECT_EQ(status, CURV_STATUS_SUCCESS);

    nlohmann::json js = nlohmann::json::parse(json_buf.data());
    EXPECT_TRUE(js.contains("domain_payload"));
    EXPECT_EQ(js["domain_payload"]["status"], "success");
    EXPECT_GT(js["domain_payload"]["wall_count"].get<int>(), 0);
    EXPECT_GT(js["domain_payload"]["room_count"].get<int>(), 0);
    EXPECT_EQ(js["domain_payload"]["vastu_zones"].size(), 9u);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}
