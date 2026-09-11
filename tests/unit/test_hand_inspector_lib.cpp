#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include <sstream>

#include <gtest/gtest.h>

#include "curv/tools/hand_inspector_lib.hpp"

namespace {

namespace fs = std::filesystem;

class HandInspectorLibTest : public ::testing::Test {
protected:
    fs::path dir;
    void SetUp() override {
        dir = fs::temp_directory_path() /
              ("curv_hand_test_" + std::to_string(::testing::UnitTest::GetInstance()->random_seed()) + "_" +
               std::to_string(++counter));
        fs::create_directories(dir);
    }
    void TearDown() override { fs::remove_all(dir); }
    static int counter;

    fs::path makePalmImage() {
        // Textured palm-like image: bright palm blob on dark background with creases.
        cv::Mat img(320, 320, CV_8UC1, cv::Scalar(25));
        cv::ellipse(img, cv::Point(160, 190), cv::Size(90, 110), 0, 0, 360, cv::Scalar(200), -1, cv::LINE_AA);
        for (int y : {130, 175, 220}) {
            cv::line(img, cv::Point(85, y), cv::Point(235, y), cv::Scalar(40), 3, cv::LINE_AA);
        }
        const auto path = dir / "palm.png";
        cv::cvtColor(img, img, cv::COLOR_GRAY2BGR);
        EXPECT_TRUE(cv::imwrite(path.string(), img));
        return path;
    }

    fs::path writeLandmarks(const nlohmann::json& js, const std::string& name) {
        const auto path = dir / name;
        std::ofstream f(path);
        f << js.dump();
        return path;
    }
};
int HandInspectorLibTest::counter = 0;

TEST_F(HandInspectorLibTest, NoImagePrintsUsageAndSucceeds) {
    std::stringstream out;
    std::stringstream err;
    const char* argv[] = {"hand_inspector_cli"};
    EXPECT_EQ(CurvEngine::tools::runHandInspector(1, const_cast<char**>(argv), out, err), 0);
    EXPECT_NE(out.str().find("Usage"), std::string::npos);
}

TEST_F(HandInspectorLibTest, MissingImageFileFailsWithExitCodeOne) {
    std::stringstream out;
    std::stringstream err;
    const std::string missing = (dir / "nope.png").string();
    const char* argv[] = {"hand_inspector_cli", "--image", missing.c_str()};
    EXPECT_EQ(CurvEngine::tools::runHandInspector(3, const_cast<char**>(argv), out, err), 1);
    EXPECT_NE(err.str().find("Could not load image"), std::string::npos);
}

TEST_F(HandInspectorLibTest, FullRunWithHeuristicLandmarksProducesAllOutputs) {
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string warp = (dir / "warp.png").string();
    const std::string overlay = (dir / "overlay.png").string();
    const std::string evidence = (dir / "evidence.json").string();
    const std::string prompt = (dir / "prompt.json").string();

    const char* argv[] = {"hand_inspector_cli", "--image",     image_arg.c_str(), "--out-warp",   warp.c_str(),
                          "--out-overlay",      overlay.c_str(), "--out-json",   evidence.c_str(),
                          "--out-vlm-prompt",   prompt.c_str()};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runHandInspector(11, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(warp));
    EXPECT_TRUE(fs::exists(overlay));
    ASSERT_TRUE(fs::exists(evidence));
    ASSERT_TRUE(fs::exists(prompt));

    nlohmann::json ev;
    { std::ifstream f(evidence); f >> ev; }
    EXPECT_TRUE(ev.contains("quality"));
    EXPECT_TRUE(ev.contains("domain_payload"));
    EXPECT_TRUE(ev.contains("evidence"));
    EXPECT_TRUE(ev["domain_payload"].contains("major_lines"));
    EXPECT_TRUE(ev["domain_payload"].contains("palm_analysis"));

    nlohmann::json pr;
    { std::ifstream f(prompt); f >> pr; }
    EXPECT_TRUE(pr.contains("contents")); // Gemini request shape
    const std::string prompt_str = pr["contents"][0]["parts"][0]["text"].get<std::string>();
    EXPECT_EQ(prompt_str.find("line_candidate"), std::string::npos); // Prompt is clean
    EXPECT_NE(out.str().find("[PASS]"), std::string::npos);
}

TEST_F(HandInspectorLibTest, DebugAllRidgesFlagEnablesRawEvidence) {
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string warp = (dir / "warp_dbg.png").string();
    const std::string overlay = (dir / "overlay_dbg.png").string();
    const std::string evidence = (dir / "evidence_dbg.json").string();
    const std::string prompt = (dir / "prompt_dbg.json").string();

    const char* argv[] = {"hand_inspector_cli", "--image", image_arg.c_str(),
                          "--debug-all-ridges",
                          "--out-warp", warp.c_str(), "--out-overlay", overlay.c_str(),
                          "--out-json", evidence.c_str(), "--out-vlm-prompt", prompt.c_str()};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runHandInspector(12, const_cast<char**>(argv), out, err), 0) << err.str();

