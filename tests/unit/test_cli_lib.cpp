#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <opencv2/imgcodecs.hpp>
#include <sstream>

#include <gtest/gtest.h>

#include "curv/tools/cli_lib.hpp"
#include "curv/tools/synth_lib.hpp"

namespace {

namespace fs = std::filesystem;

class CliLibTest : public ::testing::Test {
protected:
    fs::path dir;
    void SetUp() override {
        dir = fs::temp_directory_path() /
              ("curv_cli_test_" + std::to_string(::testing::UnitTest::GetInstance()->random_seed()) + "_" +
               std::to_string(++counter));
        fs::create_directories(dir);
    }
    void TearDown() override { fs::remove_all(dir); }
    static int counter;

    // Generates a real synthetic ridge image into the test dir via synth_lib.
    fs::path makeLineImage() {
        CurvEngine::tools::SyntheticParams p;
        p.width = 256;
        p.height = 256;
        const auto pts = CurvEngine::tools::sampleGroundTruth("line", p);
        cv::Mat img(p.height, p.width, CV_8UC1, cv::Scalar(20));
        CurvEngine::tools::drawCurve(img, pts, p);
        const auto path = dir / "input_line.png";
        EXPECT_TRUE(cv::imwrite(path.string(), img));
        return path;
    }
};
int CliLibTest::counter = 0;

TEST_F(CliLibTest, NoImagePrintsUsageAndSucceeds) {
    std::stringstream out;
    std::stringstream err;
    const char* argv[] = {"curv_cli"};
    EXPECT_EQ(CurvEngine::tools::runCurvCli(1, const_cast<char**>(argv), out, err), 0);
    EXPECT_NE(out.str().find("Usage"), std::string::npos);
    EXPECT_NE(out.str().find("sanity checks"), std::string::npos);
}

TEST_F(CliLibTest, BenchmarkFlagRunsAndSucceeds) {
    std::stringstream out;
    std::stringstream err;
    const char* argv[] = {"curv_cli", "--benchmark"};
    ASSERT_EQ(CurvEngine::tools::runCurvCli(2, const_cast<char**>(argv), out, err), 0);
    EXPECT_NE(out.str().find("Performance Benchmark"), std::string::npos);
}

TEST_F(CliLibTest, MissingImageFileFailsWithExitCodeOne) {
    std::stringstream out;
    std::stringstream err;
    const std::string missing = (dir / "nope.png").string();
    const char* argv[] = {"curv_cli", "--image", missing.c_str()};
    EXPECT_EQ(CurvEngine::tools::runCurvCli(3, const_cast<char**>(argv), out, err), 1);
    EXPECT_NE(err.str().find("Failed to load image"), std::string::npos);
}

TEST_F(CliLibTest, FullPipelineRunWritesOverlayAndEvidence) {
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string overlay = (dir / "overlay.png").string();
    const std::string evidence = (dir / "evidence.json").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out", overlay.c_str(),
                          "--json",  evidence.c_str()};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runCurvCli(7, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_TRUE(fs::exists(overlay));
    ASSERT_TRUE(fs::exists(evidence));

    std::ifstream in(evidence);
    nlohmann::json doc = nlohmann::json::parse(in);
    EXPECT_TRUE(doc.contains("quality"));
    EXPECT_TRUE(doc["quality"]["is_usable"].get<bool>());
    EXPECT_TRUE(doc.contains("evidence"));
    EXPECT_GT(doc["evidence"].size(), 0u);
    EXPECT_TRUE(doc.contains("total_duration_us"));
    EXPECT_NE(out.str().find("[PASS]"), std::string::npos);
}

TEST_F(CliLibTest, SvgExportWritesVectorCurves) {
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string overlay = (dir / "overlay.png").string();
    const std::string svg = (dir / "curves.svg").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out", overlay.c_str(),
                          "--svg",   svg.c_str()};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runCurvCli(7, const_cast<char**>(argv), out, err), 0) << err.str();
    ASSERT_TRUE(fs::exists(svg));
    std::ifstream in(svg);
    std::stringstream svg_content;
    svg_content << in.rdbuf();
    EXPECT_NE(svg_content.str().find("<svg"), std::string::npos);
    EXPECT_NE(out.str().find("SVG"), std::string::npos);
}

