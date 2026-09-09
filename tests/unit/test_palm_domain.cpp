#include <gtest/gtest.h>
#include "curv/domains/PalmDomainPack.hpp"

using namespace CurvEngine;
using namespace CurvEngine::domains;

TEST(PalmDomainTest, Test_MajorCreaseClassification) {
    // Construct mock RidgeGraph simulating a palm with 3 major creases:
    // 1. Heart line: top transverse curve (y ~ 50, x from 50 to 250)
    CurveSegment heart;
    heart.id = 10;
    heart.total_length = 200.0f;
    for (int x = 50; x <= 250; x += 10) {
        heart.points.emplace_back(static_cast<float>(x), 50.0f + 5.0f * std::sin(static_cast<float>(x) * 0.02f));
    }

    // 2. Head line: middle diagonal curve (y ~ 120, x from 40 to 220)
    CurveSegment head;
    head.id = 20;
    head.total_length = 180.0f;
    for (int x = 40; x <= 220; x += 10) {
        head.points.emplace_back(static_cast<float>(x), 100.0f + 0.3f * static_cast<float>(x - 40));
    }

    // 3. Life line: radial longitudinal curve curving downward (y from 100 to 280, x from 40 to 120)
    CurveSegment life;
    life.id = 30;
    life.total_length = 210.0f;
    for (int i = 0; i <= 18; ++i) {
        const float angle = 3.14159f * 0.5f * (static_cast<float>(i) / 18.0f);
        life.points.emplace_back(40.0f + 80.0f * (1.0f - std::cos(angle)), 100.0f + 180.0f * std::sin(angle));
    }

    RidgeGraph graph;
    graph.segments = {heart, head, life};

    cv::Mat blank(300, 300, CV_8UC3, cv::Scalar(0, 0, 0));
    Frame frame(blank, "mock_palm", 1);
    QualityReport quality{100.0, 120.0, 50.0, true, "OK"};

    PalmDomainPack palm_pack;
    DomainInput d_in{frame, graph, quality};
    DomainOutput d_out = palm_pack.process(d_in);

    EXPECT_EQ(d_out.domain_name, "org.curv.domain.palm");
    EXPECT_EQ(d_out.domain_payload["status"], "analyzed");

    // Verify identified lines
    EXPECT_EQ(d_out.domain_payload["heart_line_id"], 10);
    EXPECT_EQ(d_out.domain_payload["head_line_id"], 20);
    EXPECT_EQ(d_out.domain_payload["life_line_id"], 30);
    EXPECT_TRUE(d_out.domain_payload["fate_line_id"].is_null());

    // Verify canonical major_lines structure
    EXPECT_TRUE(d_out.domain_payload.contains("major_lines"));
    const auto& ml = d_out.domain_payload["major_lines"];
    EXPECT_TRUE(ml.contains("heart_line"));
    EXPECT_TRUE(ml["heart_line"]["present"].get<bool>());
    EXPECT_GT(ml["heart_line"]["arc_length_px"].get<float>(), 100.0f);
    EXPECT_GE(ml["heart_line"]["tortuosity"].get<float>(), 1.0f);

    EXPECT_TRUE(ml.contains("head_line"));
    EXPECT_TRUE(ml["head_line"]["present"].get<bool>());

    EXPECT_TRUE(ml.contains("life_line"));
    EXPECT_TRUE(ml["life_line"]["present"].get<bool>());

    EXPECT_TRUE(ml.contains("fate_line"));
    EXPECT_FALSE(ml["fate_line"]["present"].get<bool>());

    // Verify palm_analysis object
    EXPECT_TRUE(d_out.domain_payload.contains("palm_analysis"));
    const auto& pa = d_out.domain_payload["palm_analysis"];
    EXPECT_TRUE(pa.contains("dominant_line"));
    EXPECT_NE(pa["dominant_line"].get<std::string>(), "none");
    EXPECT_TRUE(pa.contains("heart_head_min_distance_px"));
    EXPECT_GT(pa["heart_head_min_distance_px"].get<float>(), 0.0f);

    // Continuity breaks should be sum of bridged chain gaps (0 for single segments)
    EXPECT_EQ(d_out.domain_payload["continuity_breaks"], 0);

    // Verify domain evidence generated
    EXPECT_GE(d_out.domain_evidence.size(), 3u);
}

