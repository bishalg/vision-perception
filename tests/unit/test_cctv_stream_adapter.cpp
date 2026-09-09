#include <gtest/gtest.h>
#include <atomic>
#include <chrono>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include <thread>
#include "curv/adapters/CctvStreamAdapter.hpp"
#include "curv/adapters/FrameRingBuffer.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

using namespace CurvEngine;
using namespace CurvEngine::adapters;
using namespace CurvEngine::quality;
using namespace CurvEngine::ridge;

namespace {

Frame makeSolidFrame(int w, int h, std::uint8_t value, const std::string& id = "inject") {
    cv::Mat img(h, w, CV_8UC3, cv::Scalar(value, value, value));
    return Frame(img, id, 0);
}

} // namespace

TEST(FrameRingBufferTest, PushTakeLatestDropsStale) {
    FrameRingBuffer ring(3);
    EXPECT_EQ(ring.capacity(), 3u);
    EXPECT_TRUE(ring.empty());

    EXPECT_EQ(ring.push(makeSolidFrame(8, 8, 10, "a")), FrameRingBuffer::PushResult::Inserted);
    EXPECT_EQ(ring.push(makeSolidFrame(8, 8, 20, "b")), FrameRingBuffer::PushResult::Inserted);
    EXPECT_EQ(ring.push(makeSolidFrame(8, 8, 30, "c")), FrameRingBuffer::PushResult::Inserted);
    EXPECT_EQ(ring.size(), 3u);

    EXPECT_EQ(ring.push(makeSolidFrame(8, 8, 40, "d")), FrameRingBuffer::PushResult::Overwritten);
    EXPECT_EQ(ring.size(), 3u);

    auto latest = ring.takeLatest();
    ASSERT_TRUE(latest.has_value());
    EXPECT_EQ(latest->source_id, "d");
    EXPECT_TRUE(ring.empty());
    EXPECT_FALSE(ring.takeLatest().has_value());
}

TEST(FrameRingBufferTest, ZeroCapacityFailsPush) {
    FrameRingBuffer ring(0);
    EXPECT_EQ(ring.capacity(), 0u);
    EXPECT_EQ(ring.push(makeSolidFrame(4, 4, 1)), FrameRingBuffer::PushResult::Failed);
    EXPECT_TRUE(ring.empty());
}

TEST(FrameRingBufferTest, ClearEmptiesBuffer) {
    FrameRingBuffer ring(2);
    ASSERT_EQ(ring.push(makeSolidFrame(4, 4, 1)), FrameRingBuffer::PushResult::Inserted);
    ring.clear();
    EXPECT_TRUE(ring.empty());
    EXPECT_EQ(ring.size(), 0u);
}

class CctvStreamAdapterTest : public ::testing::Test {
protected:
    static constexpr const char* kClipPath = "data/fixtures/cctv/video/caltrans_i80_day_clip.avi";
};

TEST_F(CctvStreamAdapterTest, OpenMissingFileFails) {
    CctvStreamAdapter stream;
    EXPECT_FALSE(stream.openFile("data/fixtures/cctv/video/does_not_exist.avi"));
    EXPECT_FALSE(stream.isOpened());
    EXPECT_FALSE(stream.start());
}

TEST_F(CctvStreamAdapterTest, InjectAndConsumeWithBackpressure) {
    CctvMetadata meta;
    meta.camera_id = "inject_cam";
    meta.agency = "TestAgency";
    CctvStreamAdapter stream(meta, /*ring_capacity=*/2);

    ASSERT_TRUE(stream.injectFrame(makeSolidFrame(16, 16, 50)));
    ASSERT_TRUE(stream.injectFrame(makeSolidFrame(16, 16, 60)));
    ASSERT_TRUE(stream.injectFrame(makeSolidFrame(16, 16, 70))); // overwrite
    EXPECT_GE(stream.framesOverwritten(), 1u);
    EXPECT_EQ(stream.framesCaptured(), 3u);

    Frame out;
    ASSERT_TRUE(stream.tryLatestFrame(out));
    EXPECT_EQ(out.source_id, "inject_cam");
    EXPECT_EQ(out.metadata["agency"], "TestAgency");
    EXPECT_FALSE(out.metadata["stream_sequence"].empty());
    EXPECT_FALSE(stream.tryLatestFrame(out));
}

