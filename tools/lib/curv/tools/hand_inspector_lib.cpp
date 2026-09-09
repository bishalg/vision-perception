#include "curv/tools/hand_inspector_lib.hpp"

#include <fstream>
#include <memory>
#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>
#include <string>

#include "curv/Frame.hpp"
#include "curv/Version.hpp"
#include "curv/adapters/HandBoundaryDetector.hpp"
#include "curv/adapters/HandLandmarkDetector.hpp"
#include "curv/adapters/OverlayRenderer.hpp"
#include "curv/domain/CreaseChainLinker.hpp"
#include "curv/domain/HandAnatomyDecomposer.hpp"
#include "curv/domain/PalmParameters.hpp"
#include "curv/domain/PalmZones.hpp"
#include "curv/domain/VlmPromptFormatter.hpp"
#include "curv/domains/PalmDomainPack.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

namespace CurvEngine::tools {

int runHandInspector(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    out << "==================================================" << '\n';
    out << "CurvEngine Hand Inspector & VLM Grounding CLI (v" << CurvEngine::getVersion() << ")" << '\n';
    out << "==================================================" << '\n';

    std::string image_path;
    std::string landmarks_path;
    std::string auto_landmarks_model;
    std::string out_warp_path = "canonical_palm.png";
    std::string out_overlay_path = "overlay_palm.png";
    std::string out_full_overlay_path;
    std::string out_json_path = "evidence.json";
    std::string out_vlm_prompt_path = "gemini_prompt.json";
    bool const draw_whiskers = true;
    bool is_left_hand_cli = false;
    bool debug_all_ridges = false;

    for (int i = 1; i < argc; ++i) {
        std::string const arg = argv[i];
        if (arg == "--image" && i + 1 < argc) {
            image_path = argv[++i];
        } else if (arg == "--landmarks" && i + 1 < argc) {
            landmarks_path = argv[++i];
        } else if (arg == "--auto-landmarks") {
            if (i + 1 < argc && argv[i + 1][0] != '-') {
                auto_landmarks_model = argv[++i];
            }
        } else if (arg == "--left-hand") {
            is_left_hand_cli = true;
        } else if (arg == "--debug-all-ridges") {
            debug_all_ridges = true;
        } else if (arg == "--out-warp" && i + 1 < argc) {
            out_warp_path = argv[++i];
        } else if (arg == "--out-overlay" && i + 1 < argc) {
            out_overlay_path = argv[++i];
        } else if (arg == "--out-full-overlay" && i + 1 < argc) {
            out_full_overlay_path = argv[++i];
        } else if (arg == "--out-json" && i + 1 < argc) {
            out_json_path = argv[++i];
        } else if (arg == "--out-vlm-prompt" && i + 1 < argc) {
            out_vlm_prompt_path = argv[++i];
        }
    }
    (void)debug_all_ridges;

    if (image_path.empty()) {
        out << "\nUsage: " << argv[0] << " --image <path> [options]\n\n"
            << "Options:\n"
            << "  --landmarks <path.json>      (custom 21-point landmarks JSON)\n"
            << "  --auto-landmarks [model.onnx](auto landmark detection / heuristic)\n"
            << "  --left-hand                  (hint left hand for landmarks/zones)\n"
            << "  --debug-all-ridges           (render all raw ridge fragments & whiskers)\n"
            << "  --out-warp <canonical.png>   (default: canonical_palm.png)\n"
            << "  --out-overlay <overlay.png>  (default: overlay_palm.png)\n"
            << "  --out-vlm-prompt <prompt.json>(default: gemini_prompt.json)\n"
            << "  --out-json <evidence.json>   (default: evidence.json)\n";
        return 0;
    }

    // 1. Ingest Image
    out << "[STAGE 1] Loading image: " << image_path << '\n';
    cv::Mat const raw_image = cv::imread(image_path, cv::IMREAD_COLOR);
    if (raw_image.empty()) {
        err << "[ERROR] Could not load image: " << image_path << '\n';
        return 1;
    }
    out << "  - Input Resolution: " << raw_image.cols << "x" << raw_image.rows << '\n';

    // 2. Extract or Load Hand Landmarks
    CurvEngine::geometry::HandLandmarks lms;
    lms.is_normalized = true;

    if (!landmarks_path.empty()) {
        out << "[STAGE 2] Loading user landmarks: " << landmarks_path << '\n';
        std::ifstream f(landmarks_path);
        if (f.is_open()) {
            nlohmann::json js;
            f >> js;
            if (js.is_array() && js.size() >= 21) {
                for (size_t i = 0; i < 21; ++i) {
                    if (js[i].is_object()) {
                        lms.points[i].x = js[i].value("x", 0.0f);
                        lms.points[i].y = js[i].value("y", 0.0f);
                    } else if (js[i].is_array() && js[i].size() >= 2) {
                        lms.points[i].x = js[i][0].get<float>();
                        lms.points[i].y = js[i][1].get<float>();
                    }
                }
            }
        }
    } else {
        out << "[STAGE 2] Running desktop hand landmark detector..." << '\n';
        if (is_left_hand_cli && auto_landmarks_model.empty()) {
            lms = CurvEngine::adapters::HandLandmarkDetector::generateHeuristicLandmarks(raw_image.cols, raw_image.rows,
                                                                                         true);
            out << "  - Left hand heuristic landmarks established (Wrist: " << lms.points[0].x << ", "
                << lms.points[0].y << ")" << '\n';
        } else {
            CurvEngine::adapters::HandLandmarkDetector detector(auto_landmarks_model);
            auto opt_lms = detector.detect(raw_image);
            if (opt_lms) {
                lms = *opt_lms;
                out << "  - Landmarks established (Wrist: " << lms.points[0].x << ", " << lms.points[0].y << ")"
                    << '\n';
            }
        }
    }

    // 3. Compute Canonical Palm Transform & Warp
    out << "[STAGE 3] Warping to 512x512 canonical upright palm patch..." << '\n';
    const int canonical_dim = 512;
    const auto transform =
        CurvEngine::geometry::computeCanonicalPalmTransform(lms, raw_image.cols, raw_image.rows, canonical_dim);

    cv::Mat const canonical_patch = CurvEngine::geometry::warpToCanonical(raw_image, transform.M, canonical_dim);

    cv::imwrite(out_warp_path, canonical_patch);
    out << "  - Canonical patch saved to: " << out_warp_path << '\n';

    // 4. Extract Sub-Pixel Curvilinear Ridges with Adaptive Scale
    out << "[STAGE 4] Extracting sub-pixel curvilinear ridges..." << '\n';
    CurvEngine::Frame canonical_frame(canonical_patch, "canonical_palm", 1);
    const bool is_left = is_left_hand_cli || CurvEngine::geometry::detectIsLeftHand(lms);
    canonical_frame.metadata["is_left_hand"] = is_left ? "true" : "false";

    CurvEngine::quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = 15.0;
    q_thresh.min_brightness = 5.0;
    auto const quality_analyzer = std::make_shared<CurvEngine::quality::LaplacianQualityAnalyzer>(q_thresh);

    auto const tuned_cfg =
        CurvEngine::domains::PalmParameters::computeFromLandmarks(lms, raw_image.cols, raw_image.rows);
    auto const ridge_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(tuned_cfg);

    CurvEngine::pipeline::SyncPerceptionPipeline pipeline(quality_analyzer, ridge_extractor);
    auto result = pipeline.process(canonical_frame);

    out << "  - Frame Usable: " << (result.is_usable ? "YES" : "NO") << '\n';
    out << "  - Blur Score: " << result.quality.blur_score << '\n';
    out << "  - Raw Extracted Segments: " << result.ridge_graph.segments.size() << '\n';

    // 4b. Detect Canonical Palm Boundary & Purge Background Surface Noise
    out << "[STAGE 4b] Segmenting palm boundary and filtering background noise..." << '\n';
    auto const canonical_boundary = CurvEngine::adapters::HandBoundaryDetector::detectBoundary(canonical_patch);
    if (canonical_boundary.is_valid) {
        const size_t before_count = result.ridge_graph.segments.size();
        CurvEngine::adapters::HandBoundaryDetector::filterGraphByMask(result.ridge_graph, canonical_boundary.mask,
                                                                      0.65f);
        out << "  - Palm boundary contour established (Enclosed area: " << static_cast<int>(canonical_boundary.area)
            << " px^2)" << '\n';
        out << "  - Background noise purged: " << (before_count - result.ridge_graph.segments.size())
            << " segments removed (" << result.ridge_graph.segments.size() << " valid palmar segments retained)"
            << '\n';
    }

    // 4c. Structural Hand Anatomy Decomposition (Phase 2: 5 Digits & Basal MCP Crease Arc)
    out << "[STAGE 4c] Decomposing anatomical finger rays and basal crease boundary..." << '\n';
    auto const raw_anatomy = CurvEngine::domain::HandAnatomyDecomposer::decompose(
        lms, static_cast<float>(raw_image.cols), static_cast<float>(raw_image.rows));
    if (raw_anatomy.is_valid) {
        out << "  - Anatomical decomposition complete: " << raw_anatomy.digits.size()
            << " digits delineated, basal MCP crease arc established." << '\n';
    }

    // 5. Palm Domain Semantic Analysis & Zone Tagging
    out << "[STAGE 5] Running PalmDomainPack semantic zone interpretation..." << '\n';
    CurvEngine::domains::PalmDomainPack palm_pack;
    CurvEngine::DomainInput const d_in{canonical_frame, result.ridge_graph, result.quality};
    auto const d_out = palm_pack.process(d_in);

    // 5b. Crease Chaining & Continuous Spline Fitting (Phase 3: Heart, Head, Life, Fate)
    out << "[STAGE 5b] Linking major crease fragments into continuous flowing splines..." << '\n';
    auto const major_creases =
        CurvEngine::domain::CreaseChainLinker::extractMajorLines(result.ridge_graph, is_left, 512);

    for (const auto& [name, line] : major_creases) {
        if (line.is_valid) {
            out << "  - " << name << ": chained " << line.constituent_segment_ids.size()
                << " fragments into smooth continuous curve (" << static_cast<int>(line.total_length) << " px)" << '\n';
        }
    }

    // 6. Anti-Aliased Overlay Rendering with Semantic Line Color Coding & Hand Boundary
    out << "[STAGE 6] Rendering sub-pixel overlay..." << '\n';

    std::unordered_map<int, cv::Scalar> palm_colors;
    std::unordered_map<int, std::string> palm_labels;

    if (d_out.domain_payload.contains("major_lines") && d_out.domain_payload["major_lines"].is_object()) {
        const auto& mls = d_out.domain_payload["major_lines"];
        auto color_line = [&](const std::string& name, const cv::Scalar& col, const std::string& lbl) {
            if (mls.contains(name) && mls[name].value("present", false)) {
                for (int id : mls[name]["segment_ids"]) {
                    palm_colors[id] = col;
                    palm_labels[id] = lbl;
                }
            }
        };
        color_line("heart_line", cv::Scalar(80, 50, 245), "Heart Line");
        color_line("head_line", cv::Scalar(245, 160, 20), "Head Line");
        color_line("life_line", cv::Scalar(50, 205, 50), "Life Line");
        color_line("fate_line", cv::Scalar(220, 50, 180), "Fate Line");
    } else {
        if (d_out.domain_payload.contains("heart_line_id") && !d_out.domain_payload["heart_line_id"].is_null()) {
            int const id = d_out.domain_payload["heart_line_id"].get<int>();
            palm_colors[id] = cv::Scalar(80, 50, 245); // Rose / Red
            palm_labels[id] = "Heart Line";
        }
        if (d_out.domain_payload.contains("head_line_id") && !d_out.domain_payload["head_line_id"].is_null()) {
            int const id = d_out.domain_payload["head_line_id"].get<int>();
            palm_colors[id] = cv::Scalar(245, 160, 20); // Sky Blue / Cyan
            palm_labels[id] = "Head Line";
        }
        if (d_out.domain_payload.contains("life_line_id") && !d_out.domain_payload["life_line_id"].is_null()) {
            int const id = d_out.domain_payload["life_line_id"].get<int>();
            palm_colors[id] = cv::Scalar(50, 205, 50); // Emerald Green
            palm_labels[id] = "Life Line";
        }
        if (d_out.domain_payload.contains("fate_line_id") && !d_out.domain_payload["fate_line_id"].is_null()) {
            int const id = d_out.domain_payload["fate_line_id"].get<int>();
            palm_colors[id] = cv::Scalar(220, 50, 180); // Purple / Violet
            palm_labels[id] = "Fate Line";
        }
    }

    CurvEngine::adapters::OverlayConfig o_cfg;
    o_cfg.draw_subpixel_polylines = debug_all_ridges;
    o_cfg.draw_normals = debug_all_ridges && draw_whiskers;
    o_cfg.draw_hud = true;
    o_cfg.subpixel_shift = 4;
    o_cfg.line_color = cv::Scalar(100, 220, 150);
    o_cfg.segment_colors = palm_colors;
    o_cfg.segment_labels = palm_labels;

    if (canonical_boundary.is_valid) {
        o_cfg.draw_boundary = true;
        o_cfg.boundary_contours = canonical_boundary.contours;
        o_cfg.boundary_color = cv::Scalar(255, 200, 50); // Cyan / Light Aqua Blue
        o_cfg.boundary_thickness = 2;
    }

    // Continuous spline creases on canonical patch
    o_cfg.draw_continuous_creases = true;
    for (const auto& [name, line] : major_creases) {
        if (!line.is_valid || line.points.size() < 2) continue;
        CurvEngine::adapters::OverlayConfig::ContinuousCreaseItem item;
        item.points.reserve(line.points.size());
        for (const auto& pt : line.points) {
            item.points.emplace_back(static_cast<int>(std::round(pt.x)), static_cast<int>(std::round(pt.y)));
        }
        if (name == "heart_line") {
            item.color = cv::Scalar(80, 50, 245);
            item.label = "Heart Line";
        } else if (name == "head_line") {
            item.color = cv::Scalar(245, 160, 20);
            item.label = "Head Line";
        } else if (name == "life_line") {
            item.color = cv::Scalar(50, 205, 50);
            item.label = "Life Line";
        } else if (name == "fate_line") {
            item.color = cv::Scalar(220, 50, 180);
            item.label = "Fate Line";
        }
        item.thickness = 3;
        o_cfg.continuous_creases.push_back(std::move(item));
    }

    cv::Mat const overlay =
        CurvEngine::adapters::OverlayRenderer::render(canonical_patch, result.ridge_graph, result.quality, o_cfg);
    cv::imwrite(out_overlay_path, overlay);
    out << "  - Canonical overlay saved to: " << out_overlay_path << '\n';

    if (!out_full_overlay_path.empty()) {
        out << "  - Segmenting full hand boundary contour..." << '\n';
        auto const raw_boundary = CurvEngine::adapters::HandBoundaryDetector::detectBoundary(raw_image, 0.25f);

        auto full_ridge_graph = result.ridge_graph;
        CurvEngine::geometry::invertGraphCoordinates(full_ridge_graph, transform.M_inv);

        if (raw_boundary.is_valid) {
            CurvEngine::adapters::HandBoundaryDetector::filterGraphByMask(full_ridge_graph, raw_boundary.mask, 0.65f);
        }

        CurvEngine::adapters::OverlayConfig full_cfg;
        full_cfg.draw_subpixel_polylines = debug_all_ridges;
        full_cfg.draw_normals = false;
        full_cfg.draw_hud = false;
        full_cfg.line_thickness = std::max(3, raw_image.cols / 600);
        full_cfg.subpixel_shift = 4;
        full_cfg.line_color = cv::Scalar(100, 220, 150);

        // 1. Crisp Hand Silhouette Boundary
        if (raw_boundary.is_valid) {
            full_cfg.draw_boundary = true;
            full_cfg.boundary_contours = raw_boundary.contours;
            full_cfg.boundary_color = cv::Scalar(255, 200, 50); // Cyan / Aqua contour
            full_cfg.boundary_thickness = std::max(4, raw_image.cols / 400);
        }

        // 2. 2nd-Level Anatomy Lines (Digit Rays & Basal MCP Crease Arc)
        if (raw_anatomy.is_valid) {
            full_cfg.draw_anatomy = true;
            full_cfg.anatomy_color = cv::Scalar(30, 160, 255); // Warm Amber / Gold
            full_cfg.anatomy_thickness = std::max(3, raw_image.cols / 500);

            for (const auto& digit : raw_anatomy.digits) {
                std::vector<cv::Point> ray;
                ray.reserve(digit.joint_chain.size());
                for (const auto& pt : digit.joint_chain) {
                    ray.emplace_back(static_cast<int>(std::round(pt.x)), static_cast<int>(std::round(pt.y)));
                }
                full_cfg.anatomy_contours.push_back(std::move(ray));
            }

            std::vector<cv::Point> basal_arc;
            basal_arc.reserve(raw_anatomy.basal_mcp_crease.size());
            for (const auto& pt : raw_anatomy.basal_mcp_crease) {
                basal_arc.emplace_back(static_cast<int>(std::round(pt.x)), static_cast<int>(std::round(pt.y)));
            }
            full_cfg.anatomy_contours.push_back(std::move(basal_arc));
        }

        // 3. Continuous Major Crease Spline Curves
        full_cfg.draw_continuous_creases = true;
        const auto& M_inv = transform.M_inv;
        const int crease_thick = std::max(5, raw_image.cols / 300);

        for (const auto& [name, line] : major_creases) {
            if (!line.is_valid || line.points.size() < 2) continue;
            CurvEngine::adapters::OverlayConfig::ContinuousCreaseItem item;
            item.points.reserve(line.points.size());
            for (const auto& pt : line.points) {
                const double rx =
                    M_inv.at<double>(0, 0) * pt.x + M_inv.at<double>(0, 1) * pt.y + M_inv.at<double>(0, 2);
                const double ry =
                    M_inv.at<double>(1, 0) * pt.x + M_inv.at<double>(1, 1) * pt.y + M_inv.at<double>(1, 2);
                item.points.emplace_back(static_cast<int>(std::round(rx)), static_cast<int>(std::round(ry)));
            }
            if (name == "heart_line") {
                item.color = cv::Scalar(80, 50, 245);
                item.label = "Heart Line";
            } else if (name == "head_line") {
                item.color = cv::Scalar(245, 160, 20);
                item.label = "Head Line";
            } else if (name == "life_line") {
                item.color = cv::Scalar(50, 205, 50);
                item.label = "Life Line";
            } else if (name == "fate_line") {
                item.color = cv::Scalar(220, 50, 180);
                item.label = "Fate Line";
            }
            item.thickness = crease_thick;
            full_cfg.continuous_creases.push_back(std::move(item));
        }

        cv::Mat const full_overlay =
            CurvEngine::adapters::OverlayRenderer::render(raw_image, full_ridge_graph, result.quality, full_cfg);
        cv::imwrite(out_full_overlay_path, full_overlay);
        out << "  - Full image overlay with hand boundary and continuous lines saved to: " << out_full_overlay_path
            << '\n';
    }

    // 7. Assemble Perception Evidence JSON
    nlohmann::json evidence_root;
    evidence_root["quality"] = {{"blur_score", result.quality.blur_score},
                                {"brightness_score", result.quality.brightness_score},
                                {"contrast_score", result.quality.contrast_score},
                                {"is_usable", result.quality.is_usable}};
    evidence_root["domain_payload"] = d_out.domain_payload;

    nlohmann::json ev_array = nlohmann::json::array();
    if (debug_all_ridges) {
        for (const auto& ev : result.evidence) {
            ev_array.push_back(ev.to_json());
        }
    }
    for (const auto& ev : d_out.domain_evidence) {
        ev_array.push_back(ev.to_json());
    }
    evidence_root["evidence"] = ev_array;

    std::ofstream ev_file(out_json_path);
    if (ev_file.is_open()) {
        ev_file << evidence_root.dump(2);
        out << "  - Evidence JSON saved to: " << out_json_path << '\n';
    }

    // 8. Generate Grounded Gemini / Gemma Prompt Payload
    out << "[STAGE 7] Generating grounded VLM prompt payload..." << '\n';
    nlohmann::json const gemini_payload = CurvEngine::domains::VlmPromptFormatter::formatGeminiRequest(
        evidence_root,
        "Analyze the career and longevity indicators strictly according to Vedic Samudrika Shastra rules.");

    std::ofstream prompt_file(out_vlm_prompt_path);
    if (prompt_file.is_open()) {
        prompt_file << gemini_payload.dump(2);
        out << "  - Grounded VLM prompt saved to: " << out_vlm_prompt_path << '\n';
    }

    out << "\n[PASS] Hand inspection pipeline completed successfully." << '\n';
    return 0;
}

} // namespace CurvEngine::tools
