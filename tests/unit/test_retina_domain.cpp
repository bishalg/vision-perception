#include "curv/domains/RetinaDomainPack.hpp"
#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include <gtest/gtest.h>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include <cmath>

using namespace CurvEngine;
using namespace CurvEngine::domains;

TEST(RetinaDomainTest, GreenChannelPreparation) {
    // 1. Empty image
    cv::Mat empty_mat;
    EXPECT_TRUE(RetinaDomainPack::prepareGreenChannel(empty_mat).empty());

    // 2. Synthetic 3-channel image
    cv::Mat bgr(100, 100, CV_8UC3, cv::Scalar(20, 100, 200));
    // Draw a darker green line (vessel)
    cv::line(bgr, cv::Point(10, 50), cv::Point(90, 50), cv::Scalar(10, 30, 120), 3);

    cv::Mat prepared = RetinaDomainPack::prepareGreenChannel(bgr, true);
    ASSERT_FALSE(prepared.empty());
    EXPECT_EQ(prepared.channels(), 1);
    EXPECT_EQ(prepared.cols, 100);
    EXPECT_EQ(prepared.rows, 100);

    // Center of vessel should have higher intensity in inverted green channel than background
    EXPECT_GT(prepared.at<uint8_t>(50, 50), prepared.at<uint8_t>(10, 50));
}

TEST(RetinaDomainTest, TortuosityCalculation) {
    // 1. Straight line: arc == chord -> tortuosity = 1.0
    CurveSegment straight_seg;
    straight_seg.id = 1;
    for (int i = 0; i <= 50; ++i) {
        Point2D pt(static_cast<float>(i), 10.0f, 100.0f, 1.0f);
        straight_seg.points.push_back(pt);
    }
    straight_seg.total_length = 50.0f;

    const double tau_straight = RetinaDomainPack::computeArcChordTortuosity(straight_seg);
    EXPECT_NEAR(tau_straight, 1.0, 1e-4);

    // 2. S-curve / sinewave: arc length > chord
    CurveSegment curved_seg;
    curved_seg.id = 2;
    float arc_sum = 0.0f;
    for (int i = 0; i <= 100; ++i) {
        const float px = static_cast<float>(i);
        const float py = 50.0f + 25.0f * std::sin(static_cast<float>(i) * 0.1f);
        Point2D pt(px, py, 100.0f, 1.0f);
        if (!curved_seg.points.empty()) {
            arc_sum += std::hypot(pt.x - curved_seg.points.back().x, pt.y - curved_seg.points.back().y);
        }
        curved_seg.points.push_back(pt);
    }
    curved_seg.total_length = arc_sum;

    const double tau_curved = RetinaDomainPack::computeArcChordTortuosity(curved_seg);
    EXPECT_GT(tau_curved, 1.10);
}

TEST(RetinaDomainTest, EndToEndExtractionOnRetinaFixture) {
    const std::string fixture_path = "data/fixtures/retina/sample_fundus_healthy.png";
    cv::Mat fundus_bgr = cv::imread(fixture_path, cv::IMREAD_COLOR);

    if (fundus_bgr.empty()) {
        // Fallback synthetic canvas if run from a different relative directory
        fundus_bgr = cv::Mat(400, 400, CV_8UC3, cv::Scalar(15, 70, 170));
        cv::line(fundus_bgr, cv::Point(50, 200), cv::Point(350, 200), cv::Scalar(8, 20, 85), 4);
    }

    cv::Mat prepped = RetinaDomainPack::prepareGreenChannel(fundus_bgr, true);
    Frame frame(prepped, "test_fundus", 1000);

    ridge::StegerConfig cfg;
    cfg.sigma = 1.5f;
    cfg.low_threshold = 1.0f;
    cfg.high_threshold = 3.0f;

    ridge::StegerRidgeExtractor extractor(cfg);
    const RidgeGraph ridges = extractor.extractRidgeGraph(prepped);

    QualityReport quality(0.05, 0.55, 0.85, true, "Optimal");

    RetinaDomainPack pack(10.0f);
    DomainInput input{frame, ridges, quality, nullptr};

    DomainOutput output = pack.process(input);

    EXPECT_EQ(output.domain_name, "org.curv.domain.retina");
    EXPECT_TRUE(output.domain_payload.contains("total_vessels_detected"));
    EXPECT_TRUE(output.domain_payload.contains("vessel_density_ratio"));
    EXPECT_TRUE(output.domain_payload.contains("mean_tortuosity_index"));
    EXPECT_TRUE(output.domain_payload.contains("mean_vessel_width_px"));
    EXPECT_TRUE(output.domain_payload["frame_usable"].get<bool>());

    const double mean_tortuosity = output.domain_payload["mean_tortuosity_index"].get<double>();
    EXPECT_GE(mean_tortuosity, 1.0);
}
