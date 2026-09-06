#include <cmath>
#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <sstream>
#include <vector>

#include <gtest/gtest.h>

#include "curv/tools/synth_lib.hpp"

namespace {

namespace fs = std::filesystem;
using CurvEngine::tools::addGaussianNoise;
using CurvEngine::tools::drawCurve;
using CurvEngine::tools::runSynthGenerator;
using CurvEngine::tools::sampleGroundTruth;
using CurvEngine::tools::SyntheticParams;
using CurvEngine::tools::writeOutputs;

class SynthLibTest : public ::testing::Test {
protected:
    fs::path makeTempDir(const std::string& tag) {
        const auto dir = fs::temp_directory_path() / ("curv_synth_test_" + tag + "_" +
                                                      std::to_string(::testing::UnitTest::GetInstance()->random_seed()));
        fs::create_directories(dir);
        return dir;
    }
};

TEST_F(SynthLibTest, LineGroundTruthSpansExpectedEndpoints) {
    SyntheticParams p;
    p.width = 400;
    p.height = 200;
    const auto pts = sampleGroundTruth("line", p);
    ASSERT_FALSE(pts.empty());
    // Margin is 0.2 * min(w,h) = 40 px; endpoints at y = 0.3h and y = 0.7h.
    EXPECT_NEAR(pts.front().x, 40.0, 1e-9);
    EXPECT_NEAR(pts.front().y, 60.0, 1e-9);
    EXPECT_NEAR(pts.back().x, 360.0, 1e-9);
    EXPECT_NEAR(pts.back().y, 140.0, 1e-9);
}

TEST_F(SynthLibTest, GroundTruthIsDenselySampled) {
    SyntheticParams p;
    p.width = 400;
    p.height = 200;
    const auto pts = sampleGroundTruth("line", p);
    // Sample step is 0.25 px; length ~ hypot(320, 80) ~ 329.8 -> ~1320 samples.
    const double len = std::hypot(pts.back().x - pts.front().x, pts.back().y - pts.front().y);
    EXPECT_GE(pts.size(), static_cast<size_t>(len / 0.3));
    // Consecutive samples must be tightly spaced.
    for (size_t i = 1; i < pts.size(); ++i) {
        const double d = std::hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
        ASSERT_LE(d, 0.26);
    }
}

TEST_F(SynthLibTest, ParabolaVertexAndEndsFollowTheAnalyticCurve) {
    SyntheticParams p;
    p.width = 400;
    p.height = 400;
    const auto pts = sampleGroundTruth("parabola", p);
    ASSERT_FALSE(pts.empty());
    // Vertex at (0.5w, 0.25h); sag 0.35h at x = x0 +/- 0.35w.
    const double x0 = 200.0;
    const double y0 = 100.0;
    const double a = 140.0 / (140.0 * 140.0);
    EXPECT_NEAR(pts[pts.size() / 2].x, x0, 0.3);
    EXPECT_NEAR(pts[pts.size() / 2].y, y0, 0.3);
    EXPECT_NEAR(pts.front().y, y0 + a * 140.0 * 140.0, 1e-6);
    EXPECT_NEAR(pts.back().y, y0 + a * 140.0 * 140.0, 1e-6);
    // Every sample lies on the analytic curve.
    for (const auto& pt : pts) {
        EXPECT_NEAR(pt.y, y0 + a * (pt.x - x0) * (pt.x - x0), 1e-6);
    }
}

TEST_F(SynthLibTest, SpiralGrowsFromCenterToExpectedRadius) {
    SyntheticParams p;
    p.width = 500;
    p.height = 500;
    const auto pts = sampleGroundTruth("spiral", p);
    ASSERT_FALSE(pts.empty());
    const double cx = 250.0;
    const double cy = 250.0;
    const double r_max = 0.38 * 500.0;
    EXPECT_NEAR(std::hypot(pts.front().x - cx, pts.front().y - cy), 0.0, 1e-6);
    EXPECT_NEAR(std::hypot(pts.back().x - cx, pts.back().y - cy), r_max, 1e-6);
}

TEST_F(SynthLibTest, UnknownCurveTypeYieldsNoSamples) {
    SyntheticParams p;
    EXPECT_TRUE(sampleGroundTruth("hexagon", p).empty());
}

TEST_F(SynthLibTest, DrawCurvePaintsTheCenterlineBrighterThanBackground) {
    SyntheticParams p;
    p.width = 100;
    p.height = 100;
    p.curve_brightness = 220.0;
    p.background_brightness = 20.0;
    p.thickness_px = 3.0;
    cv::Mat img(p.height, p.width, CV_8UC1, cv::Scalar(20));
    auto pts = sampleGroundTruth("line", p);
    // Shrink to fit the tiny frame: use a straight horizontal segment through y=50.
    pts.clear();
    for (double x = 10.0; x <= 90.0; x += 0.25) {
        pts.emplace_back(x, 50.0);
    }
    drawCurve(img, pts, p);
    EXPECT_NEAR(static_cast<double>(img.at<uchar>(50, 50)), 220.0, 36.0);
    EXPECT_NEAR(static_cast<double>(img.at<uchar>(20, 50)), 20.0, 0.5);
}

TEST_F(SynthLibTest, ZeroNoiseLeavesImageUntouched) {
    cv::Mat img(8, 8, CV_8UC1, cv::Scalar(77));
    cv::Mat reference = img.clone();
    addGaussianNoise(img, 0.0, 12345);
    EXPECT_EQ(cv::countNonZero(img != reference), 0);
}

TEST_F(SynthLibTest, NoiseIsDeterministicPerSeedAndDifferentAcrossSeeds) {
    cv::Mat a(64, 64, CV_8UC1, cv::Scalar(100));
    cv::Mat b = a.clone();
    cv::Mat c = a.clone();
    addGaussianNoise(a, 8.0, 1);
    addGaussianNoise(b, 8.0, 1);
    addGaussianNoise(c, 8.0, 2);
    EXPECT_EQ(cv::countNonZero(a != b), 0);          // same seed -> identical noise
    EXPECT_GT(cv::countNonZero(a != c), 0);          // different seed -> different noise
}

TEST_F(SynthLibTest, WriteOutputsProducesImageAndTruthFiles) {
    SyntheticParams p;
    p.width = 64;
    p.height = 64;
    const auto dir = makeTempDir("write");
    cv::Mat img(p.height, p.width, CV_8UC1, cv::Scalar(50));
    const auto pts = sampleGroundTruth("line", p);
    ASSERT_TRUE(writeOutputs("line", dir, img, pts, p));
    EXPECT_TRUE(fs::exists(dir / "line_image.png"));
    ASSERT_TRUE(fs::exists(dir / "line_truth.json"));
    std::ifstream in(dir / "line_truth.json");
    nlohmann::json doc = nlohmann::json::parse(in);
    EXPECT_EQ(doc["curve_type"].get<std::string>(), "line");
    EXPECT_EQ(doc["width"].get<int>(), 64);
    EXPECT_EQ(doc["image"].get<std::string>(), "line_image.png");
    ASSERT_TRUE(doc.contains("ground_truth_points"));
    EXPECT_EQ(doc["ground_truth_points"].size(), pts.size());
    fs::remove_all(dir);
}

TEST_F(SynthLibTest, WriteOutputsFailsGracefullyOnUnwritablePath) {
    SyntheticParams p;
    cv::Mat img(8, 8, CV_8UC1, cv::Scalar(0));
    // A path "under" a regular file can never be created.
    const auto dir = makeTempDir("blocked");
    fs::create_directories(dir);
    const auto blocker = dir / "blocker";
    { std::ofstream f(blocker); f << "x"; }
    std::stringstream err;
    EXPECT_FALSE(writeOutputs("line", blocker, img, {}, p, &err));
    EXPECT_NE(err.str().find("Failed to write"), std::string::npos);
    fs::remove_all(dir);
}

TEST_F(SynthLibTest, CliRunsEndToEndIntoTempDir) {
    const auto dir = makeTempDir("cli");
    std::string out_dir = (dir / "out").string();
    const char* argv[] = {"synth_generator", "--type", "line", "--width", "64", "--height", "64",
                          "--out-dir", out_dir.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(runSynthGenerator(9, const_cast<char**>(argv), out, err), 0);
    EXPECT_TRUE(fs::exists(dir / "out" / "line_image.png"));
    EXPECT_TRUE(fs::exists(dir / "out" / "line_truth.json"));
    EXPECT_NE(out.str().find("[PASS]"), std::string::npos);
    fs::remove_all(dir);
}

TEST_F(SynthLibTest, CliAcceptsKnownFlagsAndAppliesThem) {
    const auto dir = makeTempDir("flags");
    std::string out_dir = (dir / "out").string();
    const char* argv[] = {"synth_generator", "--type",  "parabola", "--width",   "48",  "--height", "48",
                          "--thickness",    "2",      "--noise",  "3.0",       "--seed", "7",
                          "--out-dir",      out_dir.c_str()};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(runSynthGenerator(15, const_cast<char**>(argv), out, err), 0);
    std::ifstream in(dir / "out" / "parabola_truth.json");
    nlohmann::json doc = nlohmann::json::parse(in);
    EXPECT_EQ(doc["thickness_px"].get<double>(), 2.0);
    EXPECT_EQ(doc["noise_sigma"].get<double>(), 3.0);
    EXPECT_EQ(doc["seed"].get<uint64_t>(), 7u);
    fs::remove_all(dir);
}

TEST_F(SynthLibTest, CliHelpExitsZeroAndUnknownArgExitsOne) {
    const char* help[] = {"synth_generator", "--help"};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(runSynthGenerator(2, const_cast<char**>(help), out, err), 0);
    EXPECT_NE(out.str().find("Usage"), std::string::npos);

    const char* bad[] = {"synth_generator", "--frobnicate"};
    EXPECT_EQ(runSynthGenerator(2, const_cast<char**>(bad), out, err), 1);
}

TEST_F(SynthLibTest, CliRejectsNonPositiveDimensions) {
    const char* argv[] = {"synth_generator", "--width", "0", "--height", "10"};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(runSynthGenerator(5, const_cast<char**>(argv), out, err), 1);
    EXPECT_NE(err.str().find("positive"), std::string::npos);
}

TEST_F(SynthLibTest, CliRejectsMalformedNumericValuesWithClearError) {
    const char* argv[] = {"synth_generator", "--width", "abc"};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(runSynthGenerator(3, const_cast<char**>(argv), out, err), 1);
    EXPECT_NE(err.str().find("Invalid numeric value for --width"), std::string::npos);

    const char* argv2[] = {"synth_generator", "--noise", "lots"};
    EXPECT_EQ(runSynthGenerator(3, const_cast<char**>(argv2), out, err), 1);
    EXPECT_NE(err.str().find("Invalid numeric value for --noise"), std::string::npos);
}

TEST_F(SynthLibTest, CliRejectsUnknownCurveType) {
    const auto dir = makeTempDir("badtype");
    std::string out_dir = (dir / "out").string();
    const char* argv[] = {"synth_generator", "--type", "hexagon", "--out-dir", out_dir.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(runSynthGenerator(5, const_cast<char**>(argv), out, err), 1);
    EXPECT_NE(err.str().find("Unknown curve type"), std::string::npos);
    fs::remove_all(dir);
}

} // namespace
