#include <gtest/gtest.h>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include <nlohmann/json.hpp>
#include <cmath>

#include "curv/curv_capi.h"
#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include "curv/geometry/ZoneIntersector.hpp"
#include "curv/domain/PalmZones.hpp"
#include "curv/domains/PalmDomainPack.hpp"
#include "curv/domains/SurfaceInspectionPack.hpp"
#include "curv/adapters/OverlayRenderer.hpp"

using namespace CurvEngine;

// =========================================================================
// Loophole 1: Elimination of Dual-Execution & Sync of Segments / Evidence
// =========================================================================
TEST(AuditRegressionTest, StegerSinglePassExtractionSync) {
    cv::Mat img = cv::Mat::zeros(200, 200, CV_8UC1);
    cv::line(img, cv::Point(30, 100), cv::Point(170, 100), cv::Scalar(255), 3);

    Frame frame(img, "test_sync", 1);

    quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = 1.0;
    q_thresh.min_brightness = 1.0;
    auto qa = std::make_shared<quality::LaplacianQualityAnalyzer>(q_thresh);

    ridge::StegerConfig cfg;
    cfg.sigma = 1.5f;
    cfg.low_threshold = 0.5f;
    cfg.high_threshold = 1.5f;
    cfg.extract_dark_lines = false;
    auto extractor = std::make_shared<ridge::StegerRidgeExtractor>(cfg);

    pipeline::SyncPerceptionPipeline pipe(qa, extractor);
    auto result = pipe.process(frame);

    EXPECT_TRUE(result.is_usable);
    ASSERT_FALSE(result.ridge_graph.segments.empty());
    // Verification: Evidence count matches ridge_graph segment count exactly
    EXPECT_EQ(result.ridge_graph.segments.size(), result.evidence.size());

    // Verify IDs match
    for (size_t i = 0; i < result.ridge_graph.segments.size(); ++i) {
        const auto& seg = result.ridge_graph.segments[i];
        const auto& ev = result.evidence[i];
        EXPECT_EQ(ev.geometry.value("segment_id", -1), seg.id);
        EXPECT_EQ(ev.geometry.value("point_count", 0), seg.points.size());
    }
}

// =========================================================================
// Loophole 2 & 4: Canonical Zone Alignment & Handedness Invariance
// =========================================================================
TEST(AuditRegressionTest, HandednessDetectionAndMountMirroring) {
    geometry::HandLandmarks right_hand;
    right_hand.is_normalized = true;
    right_hand.points[0]  = geometry::LandmarkPoint(0.50f, 0.85f); // Wrist
    right_hand.points[9]  = geometry::LandmarkPoint(0.50f, 0.40f); // Middle MCP
    right_hand.points[5]  = geometry::LandmarkPoint(0.35f, 0.45f); // Index MCP (left)
    right_hand.points[17] = geometry::LandmarkPoint(0.70f, 0.48f); // Pinky MCP (right)

    EXPECT_FALSE(geometry::detectIsLeftHand(right_hand));

    geometry::HandLandmarks left_hand;
    left_hand.is_normalized = true;
    left_hand.points[0]  = geometry::LandmarkPoint(0.50f, 0.85f); // Wrist
    left_hand.points[9]  = geometry::LandmarkPoint(0.50f, 0.40f); // Middle MCP
    left_hand.points[5]  = geometry::LandmarkPoint(0.65f, 0.45f); // Index MCP (right)
    left_hand.points[17] = geometry::LandmarkPoint(0.30f, 0.48f); // Pinky MCP (left)

    EXPECT_TRUE(geometry::detectIsLeftHand(left_hand));

    // Verify mount mirroring in PalmZones
    auto right_zones = domains::PalmZones::getCanonicalPalmZones(512, false);
    auto left_zones  = domains::PalmZones::getCanonicalPalmZones(512, true);

    auto find_zone = [](const std::vector<geometry::Zone>& zones, const std::string& id) -> const geometry::Zone* {
        for (const auto& z : zones) if (z.id == id) return &z;
        return nullptr;
    };

    const auto* r_jup = find_zone(right_zones, "jupiter_zone");
    const auto* l_jup = find_zone(left_zones, "jupiter_zone");
    ASSERT_NE(r_jup, nullptr);
    ASSERT_NE(l_jup, nullptr);

    // In right hand, Jupiter is on the radial/left side (x < 256)
    // In left hand, Jupiter is on the radial/right side (x > 256)
    float r_jup_mean_x = 0.0f;
    for (const auto& p : r_jup->polygon) r_jup_mean_x += p.x;
    r_jup_mean_x /= static_cast<float>(r_jup->polygon.size());

    float l_jup_mean_x = 0.0f;
    for (const auto& p : l_jup->polygon) l_jup_mean_x += p.x;
    l_jup_mean_x /= static_cast<float>(l_jup->polygon.size());

    EXPECT_LT(r_jup_mean_x, 256.0f);
    EXPECT_GT(l_jup_mean_x, 256.0f);
    // Symmetry check: r_jup_mean_x + l_jup_mean_x == 512
    EXPECT_NEAR(r_jup_mean_x + l_jup_mean_x, 512.0f, 1.0f);
}