TEST_F(CctvStreamAdapterTest, WaitLatestFrameTimeout) {
    CctvStreamAdapter stream;
    Frame out;
    EXPECT_FALSE(stream.waitLatestFrame(out, std::chrono::milliseconds(20)));
}

TEST_F(CctvStreamAdapterTest, WaitLatestFrameReceivesInject) {
    CctvStreamAdapter stream;
    std::atomic<bool> injected{false};
    std::thread producer([&stream, &injected]() {
        std::this_thread::sleep_for(std::chrono::milliseconds(30));
        injected.store(stream.injectFrame(makeSolidFrame(8, 8, 90, "late")), std::memory_order_release);
    });

    Frame out;
    ASSERT_TRUE(stream.waitLatestFrame(out, std::chrono::milliseconds(500)));
    producer.join();
    EXPECT_TRUE(injected.load(std::memory_order_acquire));
}

TEST_F(CctvStreamAdapterTest, PlaybackRecordedClipAndExtractRidges) {
    CctvMetadata meta;
    meta.camera_id = "caltrans_i80_clip";
    meta.name = "Stage2 Clip";
    meta.agency = "Caltrans District 3";
    meta.latitude = 39.3142;
    meta.longitude = -120.7321;
    meta.stream_or_snapshot_url = "file://synthetic_stage2_clip";

    CctvStreamAdapter stream(meta, /*ring_capacity=*/8);
    ASSERT_TRUE(stream.openFile(kClipPath)) << "Open recorded fixture " << kClipPath;
    EXPECT_TRUE(stream.isOpened());
    ASSERT_TRUE(stream.start());
    EXPECT_TRUE(stream.isRunning());

    Frame frame;
    bool got = false;
    for (int attempt = 0; attempt < 50; ++attempt) {
        if (stream.waitLatestFrame(frame, std::chrono::milliseconds(100))) {
            got = true;
            break;
        }
    }
    ASSERT_TRUE(got) << "Timed out waiting for first decoded frame";
    EXPECT_EQ(frame.width(), 320);
    EXPECT_EQ(frame.height(), 240);
    EXPECT_EQ(frame.source_id, "caltrans_i80_clip");
    EXPECT_EQ(frame.metadata["agency"], "Caltrans District 3");
    EXPECT_FALSE(frame.metadata["stream_or_snapshot_url"].empty());

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
    config.extract_dark_lines = false;
    StegerRidgeExtractor extractor(config);
    RidgeGraph graph = extractor.extractRidgeGraph(gray);
    EXPECT_FALSE(graph.segments.empty());

    // Allow capture to drain; stop must be safe mid-stream.
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    stream.stop();
    EXPECT_FALSE(stream.isRunning());
    EXPECT_GT(stream.framesCaptured(), 0u);
    stream.close();
    EXPECT_FALSE(stream.isOpened());
}

TEST_F(CctvStreamAdapterTest, LoopPlaybackProducesMoreThanFileLength) {
    CctvStreamAdapter stream;
    stream.setLoopPlayback(true);
    EXPECT_TRUE(stream.loopPlayback());
    ASSERT_TRUE(stream.openFile(kClipPath));
    ASSERT_TRUE(stream.start());

    // Clip has 30 frames; wait until capture counter exceeds that.
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (std::chrono::steady_clock::now() < deadline) {
        if (stream.framesCaptured() > 35u) {
            break;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    EXPECT_GT(stream.framesCaptured(), 35u) << "Loop playback should rewind past EOF";
    stream.stop();
}

TEST_F(CctvStreamAdapterTest, OpenUrlEmptyFails) {
    CctvStreamAdapter stream;
    EXPECT_FALSE(stream.openUrl(""));
}
