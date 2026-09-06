#include <gtest/gtest.h>
#include <opencv2/imgproc.hpp>
#include "curv/quality/LaplacianQualityAnalyzer.hpp"

using namespace CurvEngine;
using namespace CurvEngine::quality;

class QualityAnalyzerTest : public ::testing::Test {
protected:
    void SetUp() override {
        QualityThresholds thresholds;
        thresholds.min_blur_score = 80.0;
        thresholds.min_brightness = 20.0;
        thresholds.max_brightness = 240.0;
        thresholds.min_contrast = 15.0;
        analyzer = std::make_unique<LaplacianQualityAnalyzer>(thresholds);
    }

    std::unique_ptr<LaplacianQualityAnalyzer> analyzer;
};

TEST_F(QualityAnalyzerTest, Test_SharpCheckerboardImage) {
    // Generate a 500x500 high-contrast high-frequency checkerboard pattern
    cv::Mat checker = cv::Mat::zeros(500, 500, CV_8UC1);
    const int block_size = 20;
    for (int y = 0; y < 500; ++y) {
        for (int x = 0; x < 500; ++x) {
            if (((x / block_size) + (y / block_size)) % 2 == 0) {
                checker.at<uint8_t>(y, x) = 255;
            }
        }
    }

    Frame frame(checker, "test_sharp_card", 1001);
    QualityReport report = analyzer->analyze(frame);

    EXPECT_TRUE(report.is_usable);
    EXPECT_GT(report.blur_score, 500.0);
    EXPECT_GT(report.brightness_score, 100.0);
    EXPECT_LT(report.brightness_score, 150.0);
    EXPECT_GT(report.contrast_score, 50.0);
}

TEST_F(QualityAnalyzerTest, Test_BlurredImage) {
    // Generate checkerboard then apply heavy Gaussian blur
    cv::Mat checker = cv::Mat::zeros(500, 500, CV_8UC1);
    const int block_size = 20;
    for (int y = 0; y < 500; ++y) {
        for (int x = 0; x < 500; ++x) {
            if (((x / block_size) + (y / block_size)) % 2 == 0) {
                checker.at<uint8_t>(y, x) = 255;
            }
        }
    }

    cv::Mat blurred;
    cv::GaussianBlur(checker, blurred, cv::Size(45, 45), 15.0);

    Frame frame(blurred, "test_blurred_card", 1002);
    QualityReport report = analyzer->analyze(frame);

    EXPECT_FALSE(report.is_usable);
    EXPECT_LT(report.blur_score, analyzer->getThresholds().min_blur_score);
    EXPECT_NE(report.recommendation.find("blurry"), std::string::npos);
}

TEST_F(QualityAnalyzerTest, Test_DarkUnderexposedImage) {
    // Uniform dark image with low intensity (e.g. mean ~5)
    cv::Mat dark(500, 500, CV_8UC1, cv::Scalar(5));

    Frame frame(dark, "test_dark_card", 1003);
    QualityReport report = analyzer->analyze(frame);

    EXPECT_FALSE(report.is_usable);
    EXPECT_LT(report.brightness_score, analyzer->getThresholds().min_brightness);
    EXPECT_NE(report.recommendation.find("dark"), std::string::npos);
}

TEST_F(QualityAnalyzerTest, Test_OverexposedImage) {
    // Uniform saturated white image (mean ~252)
    cv::Mat bright(500, 500, CV_8UC1, cv::Scalar(252));

    Frame frame(bright, "test_bright_card", 1004);
    QualityReport report = analyzer->analyze(frame);

    EXPECT_FALSE(report.is_usable);
    EXPECT_GT(report.brightness_score, analyzer->getThresholds().max_brightness);
    EXPECT_NE(report.recommendation.find("bright"), std::string::npos);
}

TEST_F(QualityAnalyzerTest, Test_EmptyFrame) {
    Frame empty_frame;
    QualityReport report = analyzer->analyze(empty_frame);

    EXPECT_FALSE(report.is_usable);
    EXPECT_NE(report.recommendation.find("empty"), std::string::npos);
}

// === Step 4 coverage-gap tests (characterization) ===

TEST_F(QualityAnalyzerTest, Test_FourChannelFrameIsAccepted) {
    // BGRA frames take the COLOR_BGRA2GRAY conversion path.
    cv::Mat image(120, 120, CV_8UC4, cv::Scalar(90, 90, 90, 255));
    cv::line(image, cv::Point(10, 60), cv::Point(110, 60), cv::Scalar(255, 255, 255, 255), 3, cv::LINE_AA);
    Frame frame(image, "bgra_frame", 1);

    quality::QualityThresholds thresholds;
    thresholds.min_blur_score = 10.0;
    thresholds.min_brightness = 10.0;
    thresholds.min_contrast = 5.0;
    quality::LaplacianQualityAnalyzer local_analyzer(thresholds);

    const auto report = local_analyzer.analyze(frame);
    EXPECT_TRUE(report.is_usable);
    EXPECT_GT(report.blur_score, thresholds.min_blur_score);
}

TEST_F(QualityAnalyzerTest, Test_TwoChannelFrameIsRejected) {
    // Channel counts other than 1/3/4 are explicitly unsupported.
    cv::Mat image(64, 64, CV_8UC2, cv::Scalar(100, 100));
    Frame frame(image, "weird_frame", 1);

    quality::LaplacianQualityAnalyzer local_analyzer;
    const auto report = local_analyzer.analyze(frame);
    EXPECT_FALSE(report.is_usable);
    EXPECT_NE(report.recommendation.find("Unsupported channel count"), std::string::npos);
    EXPECT_NEAR(report.blur_score, 0.0, 1e-9);
}

TEST_F(QualityAnalyzerTest, Test_ThresholdGettersAndSetters) {
    quality::LaplacianQualityAnalyzer local_analyzer;
    EXPECT_DOUBLE_EQ(local_analyzer.getThresholds().min_blur_score, 80.0);

    quality::QualityThresholds updated;
    updated.min_blur_score = 5.0;
    updated.max_brightness = 200.0;
    local_analyzer.setThresholds(updated);
    EXPECT_DOUBLE_EQ(local_analyzer.getThresholds().min_blur_score, 5.0);
    EXPECT_DOUBLE_EQ(local_analyzer.getThresholds().max_brightness, 200.0);
}
