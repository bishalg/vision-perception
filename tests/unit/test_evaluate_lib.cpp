#include <cmath>
#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <vector>

#include <gtest/gtest.h>

#include "curv/tools/evaluate_lib.hpp"

namespace {

using CurvEngine::tools::evaluatePoints;
using CurvEngine::tools::EvaluationConfig;
using CurvEngine::tools::EvaluationResult;
using CurvEngine::tools::extractedFromEvidence;
using CurvEngine::tools::truthFromJson;

std::vector<cv::Point2d> gridPoints(const std::vector<std::pair<double, double>>& coords) {
    std::vector<cv::Point2d> pts;
    pts.reserve(coords.size());
    for (const auto& [x, y] : coords) {
        pts.emplace_back(x, y);
    }
    return pts;
}

TEST(EvaluateLibTest, PerfectMatchGivesZeroErrorAndPasses) {
    const auto truth = gridPoints({{0.0, 0.0}, {1.0, 0.0}, {2.0, 0.0}});
    const auto result = evaluatePoints(truth, truth);
    EXPECT_EQ(result.matched, 3);
    EXPECT_EQ(result.false_positives, 0);
    EXPECT_EQ(result.false_negatives, 0);
    EXPECT_DOUBLE_EQ(result.rmse_px, 0.0);
    EXPECT_TRUE(result.passed);
}

TEST(EvaluateLibTest, RmseIsRootMeanSquareOfMatchedResiduals) {
    const auto truth = gridPoints({{0.0, 0.0}, {10.0, 0.0}, {20.0, 0.0}});
    // Shift every extracted point 0.05 px up -> residual 0.05 for each pair.
    std::vector<cv::Point2d> extracted;
    for (const auto& pt : truth) {
        extracted.emplace_back(pt.x, pt.y + 0.05);
    }
    const auto result = evaluatePoints(truth, extracted);
    EXPECT_EQ(result.matched, 3);
    EXPECT_NEAR(result.rmse_px, 0.05, 1e-12);
    EXPECT_TRUE(result.passed);
}

TEST(EvaluateLibTest, RmseAtGateBoundaryFails) {
    const auto truth = gridPoints({{0.0, 0.0}, {10.0, 0.0}, {20.0, 0.0}, {30.0, 0.0}});
    std::vector<cv::Point2d> extracted;
    for (const auto& pt : truth) {
        extracted.emplace_back(pt.x, pt.y + 0.1); // residual exactly == gate (0.1 px)
    }
    const EvaluationConfig cfg; // rmse_gate_px = 0.1
    const auto result = evaluatePoints(truth, extracted, cfg);
    EXPECT_NEAR(result.rmse_px, 0.1, 1e-12);
    // Roadmap gate: RMSE >= 0.1 px is a failure.
    EXPECT_FALSE(result.passed);
}

TEST(EvaluateLibTest, ResidualsBeyondMatchRadiusAreFalsePositivesAndMisses) {
    const auto truth = gridPoints({{0.0, 0.0}, {10.0, 0.0}});
    const auto extracted = gridPoints({{0.5, 0.0}, {10.0, 5.0}}); // 0.5 px ok, 5 px too far
    EvaluationConfig cfg;
    cfg.match_radius_px = 2.0;
    const auto result = evaluatePoints(truth, extracted, cfg);
    EXPECT_EQ(result.matched, 1);
    // The far extracted point is spurious (FP); the second GT sample is uncovered (FN).
    EXPECT_EQ(result.false_positives, 1);
    EXPECT_EQ(result.false_negatives, 1);
    EXPECT_NEAR(result.rmse_px, 0.5, 1e-12);
    EXPECT_FALSE(result.passed);
}

TEST(EvaluateLibTest, ExtraExtractedPointsAreFalsePositives) {
    const auto truth = gridPoints({{5.0, 5.0}});
    const auto extracted = gridPoints({{5.0, 5.0}, {50.0, 50.0}, {60.0, 60.0}});
    const auto result = evaluatePoints(truth, extracted);
    EXPECT_EQ(result.matched, 1);
    EXPECT_EQ(result.false_positives, 2);
    EXPECT_EQ(result.false_negatives, 0);
    EXPECT_DOUBLE_EQ(result.rmse_px, 0.0);
    EXPECT_TRUE(result.passed); // accuracy unaffected by FPs; counted separately
}

TEST(EvaluateLibTest, DenseTruthServesManyExtractedPoints) {
    // One GT sample between two extracted points: both match it (many-to-one).
    const auto truth = gridPoints({{0.5, 0.0}});
    const auto extracted = gridPoints({{0.1, 0.0}, {0.9, 0.0}});
    EvaluationConfig cfg;
    cfg.match_radius_px = 1.0;
    const auto result = evaluatePoints(truth, extracted, cfg);
    EXPECT_EQ(result.matched, 2);
    EXPECT_EQ(result.false_negatives, 0);
    EXPECT_EQ(result.false_positives, 0);
    EXPECT_DOUBLE_EQ(result.rmse_px, 0.4);
}

TEST(EvaluateLibTest, UncoveredGroundTruthFailsEvenWithPerfectLocalAccuracy) {
    // Extraction nails one spot but never touches the second GT sample.
    const auto truth = gridPoints({{0.0, 0.0}, {10.0, 0.0}});
    const auto extracted = gridPoints({{0.0, 0.0}});
    EvaluationConfig cfg;
    cfg.match_radius_px = 2.0;
    const auto result = evaluatePoints(truth, extracted, cfg);
    EXPECT_EQ(result.false_negatives, 1);
    EXPECT_DOUBLE_EQ(result.rmse_px, 0.0);
    EXPECT_FALSE(result.passed);
}

TEST(EvaluateLibTest, EndMarginExcludesEndpointRunOutArtifacts) {
    // Dense GT along y=0 from x=0..100; extraction is accurate on the body but
    // produces run-out near both ends (the rasterized line-cap region).
    std::vector<std::pair<double, double>> body;
    for (double x = 0.0; x <= 100.0; x += 5.0) {
        body.emplace_back(x, 0.0);
    }
    const auto truth = gridPoints(body);
    std::vector<std::pair<double, double>> extracted_coords = body;
    for (auto& [x, y] : extracted_coords) {
        y += 0.02; // small uniform localization error on the curve body
    }
    // Ridge run-out past the drawn caps, inside the 2 px match radius.
    extracted_coords.emplace_back(-1.5, 0.5);
    extracted_coords.emplace_back(101.5, 0.5);
    const auto extracted = gridPoints(extracted_coords);

    EvaluationConfig trimmed;
    trimmed.end_margin_px = 10.0;
    const auto result = evaluatePoints(truth, extracted, trimmed);
    // The 10 px band around each end covers 3 GT samples per side; the 6 body
    // points matching inside the band plus the 2 run-out points are excluded.
    EXPECT_EQ(result.excluded_boundary_points, 8);
    EXPECT_EQ(result.matched, 15);
    EXPECT_EQ(result.false_positives, 0);
    EXPECT_EQ(result.false_negatives, 0);
    EXPECT_NEAR(result.rmse_px, 0.02, 1e-12);
    EXPECT_TRUE(result.passed);

    // Without trimming, the same artifacts are penalized as low-quality matches.
    const auto untrimmed = evaluatePoints(truth, extracted);
    EXPECT_EQ(untrimmed.excluded_boundary_points, 0);
    EXPECT_GT(untrimmed.rmse_px, 0.4);
    EXPECT_FALSE(untrimmed.passed);
}

TEST(EvaluateLibTest, EmptyInputsAreHandled) {
    const auto empty_truth = evaluatePoints({}, gridPoints({{1.0, 1.0}}));
    EXPECT_EQ(empty_truth.false_positives, 1);
    EXPECT_DOUBLE_EQ(empty_truth.rmse_px, 0.0);
    EXPECT_FALSE(empty_truth.passed);

    const auto empty_extracted = evaluatePoints(gridPoints({{1.0, 1.0}}), {});
    EXPECT_EQ(empty_extracted.false_negatives, 1);
    EXPECT_FALSE(empty_extracted.passed);

    const auto both_empty = evaluatePoints({}, {});
    EXPECT_EQ(both_empty.ground_truth_points, 0);
    EXPECT_DOUBLE_EQ(both_empty.rmse_px, 0.0);
    EXPECT_TRUE(both_empty.passed); // nothing to measure, nothing wrong
}

TEST(EvaluateLibTest, ReportJsonCarriesAllMetrics) {
    const auto truth = gridPoints({{0.0, 0.0}, {10.0, 0.0}});
    const auto extracted = gridPoints({{0.0, 0.0}, {10.0, 0.0}, {99.0, 99.0}});
    const auto result = evaluatePoints(truth, extracted);
    const nlohmann::json report = result.to_json();
    EXPECT_EQ(report["rmse_px"].get<double>(), result.rmse_px);
    EXPECT_EQ(report["matched"].get<int>(), 2);
    EXPECT_EQ(report["false_positives"].get<int>(), 1);
    EXPECT_EQ(report["false_negatives"].get<int>(), 0);
    EXPECT_EQ(report["ground_truth_points"].get<int>(), 2);
    EXPECT_EQ(report["extracted_points"].get<int>(), 3);
    EXPECT_EQ(report["passed"].get<bool>(), result.passed);
}

TEST(EvaluateLibTest, TruthJsonParsesGroundTruthPoints) {
    nlohmann::json doc = {{"curve_type", "line"},
                          {"width", 100},
                          {"height", 100},
                          {"ground_truth_points", nlohmann::json::array({{1.5, 2.5}, {3.5, 4.5}})}};
    const auto pts = truthFromJson(doc);
    ASSERT_EQ(pts.size(), 2);
    EXPECT_DOUBLE_EQ(pts[0].x, 1.5);
    EXPECT_DOUBLE_EQ(pts[0].y, 2.5);
    EXPECT_DOUBLE_EQ(pts[1].x, 3.5);
    EXPECT_DOUBLE_EQ(pts[1].y, 4.5);
}

TEST(EvaluateLibTest, TruthJsonWithoutGroundTruthArrayIsRejected) {
    nlohmann::json doc = {{"curve_type", "line"}};
    EXPECT_THROW(static_cast<void>(truthFromJson(doc)), std::runtime_error);
    nlohmann::json wrong_type = {{"ground_truth_points", 42}};
    EXPECT_THROW(static_cast<void>(truthFromJson(wrong_type)), std::runtime_error);
}

TEST(EvaluateLibTest, ExtractedPointsComeFromLineCandidateEvidence) {
    nlohmann::json doc = {
        {"evidence",
         nlohmann::json::array({
             nlohmann::json{{"type", "line_candidate"},
                            {"geometry",
                             {{"points",
                               nlohmann::json::array({nlohmann::json{{"x", 1.0}, {"y", 2.0}},
                                                      nlohmann::json{{"x", 3.0}, {"y", 4.0}}})}}}},
             nlohmann::json{{"type", "quality_report"}, {"geometry", {{"blur_score", 42.0}}}},
         })}};
    const auto pts = extractedFromEvidence(doc);
    ASSERT_EQ(pts.size(), 2);
    EXPECT_DOUBLE_EQ(pts[0].x, 1.0);
    EXPECT_DOUBLE_EQ(pts[1].y, 4.0);
}

TEST(EvaluateLibTest, ExtractedPointsSkipMalformedEntries) {
    nlohmann::json doc = {{"evidence",
                           nlohmann::json::array({nlohmann::json{{"type", "line_candidate"},
                                                                 {"geometry", {{"points", 7}}}},
                                                 nlohmann::json{{"type", "line_candidate"}},
                                                 nlohmann::json{{"type", "line_candidate"},
                                                                {"geometry",
                                                                 {{"points",
                                                                   nlohmann::json::array({
                                                                       nlohmann::json{{"x", 5.0}, {"y", 6.0}},
                                                                   })}}}}})}};
    const auto pts = extractedFromEvidence(doc);
    ASSERT_EQ(pts.size(), 1);
    EXPECT_DOUBLE_EQ(pts[0].x, 5.0);
    EXPECT_DOUBLE_EQ(pts[0].y, 6.0);
}

TEST(EvaluateLibTest, EvidenceWithoutEvidenceArrayIsRejected) {
    EXPECT_THROW(static_cast<void>(extractedFromEvidence(nlohmann::json::object())), std::runtime_error);
}

} // namespace
