#include <gtest/gtest.h>

#include "curv/preprocess/FlatFieldCLAHEPreprocessor.hpp"

#include <opencv2/imgproc.hpp>

#include <cmath>
#include <vector>

using CurvEngine::preprocess::FlatFieldCLAHEConfig;
using CurvEngine::preprocess::FlatFieldCLAHEPreprocessor;

namespace {

cv::Mat gradientImage(int size) {
    // Horizontal illumination ramp 100 -> 220 (washed-out right side, dark left edge).
    cv::Mat img(size, size, CV_8UC1);
    for (int y = 0; y < size; ++y) {
        for (int x = 0; x < size; ++x) {
            img.at<uchar>(y, x) = static_cast<uchar>(100 + 120 * x / (size - 1));
        }
    }
    return img;
}

double columnContrast(const cv::Mat& img, int x, int half_band) {
    // Mean |pixel - local background| around a vertical line at column x, where the
    // local background is the average of the columns half_band to each side.
    double sum = 0.0;
    int count = 0;
    for (int y = half_band; y < img.rows - half_band; ++y) {
        const int xl = std::max(0, x - half_band);
        const int xr = std::min(img.cols - 1, x + half_band);
        const double bg = 0.5 * (img.at<uchar>(y, xl) + img.at<uchar>(y, xr));
        sum += std::abs(static_cast<double>(img.at<uchar>(y, x)) - bg);
        count++;
    }
    return count > 0 ? sum / count : 0.0;
}

} // namespace

TEST(FlatFieldCLAHEPreprocessorTest, DisabledConfigReturnsInputUnchanged) {
    FlatFieldCLAHEConfig cfg;
    cfg.enabled = false;
    FlatFieldCLAHEPreprocessor prep(cfg);
    const cv::Mat img = gradientImage(128);
    const cv::Mat out = prep.apply(img, 141.0F);
    EXPECT_EQ(out.type(), img.type());
    EXPECT_EQ(cv::countNonZero(cv::abs(out - img)), 0);
}

TEST(FlatFieldCLAHEPreprocessorTest, FlattensIlluminationGradient) {
    FlatFieldCLAHEPreprocessor prep(FlatFieldCLAHEConfig{});
    const cv::Mat img = gradientImage(256);
    cv::Mat mean, stddev_in, stddev_out;
    cv::meanStdDev(img, mean, stddev_in);
    const cv::Mat out = prep.apply(img, 141.0F);
    cv::meanStdDev(out, mean, stddev_out);
    // The ramp's global spread must collapse after the division.
    EXPECT_LT(stddev_out.at<double>(0), stddev_in.at<double>(0) * 0.5);
}

TEST(FlatFieldCLAHEPreprocessorTest, LiftsFaintLineInBrightZone) {
    // Same faint dark line planted at x=64 (dark zone) and x=192 (bright zone) of the
    // gradient: before preprocessing the bright-zone line is relatively invisible;
    // after flatten+CLAHE both columns should show comparable contrast.
    const int size = 256;
    cv::Mat img = gradientImage(size);
    const uchar depth = 14;
    for (int y = 64; y < size - 64; ++y) {
        img.at<uchar>(y, 64) = static_cast<uchar>(std::max(0, img.at<uchar>(y, 64) - depth));
        img.at<uchar>(y, 192) = static_cast<uchar>(std::max(0, img.at<uchar>(y, 192) - depth));
    }

    const double dark_before = columnContrast(img, 64, 12);
    const double bright_before = columnContrast(img, 192, 12);
    ASSERT_GT(dark_before, 1e-6);

    FlatFieldCLAHEPreprocessor prep(FlatFieldCLAHEConfig{});
    const cv::Mat out = prep.apply(img, 141.0F);
    const double dark_after = columnContrast(out, 64, 12);
    const double bright_after = columnContrast(out, 192, 12);

    // Relative contrast between the two zones must improve markedly after flattening.
    const double ratio_before = bright_before / dark_before;
    const double ratio_after = bright_after / std::max(dark_after, 1e-6);
    EXPECT_LT(ratio_after, ratio_before * 0.75) << "bright-zone line should catch up after flattening";
    EXPECT_GT(bright_after, 2.0) << "CLAHE must leave the faint line visible, not erased";
}

TEST(FlatFieldCLAHEPreprocessorTest, HandlesChannelAndDegenerateInputs) {
    FlatFieldCLAHEPreprocessor prep(FlatFieldCLAHEConfig{});
    // 3-channel input converts cleanly.
    cv::Mat bgr(64, 64, CV_8UC3, cv::Scalar(120, 130, 140));
    const cv::Mat out = prep.apply(bgr, 35.0F);
    EXPECT_EQ(out.type(), CV_8UC1);
    // Empty input returns empty, no crash.
    EXPECT_TRUE(prep.apply(cv::Mat(), 100.0F).empty());
    // Palm width fallback path runs.
    const cv::Mat fallback = prep.apply(gradientImage(96), -1.0F);
    EXPECT_FALSE(fallback.empty());
}

TEST(FlatFieldCLAHEPreprocessorTest, FrameEntryPreservesMetadata) {
    FlatFieldCLAHEPreprocessor prep(FlatFieldCLAHEConfig{});
    cv::Mat img(64, 64, CV_8UC1, cv::Scalar(150));
    CurvEngine::Frame frame(img, "src", 1234);
    frame.metadata["is_left_hand"] = "true";
    const auto out = prep.process(frame);
    EXPECT_EQ(out.metadata.at("is_left_hand"), "true");
    EXPECT_EQ(out.source_id, "src");
    EXPECT_EQ(out.timestamp_ms, 1234);
}
