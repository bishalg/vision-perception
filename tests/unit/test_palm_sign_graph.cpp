// Micro-signs read their own ridge graph, not the major-crease graph.
//
// Measured on the canonical capture during the PR4 audit, holding everything else fixed.
// "reference config" = sigma 1.5 / low 0.5 / high 1.5 / minlen 5, i.e. the Steger settings
// curv_cli uses, which is the configuration the head-line island was first recorded under:
//
//   raw patch, light polarity -> 1 sign, Island conf=0.950 host=head_line   <-- the anchor
//   raw patch, dark polarity  -> 362 segs, 0 signs (no island)
//   raw patch, both polarities-> 732 segs, texture ceiling trips (k=626, skipped)
//   flatten-only, light       -> 494 segs, 0 signs
//   flatten+CLAHE 2.0, light  -> 624 segs, 0 signs
//
// CLAHE demonstrably helps the major-crease pass, so the split is: majors keep consuming
// the preprocessed image, signs take the raw patch. `micro_signs.source` records which.
//
// SCOPE: these tests pin WIRING and the isolated variables above. They deliberately do NOT
// assert a sign-inventory floor for production. The head-line island reproduces at exactly
// one Steger config and vanishes when sigma, minlen or the hysteresis pair move (measured:
// sigma 1.5 + minlen 10.3 -> 254 segs, 0 signs; minlen 5 + sigma 1.361 -> 616 segs, no
// island), and its host arc is path-dependent (0.4228 via curv_cli, 1.0000 in-process).
// Production's scale-adaptive config does not recover it. Per-sign accuracy therefore needs
// real ground truth: the rendered-signs sweep recovers 0/8 at every setting tried and
// cannot arbitrate. Inventories are printed for human review.
#include <gtest/gtest.h>

#include "curv/domain/CreaseChainLinker.hpp"
#include "curv/domain/PalmParameters.hpp"
#include "curv/domains/PalmDomainPack.hpp"
#include "curv/preprocess/FlatFieldCLAHEPreprocessor.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>

#include <cmath>
#include <iostream>
#include <string>
#include <vector>

using namespace CurvEngine;

namespace {

constexpr int kDim = 512;
// RoiWarper normalizes ||P5 - P17|| to 0.55 * target_dim, so this is the canonical
// palm radius the scale-adaptive configs are derived from.
constexpr float kPalmRadius = 0.55F * static_cast<float>(kDim) * 0.5F;

// Stand-in for HandAnatomyDecomposer::decompose's central_palm_disc on the canonical
// patch, so the test needs no landmark fixture. The decomposed disc on canonical-space
// landmarks measures ~173 px radius.
std::vector<Point2D> canonicalPalmDisc() {
    std::vector<Point2D> d;
    const float cx = 0.50F * static_cast<float>(kDim);
    const float cy = 0.625F * static_cast<float>(kDim);
    constexpr float kRadius = 165.0F;
    for (int i = 0; i < 64; ++i) {
        const float a = 6.2831853F * static_cast<float>(i) / 64.0F;
        d.push_back(Point2D{cx + kRadius * std::cos(a), cy + kRadius * std::sin(a), 0.0F, 1.0F});
    }
    return d;
}

cv::Mat loadCanonical() {
    cv::Mat img = cv::imread(std::string(CURV_SOURCE_DIR) + "/canonical_palm.png", cv::IMREAD_GRAYSCALE);
    EXPECT_FALSE(img.empty()) << "missing canonical_palm.png";
    return img;
}

cv::Mat preprocessCanonical(const cv::Mat& raw, bool with_clahe) {
    preprocess::FlatFieldCLAHEConfig ff{};
    ff.enabled = true;
    ff.enable_clahe = with_clahe;
    return preprocess::FlatFieldCLAHEPreprocessor(ff).apply(raw, 0.55F * kDim);
}

// Production major-pass config: scale-adaptive, dark polarity.
ridge::StegerConfig productionConfig(bool light_polarity) {
    auto cfg = domains::PalmParameters::computeStegerConfig(kPalmRadius);
    cfg.extract_dark_lines = !light_polarity;
    return cfg;
}

// The config the canonical island was recorded under. Used only to isolate the variable
// under test; see the SCOPE note above.
ridge::StegerConfig referenceConfig(bool light_polarity = true, bool both = false) {
    ridge::StegerConfig cfg;
    cfg.sigma = 1.5F;
    cfg.low_threshold = 0.5F;
    cfg.high_threshold = 1.5F;
    cfg.min_segment_length = 5.0F;
    cfg.extract_dark_lines = !light_polarity;
    cfg.extract_both_polarities = both;
    return cfg;
}

nlohmann::json runPack(const cv::Mat& frame_image, const cv::Mat& sign_image, const ridge::StegerConfig& sign_cfg) {
    RidgeGraph sign_graph = ridge::StegerRidgeExtractor(sign_cfg).extractRidgeGraph(sign_image);
    const auto disc = canonicalPalmDisc();
    domain::CreaseChainLinker::filterGraphByPalmDisc(sign_graph, disc, 0.40F);
    // The major pass reads the preprocessed image, as in production.
    RidgeGraph major = ridge::StegerRidgeExtractor(productionConfig(false))
                           .extractRidgeGraph(preprocessCanonical(loadCanonical(), true));
    const Frame frame(frame_image, "canonical", 0);
    const QualityReport quality;
    const DomainInput in{frame, major, quality, nullptr, "flat_field_clahe_v1", &sign_graph};
    return domains::PalmDomainPack().process(in).domain_payload;
}

bool hasHeadLineIsland(const nlohmann::json& payload) {
    const auto& ms = payload.at("micro_signs");
    if (ms.contains("skipped")) {
        return false;
    }
    for (const auto& s : ms.at("signs")) {
        if (s.value("type", "") == "Island" && s.value("host", "") == "head_line") {
            return true;
        }
    }
    return false;
}

double bestIslandConfidence(const nlohmann::json& payload) {
    const auto& ms = payload.at("micro_signs");
    if (ms.contains("skipped")) {
        return 0.0;
    }
    double best = 0.0;
    for (const auto& s : ms.at("signs")) {
        if (s.value("type", "") == "Island") {
            best = std::max(best, s.value("confidence", 0.0));
        }
    }
    return best;
}

void printInventory(const char* tag, const nlohmann::json& payload) {
    const auto& ms = payload.at("micro_signs");
    std::cout << "  [" << tag << "] source=" << ms.value("source", "?");
    if (ms.contains("skipped")) {
        std::cout << " skipped=" << ms["skipped"].dump() << "\n";
        return;
    }
    std::cout << " signs=" << ms["signs"].size() << "\n";
    for (const auto& s : ms["signs"]) {
        std::cout << "     " << s.value("type", "?") << " conf=" << s.value("confidence", 0.0) << " ("
                  << s.value("x", 0.0) << "," << s.value("y", 0.0) << ")";
        if (s.contains("host")) {
            std::cout << " host=" << s.value("host", "") << " arc=" << s.value("host_arc_s", 0.0);
        }
        std::cout << "\n";
    }
}

} // namespace