    nlohmann::json ev;
    { std::ifstream f(evidence); f >> ev; }
    EXPECT_TRUE(ev.contains("evidence"));

    bool has_line_candidate = false;
    for (const auto& item : ev["evidence"]) {
        if (item.value("type", "") == "line_candidate") {
            has_line_candidate = true;
            break;
        }
    }
    EXPECT_TRUE(has_line_candidate);
}


TEST_F(HandInspectorLibTest, ObjectFormLandmarksJsonIsLoaded) {
    nlohmann::json lms = nlohmann::json::array();
    for (int i = 0; i < 21; ++i) {
        const double x = 0.3 + 0.4 * (i % 5) / 4.0;
        const double y = 0.3 + 0.4 * (i / 5) / 4.0;
        lms.push_back({{"x", x}, {"y", y}});
    }
    const auto lm_path = writeLandmarks(lms, "lms_obj.json").string();
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string warp = (dir / "warp_obj.png").string();
    const std::string overlay = (dir / "overlay_obj.png").string();

    const char* argv[] = {"hand_inspector_cli", "--image",     image_arg.c_str(), "--landmarks", lm_path.c_str(),
                          "--out-warp",         warp.c_str(),  "--out-overlay",   overlay.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runHandInspector(9, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(warp));
    EXPECT_TRUE(out.str().find("Loaded landmarks from") != std::string::npos ||
                out.str().find("Loading user landmarks") != std::string::npos);
}

TEST_F(HandInspectorLibTest, ArrayFormLandmarksJsonIsLoaded) {
    nlohmann::json lms = nlohmann::json::array();
    for (int i = 0; i < 21; ++i) {
        lms.push_back({0.35 + 0.01 * i, 0.4 + 0.01 * (i % 7)});
    }
    const auto lm_path = writeLandmarks(lms, "lms_arr.json").string();
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string warp = (dir / "warp_arr.png").string();

    const char* argv[] = {"hand_inspector_cli", "--image", image_arg.c_str(), "--landmarks", lm_path.c_str(),
                          "--out-warp",         warp.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runHandInspector(7, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(warp));
}

TEST_F(HandInspectorLibTest, MalformedLandmarksFileFallsBackGracefully) {
    // Nonexistent landmarks path: the loader stays silent and the run continues
    // with default landmarks rather than crashing.
    const std::string lm_path = (dir / "missing.json").string();
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string warp = (dir / "warp_fallback.png").string();

    const char* argv[] = {"hand_inspector_cli", "--image", image_arg.c_str(), "--landmarks", lm_path.c_str(),
                          "--out-warp",         warp.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runHandInspector(7, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(warp));
}

TEST_F(HandInspectorLibTest, AutoLandmarksWithGarbageModelFallsBackToHeuristic) {
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string model = (dir / "not_a_model.onnx").string();
    { std::ofstream f(model); f << "garbage"; }
    const std::string warp = (dir / "warp_auto.png").string();

    const char* argv[] = {"hand_inspector_cli", "--image", image_arg.c_str(), "--auto-landmarks", model.c_str(),
                          "--out-warp",         warp.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runHandInspector(7, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(warp));
    EXPECT_NE(out.str().find("hand landmark detector"), std::string::npos);
}

TEST_F(HandInspectorLibTest, AutoLandmarksBareFlagConsumesNoPath) {
    // Bare --auto-landmarks (next arg is another flag) must not swallow it.
    const auto image = makePalmImage();
    const std::string image_arg = image.string();
    const std::string warp = (dir / "warp_bare.png").string();

    const char* argv[] = {"hand_inspector_cli", "--image", image_arg.c_str(), "--auto-landmarks",
                          "--out-warp",         warp.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runHandInspector(6, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(warp));
}

} // namespace
