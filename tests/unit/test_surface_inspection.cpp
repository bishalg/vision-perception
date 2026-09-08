#include <gtest/gtest.h>
#include "curv/domains/SurfaceInspectionPack.hpp"

using namespace CurvEngine;
using namespace CurvEngine::domains;

TEST(SurfaceInspectionTest, Test_DefectClassificationAndPayload) {
    // 1. Create a severe crack (> 80 px)
    CurveSegment crack;
    crack.id = 1;
    crack.total_length = 95.0f;
    for (int i = 0; i < 95; ++i) {
        crack.points.emplace_back(static_cast<float>(i), 50.0f);
    }

    // 2. Create a micro scratch (< 40 px)
    CurveSegment scratch;
    scratch.id = 2;
    scratch.total_length = 32.0f;
    for (int i = 0; i < 32; ++i) {
        scratch.points.emplace_back(100.0f + static_cast<float>(i), 100.0f);
    }

    // 3. Create a tiny noise segment (< 10 px) which should be ignored
    CurveSegment noise;
    noise.id = 3;
    noise.total_length = 8.0f;
    noise.points.emplace_back(10.0f, 10.0f);

    RidgeGraph graph;
    graph.segments = {crack, scratch, noise};

    cv::Mat blank(200, 200, CV_8UC1, cv::Scalar(128));
    Frame frame(blank, "surface_frame", 1);
    QualityReport quality{200.0, 128.0, 40.0, true, "OK"};

    SurfaceInspectionPack pack(20.0f); // Filter out segments < 20px
    DomainInput d_in{frame, graph, quality};
    DomainOutput d_out = pack.process(d_in);

    EXPECT_EQ(d_out.domain_name, "org.curv.domain.surface_inspection");
    EXPECT_EQ(d_out.domain_payload["total_defects_found"], 2);
    EXPECT_EQ(d_out.domain_payload["severe_cracks"], 1);
    EXPECT_EQ(d_out.domain_payload["micro_scratches"], 1);
    EXPECT_NEAR(static_cast<double>(d_out.domain_payload["max_defect_length"]), 95.0, 1e-3);

    ASSERT_EQ(d_out.domain_evidence.size(), 2u);
    EXPECT_EQ(d_out.domain_evidence[0].type, "severe_crack");
    EXPECT_EQ(d_out.domain_evidence[1].type, "micro_scratch");
}

TEST(SurfaceInspectionTest, ZeroDefectsReturnsZeroMean) {
    RidgeGraph graph; // empty
    cv::Mat blank(200, 200, CV_8UC1, cv::Scalar(128));
    Frame frame(blank, "surface_frame", 1);
    QualityReport quality{200.0, 128.0, 40.0, true, "OK"};
    SurfaceInspectionPack pack(20.0f);
    DomainInput d_in{frame, graph, quality};
    DomainOutput d_out = pack.process(d_in);
    EXPECT_EQ(d_out.domain_payload["total_defects_found"], 0);
    EXPECT_DOUBLE_EQ(static_cast<double>(d_out.domain_payload["mean_defect_length"]), 0.0);
}
