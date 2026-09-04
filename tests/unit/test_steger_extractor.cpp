#include <gtest/gtest.h>
#include <cmath>
#include <numeric>
#include <opencv2/imgproc.hpp>
#include "curv/ridge/HessianMath.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"

using namespace CurvEngine;
using namespace CurvEngine::ridge;

TEST(HessianMathTest, Test_DiagonalHessian) {
    // Matrix [ 4.0   0.0 ]
    //        [ 0.0   1.0 ]
    const auto res = computeHessianEigen2x2(4.0f, 0.0f, 1.0f);

    EXPECT_NEAR(res.lambda1, 4.0f, 1e-5f);
    EXPECT_NEAR(res.lambda2, 1.0f, 1e-5f);
    EXPECT_NEAR(std::abs(res.nx), 1.0f, 1e-4f);
    EXPECT_NEAR(std::abs(res.ny), 0.0f, 1e-4f);
}

TEST(HessianMathTest, Test_RotatedHessian45Deg) {
    // Matrix [ 2.0   1.0 ]
    //        [ 1.0   2.0 ]
    // Eigenvalues: 3.0 (eigenvector [1, 1]/sqrt(2)) and 1.0 (eigenvector [-1, 1]/sqrt(2))
    const auto res = computeHessianEigen2x2(2.0f, 1.0f, 2.0f);

    EXPECT_NEAR(res.lambda1, 3.0f, 1e-4f);
    EXPECT_NEAR(res.lambda2, 1.0f, 1e-4f);

    EXPECT_NEAR(std::abs(res.nx), 0.707106f, 1e-3f);
    EXPECT_NEAR(std::abs(res.ny), 0.707106f, 1e-3f);
}

TEST(HessianMathTest, Test_SubpixelOffsetTaylorFormula) {
    const float offset = computeSubpixelOffset(2.5f, 0.0f, 1.0f, 0.0f, 10.0f);
    EXPECT_NEAR(offset, -0.25f, 1e-5f);
}

TEST(StegerExtractorTest, Test_SyntheticCircleSubpixelAccuracy) {
    const int W = 300;
    const int H = 300;
    const float center_x = 150.0f;
    const float center_y = 150.0f;
    const float target_radius = 70.0f;
    const float ridge_sigma = 2.0f;

    cv::Mat canvas = cv::Mat::zeros(H, W, CV_32F);

    for (int y = 0; y < H; ++y) {
        float* row_ptr = canvas.ptr<float>(y);
        for (int x = 0; x < W; ++x) {
            const float dx = static_cast<float>(x) - center_x;
            const float dy = static_cast<float>(y) - center_y;
            const float dist = std::sqrt(dx * dx + dy * dy);
            const float r_diff = dist - target_radius;
            row_ptr[x] = 255.0f * std::exp(-0.5f * (r_diff * r_diff) / (ridge_sigma * ridge_sigma));
        }
    }

    cv::Mat canvas_8u;
    canvas.convertTo(canvas_8u, CV_8U);

    StegerConfig config;
    config.sigma = ridge_sigma;
    config.low_threshold = 0.5f;
    config.high_threshold = 1.5f;
    config.extract_dark_lines = false;
    config.min_segment_length = 20.0f;

    StegerRidgeExtractor extractor(config);
    const RidgeGraph graph = extractor.extractRidgeGraph(canvas_8u);

    ASSERT_FALSE(graph.segments.empty()) << "Expected at least one extracted curve segment.";

    std::vector<float> radial_errors;
    size_t total_points = 0;

    for (const auto& seg : graph.segments) {
        for (const auto& pt : seg.points) {
            const float dx = pt.x - center_x;
            const float dy = pt.y - center_y;
            const float measured_radius = std::sqrt(dx * dx + dy * dy);
            const float err = std::abs(measured_radius - target_radius);
            radial_errors.push_back(err);
            total_points++;
        }
    }

    ASSERT_GT(total_points, 50u) << "Circle extraction should have yielded continuous points.";

    const float mean_error = std::accumulate(radial_errors.begin(), radial_errors.end(), 0.0f) /
                             static_cast<float>(radial_errors.size());

    // Sub-pixel accuracy validation: Error must be strictly under 0.1 pixels
    EXPECT_LT(mean_error, 0.10f) << "Mean radial sub-pixel error exceeded 0.1 pixel tolerance: " << mean_error;
}

TEST(PipelineTest, Test_SyncPerceptionPipelineEndToEnd) {
    // 1. Create a well-illuminated frame with high-contrast line
    cv::Mat image(300, 300, CV_8UC3, cv::Scalar(40, 40, 40));
    cv::line(image, cv::Point(50, 150), cv::Point(250, 150), cv::Scalar(255, 255, 255), 3, cv::LINE_AA);

    Frame frame(image, "unit_test_frame_001", 1725410000000ULL);

    quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = 10.0;
    q_thresh.min_contrast = 5.0;
    q_thresh.min_brightness = 10.0;

    auto quality_analyzer = std::make_shared<quality::LaplacianQualityAnalyzer>(q_thresh);

    StegerConfig s_cfg;
    s_cfg.sigma = 1.5f;
    s_cfg.low_threshold = 0.3f;
    s_cfg.high_threshold = 1.0f;
    s_cfg.extract_dark_lines = false;

    auto ridge_extractor = std::make_shared<StegerRidgeExtractor>(s_cfg);

    pipeline::SyncPerceptionPipeline pipeline(quality_analyzer, ridge_extractor);
    const auto result = pipeline.process(frame);

    EXPECT_TRUE(result.is_usable);
    EXPECT_GT(result.total_duration_us, 0);
    EXPECT_FALSE(result.ridge_graph.segments.empty());
    EXPECT_FALSE(result.evidence.empty());
}
