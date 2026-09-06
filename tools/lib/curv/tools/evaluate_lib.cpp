#include "curv/tools/evaluate_lib.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>

namespace CurvEngine::tools {

namespace {

double squaredDistance(const cv::Point2d& a, const cv::Point2d& b) {
    const double dx = a.x - b.x;
    const double dy = a.y - b.y;
    return dx * dx + dy * dy;
}

} // namespace

nlohmann::json EvaluationResult::to_json() const {
    return nlohmann::json{{"rmse_px", rmse_px},
                          {"matched", matched},
                          {"false_positives", false_positives},
                          {"false_negatives", false_negatives},
                          {"ground_truth_points", ground_truth_points},
                          {"extracted_points", extracted_points},
                          {"excluded_boundary_points", excluded_boundary_points},
                          {"passed", passed}};
}

EvaluationResult evaluatePoints(const std::vector<cv::Point2d>& truth, const std::vector<cv::Point2d>& extracted,
                                const EvaluationConfig& cfg) {
    EvaluationResult result;
    result.ground_truth_points = static_cast<int>(truth.size());
    result.extracted_points = static_cast<int>(extracted.size());

    const double match_radius_sq = cfg.match_radius_px * cfg.match_radius_px;

    // Endpoint trim band: ground-truth samples within end_margin_px of either
    // curve end are excluded from coverage, and extracted points whose nearest
    // GT sample falls in the band are excluded from accuracy metrics.
    std::vector<bool> gt_trimmed(truth.size(), false);
    if (cfg.end_margin_px > 0.0 && truth.size() >= 2) {
        const cv::Point2d& first = truth.front();
        const cv::Point2d& last = truth.back();
        for (size_t i = 0; i < truth.size(); ++i) {
            gt_trimmed[i] = squaredDistance(truth[i], first) <= cfg.end_margin_px * cfg.end_margin_px ||
                            squaredDistance(truth[i], last) <= cfg.end_margin_px * cfg.end_margin_px;
        }
    }

    // Accuracy: residual of each extracted point = distance to nearest GT sample.
    // Unmatched points (no GT inside the radius) are false positives, excluded
    // from RMSE; points trimmed by the end band are excluded from all metrics.
    long double sum_squared_residual = 0.0L;
    for (const auto& ex : extracted) {
        if (truth.empty()) {
            ++result.false_positives;
            continue;
        }
        double nearest_sq = std::numeric_limits<double>::max();
        size_t nearest_index = 0;
        for (size_t i = 0; i < truth.size(); ++i) {
            const double sq = squaredDistance(ex, truth[i]);
            if (sq < nearest_sq) {
                nearest_sq = sq;
                nearest_index = i;
            }
        }
        if (gt_trimmed[nearest_index]) {
            ++result.excluded_boundary_points;
            continue;
        }
        if (nearest_sq >= match_radius_sq) {
            ++result.false_positives;
            continue;
        }
        ++result.matched;
        sum_squared_residual += static_cast<long double>(nearest_sq);
    }
    if (result.matched > 0) {
        const long double mean_sq = sum_squared_residual / static_cast<long double>(result.matched);
        result.rmse_px = static_cast<double>(std::sqrt(mean_sq));
    }

    // Coverage: every non-trimmed GT sample must have some extracted point within the radius.
    for (size_t i = 0; i < truth.size(); ++i) {
        if (gt_trimmed[i]) {
            continue;
        }
        bool covered = false;
        for (const auto& ex : extracted) {
            if (squaredDistance(truth[i], ex) <= match_radius_sq) {
                covered = true;
                break;
            }
        }
        if (!covered) {
            ++result.false_negatives;
        }
    }

    const bool has_truth = result.ground_truth_points > 0;
    const bool completely_empty = result.ground_truth_points == 0 && result.extracted_points == 0;
    result.passed = (has_truth || completely_empty) && result.false_negatives == 0 && result.rmse_px < cfg.rmse_gate_px;
    return result;
}

std::vector<cv::Point2d> truthFromJson(const nlohmann::json& truth_doc) {
    std::vector<cv::Point2d> pts;
    const auto it = truth_doc.find("ground_truth_points");
    if (it == truth_doc.end() || !it->is_array()) {
        throw std::runtime_error("truth document has no ground_truth_points array");
    }
    pts.reserve(it->size());
    for (const auto& entry : *it) {
        if (entry.is_array() && entry.size() >= 2 && entry[0].is_number() && entry[1].is_number()) {
            pts.emplace_back(entry[0].get<double>(), entry[1].get<double>());
        }
    }
    return pts;
}

std::vector<cv::Point2d> extractedFromEvidence(const nlohmann::json& evidence_doc) {
    const auto evidence_it = evidence_doc.find("evidence");
    if (evidence_it == evidence_doc.end() || !evidence_it->is_array()) {
        throw std::runtime_error("evidence document has no evidence array");
    }
    std::vector<cv::Point2d> pts;
    for (const auto& item : *evidence_it) {
        if (!item.is_object() || item.value("type", std::string{}) != "line_candidate") {
            continue;
        }
        const auto geometry_it = item.find("geometry");
        if (geometry_it == item.end() || !geometry_it->is_object()) {
            continue;
        }
        const auto points_it = geometry_it->find("points");
        if (points_it == geometry_it->end() || !points_it->is_array()) {
            continue;
        }
        for (const auto& pt : *points_it) {
            if (pt.is_object() && pt.contains("x") && pt.contains("y") && pt["x"].is_number() && pt["y"].is_number()) {
                pts.emplace_back(pt["x"].get<double>(), pt["y"].get<double>());
            }
        }
    }
    return pts;
}

int runEvaluator(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    std::string truth_path;
    std::string evidence_path;
    std::string report_path;
    EvaluationConfig cfg;

    for (int i = 1; i < argc; ++i) {
        const std::string arg = argv[i];
        if (arg == "--truth" && i + 1 < argc) {
            truth_path = argv[++i];
        } else if (arg == "--evidence" && i + 1 < argc) {
            evidence_path = argv[++i];
        } else if (arg == "--gate" && i + 1 < argc) {
            cfg.rmse_gate_px = std::stod(argv[++i]);
        } else if (arg == "--radius" && i + 1 < argc) {
            cfg.match_radius_px = std::stod(argv[++i]);
        } else if (arg == "--end-margin" && i + 1 < argc) {
            cfg.end_margin_px = std::stod(argv[++i]);
        } else if (arg == "--report" && i + 1 < argc) {
            report_path = argv[++i];
        } else {
            err << "Usage: " << argv[0] << " --truth <truth.json> --evidence <evidence.json>"
                << " [--gate px] [--radius px] [--end-margin px] [--report out.json]\n";
            return 1;
        }
    }

    if (truth_path.empty() || evidence_path.empty()) {
        err << "[ERROR] --truth and --evidence are required\n";
        return 1;
    }

    nlohmann::json truth_doc;
    nlohmann::json evidence_doc;
    try {
        std::ifstream t_in(truth_path);
        if (!t_in) {
            err << "[ERROR] Failed to open truth file: " << truth_path << '\n';
            return 1;
        }
        truth_doc = nlohmann::json::parse(t_in);
        std::ifstream e_in(evidence_path);
        if (!e_in) {
            err << "[ERROR] Failed to open evidence file: " << evidence_path << '\n';
            return 1;
        }
        evidence_doc = nlohmann::json::parse(e_in);
    } catch (const std::exception& e) {
        err << "[ERROR] Failed to parse JSON: " << e.what() << '\n';
        return 1;
    }

    std::vector<cv::Point2d> truth;
    std::vector<cv::Point2d> extracted;
    try {
        truth = truthFromJson(truth_doc);
        extracted = extractedFromEvidence(evidence_doc);
    } catch (const std::exception& e) {
        err << "[ERROR] " << e.what() << '\n';
        return 1;
    }

    const EvaluationResult result = evaluatePoints(truth, extracted, cfg);
    const nlohmann::json report = result.to_json();
    out << report.dump(2) << '\n';

    if (!report_path.empty()) {
        std::ofstream report_out(report_path);
        if (!report_out) {
            err << "[ERROR] Failed to write report to: " << report_path << '\n';
            return 1;
        }
        report_out << report.dump(2);
    }

    if (!result.passed) {
        err << "[FAIL] RMSE gate violated: rmse=" << result.rmse_px << " px >= gate " << cfg.rmse_gate_px
            << " px (or unmatched ground truth)\n";
        return 2;
    }
    out << "[PASS] Extraction accuracy within gate: rmse=" << result.rmse_px << " px < " << cfg.rmse_gate_px << " px\n";
    return 0;
}

} // namespace CurvEngine::tools
