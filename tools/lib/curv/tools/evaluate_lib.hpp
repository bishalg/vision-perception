#pragma once

#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <string>
#include <vector>

namespace CurvEngine::tools {

/**
 * @brief Thresholds governing extraction-accuracy evaluation (roadmap M6 gate).
 */
struct EvaluationConfig {
    double rmse_gate_px{0.1};    ///< Evaluation fails when RMSE >= this value.
    double match_radius_px{2.0}; ///< Max distance for a GT<->extracted association.
    /// Width (px) of the trim band around the ground-truth curve endpoints.
    /// Extracted ridge run-out and rasterized cap artifacts live here; such
    /// points are excluded from all metrics. 0 disables trimming.
    double end_margin_px{0.0};
};

/**
 * @brief Aggregate accuracy metrics of an extracted point set vs. ground truth.
 */
struct EvaluationResult {
    double rmse_px{0.0};
    int matched{0};
    int false_positives{0};
    int false_negatives{0};
    int ground_truth_points{0};
    int extracted_points{0};
    int excluded_boundary_points{0};
    bool passed{false};

    [[nodiscard]] nlohmann::json to_json() const;
};

/**
 * @brief Greedy nearest-neighbour evaluation of extracted points against dense ground truth.
 *
 * Accuracy: for every extracted point, the distance to its nearest ground-truth sample;
 * rmse_px is the root-mean-square of those residuals over matched points (extracted points
 * with no ground truth inside match_radius_px count as false positives and are excluded
 * from the RMSE). Ground truth is densely sampled along the true curve, so this measures
 * the localization error of each extracted point.
 *
 * Coverage: a ground-truth sample with no extracted point within match_radius_px counts
 * as a false negative.
 *
 * passed is true only when ground truth exists (or both sets are empty), every
 * ground-truth sample is covered (no false negatives), and rmse_px < rmse_gate_px.
 */
[[nodiscard]] EvaluationResult evaluatePoints(const std::vector<cv::Point2d>& truth,
                                              const std::vector<cv::Point2d>& extracted,
                                              const EvaluationConfig& cfg = EvaluationConfig{});

/**
 * @brief Parses ground-truth samples from a synth_generator truth document ([[x, y], ...] pairs).
 * @throws std::runtime_error when the document has no usable ground_truth_points array.
 */
[[nodiscard]] std::vector<cv::Point2d> truthFromJson(const nlohmann::json& truth_doc);

/**
 * @brief Collects extracted sub-pixel points from an evidence JSON document
 *        (every "line_candidate" item's geometry.points). Malformed entries are skipped.
 * @throws std::runtime_error when the document has no "evidence" array.
 */
[[nodiscard]] std::vector<cv::Point2d> extractedFromEvidence(const nlohmann::json& evidence_doc);

/**
 * @brief CLI entry point: --truth <file> --evidence <file> [--gate px] [--radius px] [--report file].
 * @return 0 when the gate passes, 2 when accuracy fails the gate, 1 on usage/IO errors.
 */
int runEvaluator(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