// Wiring: a caller-supplied sign graph is used, is non-empty, and is declared in the payload.
TEST(PalmSignGraph, SignRidgesAreUsedAndDeclared) {
    const cv::Mat raw = loadCanonical();
    ASSERT_FALSE(raw.empty());
    const cv::Mat preprocessed = preprocessCanonical(raw, true);
    RidgeGraph major = ridge::StegerRidgeExtractor(productionConfig(false)).extractRidgeGraph(preprocessed);

    const auto disc = canonicalPalmDisc();
    RidgeGraph sign_graph = ridge::StegerRidgeExtractor(productionConfig(true)).extractRidgeGraph(raw);
    domain::CreaseChainLinker::filterGraphByPalmDisc(sign_graph, disc, 0.40F);
    ASSERT_FALSE(sign_graph.segments.empty()) << "sign pass must find creases inside the disc";

    const Frame frame(preprocessed, "canonical", 0);
    const QualityReport quality;
    const DomainInput in{frame, major, quality, nullptr, "flat_field_clahe_v1", &sign_graph};
    const auto out = domains::PalmDomainPack().process(in);

    const auto& ms = out.domain_payload.at("micro_signs");
    EXPECT_EQ(ms.value("source", ""), "sign_ridges");
    EXPECT_EQ(ms.value("schema_version", 0), 1) << "adding source must not bump the schema";
    EXPECT_FALSE(ms.contains("skipped")) << "the pipeline's sign graph is well inside the texture ceiling";
    printInventory("production sign_ridges", out.domain_payload);
}

// Absent sign_ridges keeps the legacy path and declares it.
TEST(PalmSignGraph, NoSignRidgesFallsBackToMajorGraph) {
    const cv::Mat raw = loadCanonical();
    ASSERT_FALSE(raw.empty());
    const cv::Mat preprocessed = preprocessCanonical(raw, true);
    RidgeGraph major = ridge::StegerRidgeExtractor(productionConfig(false)).extractRidgeGraph(preprocessed);

    const Frame frame(preprocessed, "canonical", 0);
    const QualityReport quality;
    const DomainInput in{frame, major, quality, nullptr, "flat_field_clahe_v1"};
    const auto out = domains::PalmDomainPack().process(in);

    EXPECT_EQ(out.domain_payload.at("micro_signs").value("source", ""), "ridges");
    printInventory("ridges (legacy, preprocessed)", out.domain_payload);
}

