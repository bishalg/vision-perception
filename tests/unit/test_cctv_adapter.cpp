#include <gtest/gtest.h>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include "curv/adapters/CctvFrameSource.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

using namespace CurvEngine;
using namespace CurvEngine::adapters;
using namespace CurvEngine::quality;
using namespace CurvEngine::ridge;

class CctvAdapterTest : public ::testing::Test {
protected:
    void SetUp() override {
        QualityThresholds thresholds;
        thresholds.min_blur_score = 60.0;
        thresholds.min_brightness = 15.0;
        thresholds.max_brightness = 245.0;
        thresholds.min_contrast = 10.0;
        analyzer = std::make_unique<LaplacianQualityAnalyzer>(thresholds);
    }

    std::unique_ptr<LaplacianQualityAnalyzer> analyzer;
};

TEST_F(CctvAdapterTest, MetadataDefaultAndCustom) {
    CctvMetadata meta;
    EXPECT_TRUE(meta.camera_id.empty());
    EXPECT_DOUBLE_EQ(meta.latitude, 0.0);
    EXPECT_DOUBLE_EQ(meta.longitude, 0.0);
    EXPECT_DOUBLE_EQ(meta.fov_horizontal_deg, 60.0);

    meta.camera_id = "caltrans_i80";
    meta.name = "I-80 Drum Forebay";
    meta.agency = "Caltrans";
    meta.latitude = 39.3142;
    meta.longitude = -120.7321;
    meta.elevation_m = 1420.0;
    meta.heading_deg = 65.0;
    meta.pitch_deg = -15.0;
    meta.roll_deg = 0.0;
    meta.fov_horizontal_deg = 55.0;

    CctvFrameSource source(meta);
    EXPECT_EQ(source.getMetadata().camera_id, "caltrans_i80");
    EXPECT_EQ(source.getMetadata().name, "I-80 Drum Forebay");
    EXPECT_DOUBLE_EQ(source.getMetadata().latitude, 39.3142);
    EXPECT_DOUBLE_EQ(source.getMetadata().fov_horizontal_deg, 55.0);

    CctvMetadata updated = meta;
    updated.camera_id = "austin_01";
    source.setMetadata(updated);
    EXPECT_EQ(source.getMetadata().camera_id, "austin_01");
}

TEST_F(CctvAdapterTest, LoadNonExistentFileReturnsFalse) {
    CctvFrameSource source;
    Frame frame;
    bool success = source.loadFromFile("non_existent_file_path_12345.png", frame);
    EXPECT_FALSE(success);
    EXPECT_TRUE(frame.empty());
}

TEST_F(CctvAdapterTest, LoadValidDayFixtureAndParseMetadata) {
    CctvMetadata initial_meta;
    initial_meta.camera_id = "caltrans_i80_drum";
    CctvFrameSource source(initial_meta);

    Frame frame;
    const std::string fixture_path = "data/fixtures/cctv/day_clear/caltrans_i80_day.png";
    bool success = source.loadFromFile(fixture_path, frame);

    ASSERT_TRUE(success);
    EXPECT_FALSE(frame.empty());
    EXPECT_EQ(frame.width(), 640);
    EXPECT_EQ(frame.height(), 480);
    EXPECT_EQ(frame.source_id, "caltrans_i80_drum");

    // Verify metadata auto-populated from sidecar JSON or initial metadata
    EXPECT_EQ(frame.metadata["camera_id"], "caltrans_i80_drum");
    EXPECT_FALSE(frame.metadata["agency"].empty());

    // Quality validation
    QualityReport report = source.validateFrame(frame, *analyzer);
    EXPECT_TRUE(report.is_usable);
    EXPECT_GT(report.blur_score, 50.0);
    EXPECT_GT(report.brightness_score, 50.0);
}

TEST_F(CctvAdapterTest, LoadFromMemoryBuffer) {
    CctvMetadata meta;
    meta.camera_id = "mem_cam";
    CctvFrameSource source(meta);

    // Create an in-memory image
    cv::Mat test_img = cv::Mat::ones(100, 100, CV_8UC3) * 128;
    std::vector<uint8_t> encoded_buffer;
    cv::imencode(".png", test_img, encoded_buffer);

    Frame frame;
    bool success = source.loadFromMemory(encoded_buffer, frame);
    ASSERT_TRUE(success);
    EXPECT_FALSE(frame.empty());
    EXPECT_EQ(frame.width(), 100);
    EXPECT_EQ(frame.height(), 100);
    EXPECT_EQ(frame.source_id, "mem_cam");

    // Empty buffer failure test
    std::vector<uint8_t> empty_buffer;
    Frame empty_frame;
    EXPECT_FALSE(source.loadFromMemory(empty_buffer, empty_frame));
    EXPECT_TRUE(empty_frame.empty());

    // Corrupted buffer failure test
    std::vector<uint8_t> corrupted_buffer = {0x00, 0xFF, 0x12, 0x34};
    Frame corrupted_frame;
    EXPECT_FALSE(source.loadFromMemory(corrupted_buffer, corrupted_frame));
    EXPECT_TRUE(corrupted_frame.empty());
}

TEST_F(CctvAdapterTest, ValidateNightAndBlurFixtures) {
    CctvFrameSource source;

    // 1. Night frame
    Frame night_frame;
    ASSERT_TRUE(source.loadFromFile("data/fixtures/cctv/night_lowlux/caltrans_i80_night.png", night_frame));
    QualityReport night_report = source.validateFrame(night_frame, *analyzer);
    EXPECT_LT(night_report.brightness_score, 100.0);

    // 2. Weather rain blur frame
    Frame rain_frame;
    ASSERT_TRUE(source.loadFromFile("data/fixtures/cctv/weather_rain/caltrans_i80_rain_blur.png", rain_frame));
    QualityReport rain_report = source.validateFrame(rain_frame, *analyzer);
    // Blur score should be notably lower on blurred rain frame
    EXPECT_LT(rain_report.blur_score, 300.0);
}

TEST_F(CctvAdapterTest, StegerCurvilinearExtractionOnCctvFrame) {
    CctvFrameSource source;
    Frame frame;
    ASSERT_TRUE(source.loadFromFile("data/fixtures/cctv/day_clear/caltrans_i80_day.png", frame));

    // Convert frame image to grayscale for Steger extraction
    cv::Mat gray;
    cv::cvtColor(frame.image, gray, cv::COLOR_BGR2GRAY);

    StegerConfig config;
    config.sigma = 1.5f;
    config.low_threshold = 0.5f;
    config.high_threshold = 1.5f;
    config.extract_dark_lines = false;

    StegerRidgeExtractor extractor(config);
    RidgeGraph graph = extractor.extractRidgeGraph(gray);

    // Ensure our sub-pixel ridge engine successfully extracts lane lines from the CCTV fixture
    EXPECT_FALSE(graph.segments.empty());
    size_t total_pts = 0;
    for (const auto& seg : graph.segments) {
        total_pts += seg.points.size();
    }
    EXPECT_GT(total_pts, 50);
}
