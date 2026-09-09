#include <gtest/gtest.h>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

#include "curv/adapters/CctvHttpSnapshot.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"

using namespace CurvEngine;
using namespace CurvEngine::adapters;
using namespace CurvEngine::quality;

namespace {

std::string uniqueCacheDir() {
    const auto stamp = std::chrono::steady_clock::now().time_since_epoch().count();
    return (std::filesystem::temp_directory_path() / ("curv_http_cache_" + std::to_string(stamp))).string();
}

std::vector<std::uint8_t> readFileBytes(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    return {std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
}

} // namespace

TEST(CctvHttpSnapshotTest, ResolveMockAndRegistry) {
    EXPECT_EQ(CctvHttpSnapshot::resolveCameraUrl("mock://caltrans_i80_day"), "mock://caltrans_i80_day");
    EXPECT_NE(CctvHttpSnapshot::resolveCameraUrl("caltrans_i80_drum").find("http"), std::string::npos);
    EXPECT_EQ(CctvHttpSnapshot::resolveCameraUrl("bhote_koshi_pier"), "mock://bhote_koshi_pier");
}

TEST(CctvHttpSnapshotTest, FetchMockFixture) {
    CctvMetadata meta;
    meta.camera_id = "mock_cam";
    CctvHttpSnapshot snap(meta);
    Frame frame;
    CctvHttpSnapshot::FetchStats stats;
    ASSERT_TRUE(snap.fetch("mock://caltrans_i80_day", frame, &stats));
    EXPECT_TRUE(stats.from_mock);
    EXPECT_EQ(frame.width(), 640);
    EXPECT_EQ(frame.height(), 480);
    EXPECT_EQ(frame.source_id, "mock_cam");

    QualityThresholds thresholds;
    thresholds.min_blur_score = 20.0;
    thresholds.min_brightness = 10.0;
    thresholds.max_brightness = 250.0;
    thresholds.min_contrast = 5.0;
    LaplacianQualityAnalyzer analyzer(thresholds);
    EXPECT_TRUE(analyzer.analyze(frame).is_usable);
}

TEST(CctvHttpSnapshotTest, FetchFileUrl) {
    CctvHttpSnapshot snap;
    Frame frame;
    CctvHttpSnapshot::FetchStats stats;
    ASSERT_TRUE(snap.fetch("file://data/fixtures/cctv/night_lowlux/caltrans_i80_night.png", frame, &stats));
    EXPECT_FALSE(stats.from_cache);
    EXPECT_EQ(frame.width(), 640);
}

TEST(CctvHttpSnapshotTest, UnknownMockFails) {
    CctvHttpSnapshot snap;
    Frame frame;
    CctvHttpSnapshot::FetchStats stats;
    EXPECT_FALSE(snap.fetch("mock://does_not_exist", frame, &stats));
    EXPECT_FALSE(stats.error.empty());
    EXPECT_TRUE(frame.empty());
}

TEST(CctvHttpSnapshotTest, PreferCacheServesSeededBytes) {
    const std::string cache_dir = uniqueCacheDir();
    const std::string fake_url = "https://example.invalid/cctv/test.jpg";
    const auto body = readFileBytes("data/fixtures/cctv/day_clear/caltrans_i80_day.png");
    ASSERT_FALSE(body.empty());

    CctvHttpSnapshot snap;
    snap.setCacheDir(cache_dir);
    snap.setPreferCache(true);
    ASSERT_TRUE(snap.putCache(fake_url, body));

    Frame frame;
    CctvHttpSnapshot::FetchStats stats;
    ASSERT_TRUE(snap.fetch(fake_url, frame, &stats));
    EXPECT_TRUE(stats.from_cache);
    EXPECT_EQ(frame.width(), 640);
    EXPECT_EQ(frame.height(), 480);

    std::filesystem::remove_all(cache_dir);
}

TEST(CctvHttpSnapshotTest, BareFixturePathWorks) {
    CctvHttpSnapshot snap;
    Frame frame;
    ASSERT_TRUE(snap.fetch("data/fixtures/cctv/weather_rain/caltrans_i80_rain_blur.png", frame));
    EXPECT_FALSE(frame.empty());
}
