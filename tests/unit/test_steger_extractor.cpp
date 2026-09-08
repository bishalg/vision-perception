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

TEST(HessianMathTest, Test_SubpixelOffsetZeroCurvatureIsInvalid) {
    // A zero dominant curvature has no Taylor zero-crossing: the sentinel
    // offset (100) marks the candidate invalid.
    EXPECT_FLOAT_EQ(computeSubpixelOffset(2.5f, 1.0f, 1.0f, 0.0f, 0.0f), 100.0f);
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

// === Step 4 coverage-gap tests (characterization) ===

namespace {

// A minimal non-Steger extractor to exercise the pipeline's generic path.
class FakeExtractor final : public interfaces::ILineCurveExtractor {
public:
    std::vector<Evidence> extract(const Frame&) override {
        return {Evidence("fake_seg_1", "line_candidate", 0.5)};
    }
};

} // anonymous namespace

TEST(StegerExtractorTest, Test_EmptyImageReturnsEmptyGraph) {
    StegerRidgeExtractor extractor;
    const RidgeGraph graph = extractor.extractRidgeGraph(cv::Mat());
    EXPECT_TRUE(graph.segments.empty());
    EXPECT_TRUE(graph.junctions.empty());
    EXPECT_TRUE(graph.debug_mask.empty());
}

TEST(StegerExtractorTest, Test_FourChannelImageExtraction) {
    // BGRA input must be converted (COLOR_BGRA2GRAY path) before extraction.
    cv::Mat image(200, 200, CV_8UC4, cv::Scalar(40, 40, 40, 255));
    cv::line(image, cv::Point(30, 100), cv::Point(170, 100), cv::Scalar(255, 255, 255, 255), 3, cv::LINE_AA);

    StegerConfig config;
    config.low_threshold = 0.5f;
    config.high_threshold = 1.0f;
    config.min_segment_length = 20.0f;
    StegerRidgeExtractor extractor(config);
    const RidgeGraph graph = extractor.extractRidgeGraph(image);
    EXPECT_FALSE(graph.segments.empty());
}

TEST(StegerExtractorTest, Test_FullSpanPlusShapeTracesToBorders) {
    // A thick plus spanning the entire frame forces the tracer to reach the
    // image borders (out-of-range neighbor guards) and to reject ambiguous
    // junction neighbors (dot <= best_score) at the crossing.
    cv::Mat image(300, 300, CV_8UC1, cv::Scalar(0));
    cv::line(image, cv::Point(0, 150), cv::Point(299, 150), cv::Scalar(255), 3, cv::LINE_AA);
    cv::line(image, cv::Point(150, 0), cv::Point(150, 299), cv::Scalar(255), 3, cv::LINE_AA);

    StegerConfig config;
    config.sigma = 1.5f;
    config.low_threshold = 0.5f;
    config.high_threshold = 1.5f;
    config.min_segment_length = 30.0f;
    StegerRidgeExtractor extractor(config);
    const RidgeGraph graph = extractor.extractRidgeGraph(image);

    ASSERT_GE(graph.segments.size(), 2u);
    EXPECT_FALSE(graph.debug_mask.empty());
    EXPECT_GT(cv::countNonZero(graph.debug_mask), 0);
    for (const auto& seg : graph.segments) {
        EXPECT_GE(seg.total_length, config.min_segment_length);
        EXPECT_FALSE(seg.points.empty());
    }
}

TEST(StegerExtractorTest, Test_DegenerateDirectionalCurvatureIsSkipped) {
    // Amplitude 1/255 with sigma=200 keeps |lambda1| (== |f_dprime|) far below
    // the 1e-6 guard: every candidate must be rejected by the directional
    // second-derivative guard, yielding no segments and no crash.
    cv::Mat image(64, 64, CV_8UC1, cv::Scalar(0));
    cv::line(image, cv::Point(0, 32), cv::Point(63, 32), cv::Scalar(1), 1);

    StegerConfig config;
    config.sigma = 200.0f;
    config.low_threshold = 1e-9f;
    config.high_threshold = 1e-10f;
    config.min_segment_length = 0.0f;
    StegerRidgeExtractor extractor(config);
    const RidgeGraph graph = extractor.extractRidgeGraph(image);
    EXPECT_TRUE(graph.segments.empty());
}

TEST(StegerExtractorTest, Test_ConfigGettersAndSetters) {
    StegerConfig initial;
    initial.sigma = 2.5f;
    StegerRidgeExtractor extractor(initial);
    EXPECT_FLOAT_EQ(extractor.getConfig().sigma, 2.5f);

    StegerConfig updated;
    updated.sigma = 0.8f;
    updated.extract_dark_lines = true;
    extractor.setConfig(updated);
    EXPECT_FLOAT_EQ(extractor.getConfig().sigma, 0.8f);
    EXPECT_TRUE(extractor.getConfig().extract_dark_lines);
}

TEST(StegerExtractorTest, Test_ExtractOnEmptyFrameReturnsNoEvidence) {
    StegerRidgeExtractor extractor;
    Frame empty_frame(cv::Mat(), "empty", 0);
    EXPECT_TRUE(extractor.extract(empty_frame).empty());
}

TEST(PipelineTest, Test_NullQualityAnalyzerDefaultsToUsable) {
    cv::Mat image(120, 120, CV_8UC1, cv::Scalar(0));
    cv::line(image, cv::Point(10, 60), cv::Point(110, 60), cv::Scalar(255), 3, cv::LINE_AA);
    Frame frame(image, "null_quality", 1);

    StegerConfig config;
    config.low_threshold = 0.5f;
    config.high_threshold = 1.0f;
    config.min_segment_length = 20.0f;
    pipeline::SyncPerceptionPipeline pipeline(nullptr, std::make_shared<StegerRidgeExtractor>(config));
    const auto result = pipeline.process(frame);

    EXPECT_TRUE(result.is_usable);
    EXPECT_FALSE(result.ridge_graph.segments.empty());
    EXPECT_FALSE(result.evidence.empty());
}

TEST(PipelineTest, Test_NullRidgeExtractorSkipsExtraction) {
    cv::Mat image(120, 120, CV_8UC3, cv::Scalar(80, 80, 80));
    Frame frame(image, "null_extractor", 1);

    quality::QualityThresholds lenient;
    lenient.min_blur_score = 0.0;
    lenient.min_brightness = 0.0;
    lenient.min_contrast = 0.0;
    auto analyzer = std::make_shared<quality::LaplacianQualityAnalyzer>(lenient);
    pipeline::SyncPerceptionPipeline pipeline(analyzer, nullptr);
    const auto result = pipeline.process(frame);

    EXPECT_TRUE(result.is_usable);
    EXPECT_TRUE(result.ridge_graph.segments.empty());
    EXPECT_TRUE(result.evidence.empty());
    EXPECT_GT(result.total_duration_us, 0);
}

TEST(PipelineTest, Test_NonStegerExtractorUsesGenericExtractPath) {
    cv::Mat image(120, 120, CV_8UC3, cv::Scalar(80, 80, 80));
    Frame frame(image, "fake_extractor", 1);

    quality::QualityThresholds lenient;
    lenient.min_blur_score = 0.0;
    lenient.min_brightness = 0.0;
    lenient.min_contrast = 0.0;
    auto analyzer = std::make_shared<quality::LaplacianQualityAnalyzer>(lenient);
    pipeline::SyncPerceptionPipeline pipeline(analyzer, std::make_shared<FakeExtractor>());
    const auto result = pipeline.process(frame);

    EXPECT_TRUE(result.is_usable);
    EXPECT_TRUE(result.ridge_graph.segments.empty());
    ASSERT_EQ(result.evidence.size(), 1u);
    EXPECT_EQ(result.evidence[0].evidence_id, "fake_seg_1");
}

TEST(StegerExtractorTest, ExtractFromValidFrameProducesEvidence) {
    cv::Mat canvas = cv::Mat::zeros(100, 100, CV_8UC1);
    cv::line(canvas, cv::Point(10, 50), cv::Point(90, 50), cv::Scalar(255), 3);
    Frame frame(canvas, "test_frame", 1);

    StegerConfig cfg;
    cfg.low_threshold = 0.5f;
    cfg.high_threshold = 1.0f;
    cfg.min_segment_length = 10.0f;
    StegerRidgeExtractor extractor(cfg);

    const auto evidence = extractor.extract(frame);
    EXPECT_FALSE(evidence.empty());
}