TEST_F(CliLibTest, QualityRejectedFrameSkipsRenderingWithWarning) {
    // A pitch-black frame is rejected by the Laplacian quality gate.
    const auto path = dir / "black.png";
    cv::Mat black(64, 64, CV_8UC3, cv::Scalar(0, 0, 0));
    ASSERT_TRUE(cv::imwrite(path.string(), black));
    const std::string image_arg = path.string();
    const std::string overlay = (dir / "overlay.png").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out", overlay.c_str()};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runCurvCli(5, const_cast<char**>(argv), out, err), 0);
    EXPECT_NE(out.str().find("rejected by quality gate"), std::string::npos);
    EXPECT_FALSE(fs::exists(overlay)); // rendering skipped
}

TEST_F(CliLibTest, DomainPacksRunAndEnrichTheEvidencePayload) {
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string evidence = (dir / "evidence_palm.json").string();
    const std::string overlay = (dir / "overlay.png").string();

    const char* argv[] = {"curv_cli", "--image",  image_arg.c_str(), "--out",    overlay.c_str(),
                          "--json",  evidence.c_str(), "--domain", "palm"};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runCurvCli(9, const_cast<char**>(argv), out, err), 0) << err.str();
    std::ifstream in(evidence);
    nlohmann::json doc = nlohmann::json::parse(in);
    EXPECT_TRUE(doc.contains("domain_payload"));
    EXPECT_NE(out.str().find("palm"), std::string::npos);
}

TEST_F(CliLibTest, SurfaceInspectionAliasIsAccepted) {
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string evidence = (dir / "evidence_surface.json").string();
    const std::string overlay = (dir / "overlay.png").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out",    overlay.c_str(),
                          "--json",  evidence.c_str(), "--domain", "surface"};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runCurvCli(9, const_cast<char**>(argv), out, err), 0) << err.str();
    EXPECT_NE(out.str().find("surface_inspection"), std::string::npos);
}

TEST_F(CliLibTest, UnknownDomainNameRunsWithoutDomainPack) {
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string evidence = (dir / "evidence_none.json").string();
    const std::string overlay = (dir / "overlay.png").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out",    overlay.c_str(),
                          "--json",  evidence.c_str(), "--domain", "astrology"};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runCurvCli(9, const_cast<char**>(argv), out, err), 0) << err.str();
    std::ifstream in(evidence);
    nlohmann::json doc = nlohmann::json::parse(in);
    EXPECT_FALSE(doc.contains("domain_payload"));
}

TEST_F(CliLibTest, UnknownFlagsAreIgnoredAndValuelessNumericFlagsDontConsume) {
    // Unknown flags are skipped; a numeric flag at the end of argv (no value)
    // is ignored thanks to the bounds guard.
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string overlay = (dir / "overlay.png").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out", overlay.c_str(),
                          "--frobnicate", "--dark", "--low"};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runCurvCli(8, const_cast<char**>(argv), out, err), 0) << err.str();
}

TEST_F(CliLibTest, MalformedNumericValueFailsWithClearError) {
    const auto image = makeLineImage();
    const std::string image_arg = image.string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--sigma", "fast"};
    std::stringstream out;
    std::stringstream err;
    EXPECT_EQ(CurvEngine::tools::runCurvCli(5, const_cast<char**>(argv), out, err), 1);
    EXPECT_NE(err.str().find("Invalid numeric value for --sigma"), std::string::npos);
}

TEST_F(CliLibTest, ThresholdFlagsShapeTheExtractionConfig) {
    // Absurdly high hysteresis thresholds suppress all segments but must not crash.
    const auto image = makeLineImage();
    const std::string image_arg = image.string();
    const std::string evidence = (dir / "evidence_high.json").string();
    const std::string overlay = (dir / "overlay.png").string();

    const char* argv[] = {"curv_cli", "--image", image_arg.c_str(), "--out", overlay.c_str(),
                          "--json",  evidence.c_str(), "--sigma", "9.0", "--low", "500", "--high", "900"};
    std::stringstream out;
    std::stringstream err;
    ASSERT_EQ(CurvEngine::tools::runCurvCli(13, const_cast<char**>(argv), out, err), 0) << err.str();
    std::ifstream in(evidence);
    nlohmann::json doc = nlohmann::json::parse(in);
    EXPECT_TRUE(doc.contains("evidence"));
}

} // namespace
