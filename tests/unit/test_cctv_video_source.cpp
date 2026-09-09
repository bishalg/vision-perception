#include <gtest/gtest.h>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include "curv/adapters/CctvVideoSource.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

using namespace CurvEngine;
using namespace CurvEngine::adapters;
using namespace CurvEngine::quality;
using namespace CurvEngine::ridge;

class CctvVideoSourceTest : public ::testing::Test {
protected:
    // Synthetic 30-frame Stage 2 fixture (MJPG AVI, also aliased under cctv_video/).
    static constexpr const char* kClip = "data/fixtures/cctv_video/caltrans_i80_sample.avi";
};

TEST_F(CctvVideoSourceTest, OpenMissingFails) {
    CctvVideoSource source;
    EXPECT_FALSE(source.open("data/fixtures/cctv_video/missing.avi"));
    EXPECT_FALSE(source.isOpened());
    Frame frame;
    EXPECT_FALSE(source.readNext(frame));
}

TEST_F(CctvVideoSourceTest, ReadAllFramesWithDefaultStride) {
    CctvMetadata meta;
    meta.camera_id = "caltrans_i80_sample";
    meta.agency = "Caltrans District 3";
    CctvVideoSource source(meta);
    ASSERT_TRUE(source.open(kClip));
    EXPECT_TRUE(source.isOpened());
    EXPECT_GE(source.frameCount(), 20);

    Frame frame;
    int count = 0;
    while (source.readNext(frame)) {
        ++count;
        EXPECT_EQ(frame.width(), 320);
        EXPECT_EQ(frame.height(), 240);
        EXPECT_EQ(frame.source_id, "caltrans_i80_sample");
        EXPECT_EQ(frame.metadata["agency"], "Caltrans District 3");
        EXPECT_EQ(frame.metadata["fps_stride"], "1");
    }
    EXPECT_EQ(count, 30);
    EXPECT_EQ(source.framesEmitted(), 30u);
    EXPECT_EQ(source.framesDecoded(), 30u);
    source.close();
    EXPECT_FALSE(source.isOpened());
}

TEST_F(CctvVideoSourceTest, FpsStrideSkipsFrames) {
    CctvVideoSource source;
    source.setFpsStride(3);
    EXPECT_EQ(source.fpsStride(), 3);
    ASSERT_TRUE(source.open(kClip));

    Frame frame;
    int count = 0;
    while (source.readNext(frame)) {
        ++count;
    }
    // 30 frames, stride 3 → indices 0,3,6,...,27 → 10 emitted
    EXPECT_EQ(count, 10);
    EXPECT_EQ(source.framesEmitted(), 10u);
    EXPECT_EQ(source.framesDecoded(), 30u);
}

TEST_F(CctvVideoSourceTest, QualityAndStegerOnEmittedFrame) {
    CctvVideoSource source;
    ASSERT_TRUE(source.open(kClip));
    Frame frame;
    ASSERT_TRUE(source.readNext(frame));

    QualityThresholds thresholds;
    thresholds.min_blur_score = 20.0;
    thresholds.min_brightness = 10.0;
    thresholds.max_brightness = 250.0;
    thresholds.min_contrast = 5.0;
    LaplacianQualityAnalyzer analyzer(thresholds);
    QualityReport report = analyzer.analyze(frame);
    EXPECT_TRUE(report.is_usable);

    cv::Mat gray;
    cv::cvtColor(frame.image, gray, cv::COLOR_BGR2GRAY);
    StegerConfig config;
    config.sigma = 1.5f;
    config.low_threshold = 0.5f;
    config.high_threshold = 1.5f;
    StegerRidgeExtractor extractor(config);
    RidgeGraph graph = extractor.extractRidgeGraph(gray);
    EXPECT_FALSE(graph.segments.empty());
}

TEST_F(CctvVideoSourceTest, InvalidStrideClampedToOne) {
    CctvVideoSource source;
    source.setFpsStride(0);
    EXPECT_EQ(source.fpsStride(), 1);
    source.setFpsStride(-5);
    EXPECT_EQ(source.fpsStride(), 1);
}