// =========================================================================
// Loophole 3: Quad ROI Length Recalculation
// =========================================================================
TEST(AuditRegressionTest, QuadRoiLengthRecalculation) {
    CurvPipelineHandle pipeline = curv_pipeline_create("surface_inspection");
    ASSERT_NE(pipeline, nullptr);

    const int width = 400;
    const int height = 400;
    cv::Mat img = cv::Mat::ones(height, width, CV_8UC3) * 50;
    cv::line(img, cv::Point(50, 150), cv::Point(350, 150), cv::Scalar(240, 240, 240), 2);
    cv::circle(img, cv::Point(200, 200), 100, cv::Scalar(240, 240, 240), 2);

    CurvFrameHandle frame = curv_frame_create_from_buffer(
        img.data,
        width,
        height,
        0,
        CURV_PIXEL_FORMAT_BGR
    );
    ASSERT_NE(frame, nullptr);

    float quad_corners[8] = {
        20.0f, 20.0f,
        380.0f, 20.0f,
        380.0f, 380.0f,
        20.0f, 380.0f
    };

    std::vector<char> json_buf(262144, 0);
    int status = curv_pipeline_process_quad_roi(
        pipeline,
        frame,
        quad_corners,
        json_buf.data(),
        static_cast<int>(json_buf.size())
    );

    EXPECT_EQ(status, CURV_STATUS_SUCCESS);

    auto root = nlohmann::json::parse(json_buf.data());
    EXPECT_TRUE(root.contains("domain_payload"));
    const auto& dp = root["domain_payload"];
    // In unwarped coordinates the line is 200px, so it must be classified as severe_cracks
    EXPECT_GE(dp.value("severe_cracks", 0), 1);
    EXPECT_GE(dp.value("max_defect_length", 0.0f), 150.0f);

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

// =========================================================================
// Loophole 5: C-ABI Buffer Query, Truncation Safety & Null-Termination
// =========================================================================
TEST(AuditRegressionTest, CAbiBufferSafetyAndDynamicQuery) {
    CurvPipelineHandle pipeline = curv_pipeline_create("palm");
    ASSERT_NE(pipeline, nullptr);

    cv::Mat img = cv::Mat::ones(512, 512, CV_8UC3) * 50;
    // Draw distinct curves to produce evidence
    cv::line(img, cv::Point(100, 150), cv::Point(350, 150), cv::Scalar(240, 240, 240), 3);
    cv::line(img, cv::Point(100, 250), cv::Point(350, 250), cv::Scalar(240, 240, 240), 3);

    CurvFrameHandle frame = curv_frame_create_from_buffer(
        img.data,
        img.cols,
        img.rows,
        0,
        CURV_PIXEL_FORMAT_BGR
    );
    ASSERT_NE(frame, nullptr);

    std::array<float, 42> lms{};
    lms[0] = 0.5f; lms[1] = 0.85f;
    lms[18] = 0.5f; lms[19] = 0.40f; // P9
    lms[10] = 0.35f; lms[11] = 0.45f; // P5
    lms[34] = 0.70f; lms[35] = 0.48f; // P17

    // Test 1: Intentionally tiny buffer (e.g. 16 bytes)
    std::vector<char> small_buf(16, 'A');
    int status = curv_pipeline_process_landmarks_roi(
        pipeline,
        frame,
        lms.data(),
        small_buf.data(),
        static_cast<int>(small_buf.size())
    );

    EXPECT_EQ(status, CURV_STATUS_BUFFER_TOO_SMALL);
    // Verification: buffer must be safely null-terminated
    EXPECT_EQ(small_buf[0], '\0');

    // Test 2: Query needed size
    int needed_size = 0;
    int q_status = curv_pipeline_get_last_json(pipeline, nullptr, 0, &needed_size);
    EXPECT_EQ(q_status, CURV_STATUS_SUCCESS);
    EXPECT_GT(needed_size, 100);

    // Test 3: Allocate exact needed size and retrieve
    std::vector<char> exact_buf(static_cast<size_t>(needed_size), 0);
    int fetch_status = curv_pipeline_get_last_json(
        pipeline,
        exact_buf.data(),
        static_cast<int>(exact_buf.size()),
        nullptr
    );
    EXPECT_EQ(fetch_status, CURV_STATUS_SUCCESS);

    // Verification: retrieved string parses into valid JSON
    auto root = nlohmann::json::parse(exact_buf.data());
    EXPECT_TRUE(root.contains("quality"));
    EXPECT_TRUE(root.contains("domain_payload"));

    curv_frame_destroy(frame);
    curv_pipeline_destroy(pipeline);
}

// =========================================================================
// Loophole 6: Small Image Safety in OverlayRenderer
// =========================================================================
TEST(AuditRegressionTest, SmallImageOverlayRendererNoCrash) {
    cv::Mat small_img = cv::Mat::zeros(64, 64, CV_8UC3);
    RidgeGraph graph;
    QualityReport quality{50.0, 100.0, 40.0, true, "nominal"};

    adapters::OverlayConfig cfg;
    cfg.draw_hud = true;
    cfg.draw_subpixel_polylines = true;

    // Must not throw cv::Exception or crash
    cv::Mat rendered;
    EXPECT_NO_THROW({
        rendered = adapters::OverlayRenderer::render(small_img, graph, quality, cfg);
    });

    EXPECT_EQ(rendered.cols, 64);
    EXPECT_EQ(rendered.rows, 64);
}

// =========================================================================
// Loophole 7: Negative or Zero Sigma Numerical Guard
// =========================================================================
TEST(AuditRegressionTest, NonPositiveSigmaNumericalGuard) {
    ridge::StegerConfig bad_cfg_zero;
    bad_cfg_zero.sigma = 0.0f;
    ridge::StegerRidgeExtractor extractor_zero(bad_cfg_zero);

    ridge::StegerConfig bad_cfg_neg;
    bad_cfg_neg.sigma = -3.5f;
    ridge::StegerRidgeExtractor extractor_neg(bad_cfg_neg);

    cv::Mat img = cv::Mat::ones(50, 50, CV_8UC1) * 128;

    EXPECT_NO_THROW({
        auto g1 = extractor_zero.extractRidgeGraph(img);
        auto g2 = extractor_neg.extractRidgeGraph(img);
    });
}

// =========================================================================
// Loophole 8: Omnidirectional Continuity Breaks
// =========================================================================
TEST(AuditRegressionTest, OmnidirectionalContinuityBreaks) {
    domains::PalmDomainPack pack;

    RidgeGraph graph;
    // Segment 1: from (100, 100) to (150, 100)
    CurveSegment seg1;
    seg1.id = 1;
    seg1.points = {Point2D(100.0f, 100.0f), Point2D(150.0f, 100.0f)};
    seg1.total_length = 50.0f;
    graph.segments.push_back(seg1);

    // Segment 2 oriented oppositely: front is at (155, 100) (gap of 5px from seg1 back)
    // but represented with points going backwards
    CurveSegment seg2;
    seg2.id = 2;
    seg2.points = {Point2D(155.0f, 100.0f), Point2D(200.0f, 100.0f)};
    seg2.total_length = 45.0f;
    graph.segments.push_back(seg2);

    QualityReport quality{50.0, 100.0, 40.0, true, "nominal"};
    cv::Mat dummy_img = cv::Mat::zeros(512, 512, CV_8UC3);
    Frame frame(dummy_img, "test_breaks", 1);

    DomainInput d_in{frame, graph, quality};
    auto out = pack.process(d_in);

    EXPECT_GE(out.domain_payload.value("continuity_breaks", 0), 1);
}

// =========================================================================
// Loophole 9: InvertGraphCoordinates Dimension Check
// =========================================================================
TEST(AuditRegressionTest, InvertGraphCoordinatesDimensionSafety) {
    RidgeGraph graph;
    CurveSegment seg;
    seg.id = 1;
    seg.points = {Point2D(10.0f, 20.0f), Point2D(30.0f, 40.0f)};
    seg.total_length = 25.0f;
    graph.segments.push_back(seg);

    // 1x1 matrix should not crash
    cv::Mat invalid_m1 = cv::Mat::ones(1, 1, CV_64F);
    EXPECT_NO_THROW({
        geometry::invertGraphCoordinates(graph, invalid_m1);
    });
    EXPECT_FLOAT_EQ(graph.segments[0].points[0].x, 10.0f);

    // Empty matrix should not crash
    cv::Mat invalid_m2;
    EXPECT_NO_THROW({
        geometry::invertGraphCoordinates(graph, invalid_m2);
    });
    EXPECT_FLOAT_EQ(graph.segments[0].points[0].x, 10.0f);
}