// CLAHE is excluded from the sign path because it textures the graph past the ceiling, so
// the sign pass is skipped outright rather than returning degraded signs. Raw does not skip.
// Flatten-only is reported but not asserted: it returns two islands, one at conf 0.95, at
// different positions, so a confidence floor alone does not separate it from raw.
TEST(PalmSignGraph, ClaheTripsTheSignCeilingWhileRawDoesNot) {
    const cv::Mat raw = loadCanonical();
    ASSERT_FALSE(raw.empty());
    const auto cfg = referenceConfig();

    const auto on_raw = runPack(raw, raw, cfg);
    const auto on_clahe = runPack(raw, preprocessCanonical(raw, true), cfg);

    EXPECT_FALSE(on_raw.at("micro_signs").contains("skipped")) << "raw sign graph must stay inside the ceiling";
    EXPECT_TRUE(on_clahe.at("micro_signs").contains("skipped"))
        << "CLAHE textures the sign graph past the ceiling (measured k=455)";
    EXPECT_GE(bestIslandConfidence(on_raw), 0.9) << "raw patch should yield a high-confidence island";

    printInventory("raw", on_raw);
    printInventory("flatten-only", runPack(raw, preprocessCanonical(raw, false), cfg));
    printInventory("flatten+CLAHE", on_clahe);
}

// Host attribution crosses the split and is a known limitation: host curves come from the
// MAJOR graph (preprocessed image) while signs come from the raw-patch sign graph. The
// high-confidence island on the raw patch is therefore detected but not attributed to the
// head line. Pinned so the gap is visible rather than discovered in the field.
TEST(PalmSignGraph, HostAttributionDoesNotSpanTheGraphSplit) {
    const cv::Mat raw = loadCanonical();
    ASSERT_FALSE(raw.empty());
    const auto payload = runPack(raw, raw, referenceConfig());

    ASSERT_GE(bestIslandConfidence(payload), 0.9) << "precondition: a high-confidence island is detected";
    EXPECT_FALSE(hasHeadLineIsland(payload))
        << "host curves are derived from the major graph, so a raw-patch sign is not attributed "
           "against the preprocessed head line. If this starts passing, host attribution has been "
           "reconciled across the split and the note can be retired.";
    printInventory("host attribution across split", payload);
}

// Polarity is load-bearing: this capture's creases trace as bright ridges, and the dark pass
// finds no high-confidence island.
TEST(PalmSignGraph, PolarityDecidesTheHighConfidenceIsland) {
    const cv::Mat raw = loadCanonical();
    ASSERT_FALSE(raw.empty());

    const auto on_light = runPack(raw, raw, referenceConfig(/*light=*/true));
    const auto on_dark = runPack(raw, raw, referenceConfig(/*light=*/false));
    const auto on_both = runPack(raw, raw, referenceConfig(true, /*both=*/true));

    EXPECT_GE(bestIslandConfidence(on_light), 0.9) << "light polarity should surface the island";
    EXPECT_LT(bestIslandConfidence(on_dark), 0.9) << "dark-only was measured to be blind on the raw patch";

    // Both polarities is measured-and-rejected: it doubles the segment count and trips the
    // texture ceiling rather than adding the dark-polarity signs.
    EXPECT_TRUE(on_both.at("micro_signs").contains("skipped"))
        << "both-polarity extraction was measured to exceed the texture ceiling";

    printInventory("light", on_light);
    printInventory("dark", on_dark);
    printInventory("both", on_both);
}

// The new switch must be a no-op for existing callers and must actually isolate the lift.
TEST(FlatFieldCLAHEConfig, EnableClaheIsolatesTheLocalContrastLift) {
    cv::Mat img = cv::Mat::ones(kDim, kDim, CV_8UC1) * 128;
    cv::line(img, cv::Point(40, 200), cv::Point(470, 320), cv::Scalar(90), 3);

    preprocess::FlatFieldCLAHEConfig off{};
    off.enabled = false;
    preprocess::FlatFieldCLAHEConfig flatten{};
    flatten.enabled = true;
    flatten.enable_clahe = false;
    preprocess::FlatFieldCLAHEConfig lift{};
    lift.enabled = true;

    const cv::Mat a = preprocess::FlatFieldCLAHEPreprocessor(off).apply(img, 0.55F * kDim);
    const cv::Mat b = preprocess::FlatFieldCLAHEPreprocessor(flatten).apply(img, 0.55F * kDim);
    const cv::Mat c = preprocess::FlatFieldCLAHEPreprocessor(lift).apply(img, 0.55F * kDim);

    EXPECT_EQ(cv::countNonZero(cv::Mat(a != img)), 0) << "disabled preprocessor must be a passthrough";
    EXPECT_GT(cv::countNonZero(cv::Mat(b != img)), 0) << "flattening must change the image";
    EXPECT_GT(cv::countNonZero(cv::Mat(c != b)), 0) << "enable_clahe=false must skip the local contrast lift";
    EXPECT_EQ(b.type(), img.type()) << "preprocessor outputs must stay 8-bit single channel";
}