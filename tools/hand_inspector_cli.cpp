#include <iostream>
#include <fstream>
#include <string>
#include <vector>
#include <memory>
#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>
#include <nlohmann/json.hpp>

#include "curv/Version.hpp"
#include "curv/Frame.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include "curv/domain/PalmParameters.hpp"
#include "curv/domain/PalmZones.hpp"
#include "curv/domain/VlmPromptFormatter.hpp"
#include "curv/domains/PalmDomainPack.hpp"
#include "curv/adapters/OverlayRenderer.hpp"
#include "curv/adapters/HandLandmarkDetector.hpp"

int main(int argc, char* argv[]) {
    std::cout << "==================================================" << std::endl;
    std::cout << "CurvEngine Hand Inspector & VLM Grounding CLI (v" << CurvEngine::getVersion() << ")" << std::endl;
    std::cout << "==================================================" << std::endl;

    std::string image_path;
    std::string landmarks_path;
    std::string auto_landmarks_model;
    std::string out_warp_path = "canonical_palm.png";
    std::string out_overlay_path = "overlay_palm.png";
    std::string out_json_path = "evidence.json";
    std::string out_vlm_prompt_path = "gemini_prompt.json";
    bool draw_whiskers = true;

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--image" && i + 1 < argc) {
            image_path = argv[++i];
        } else if (arg == "--landmarks" && i + 1 < argc) {
            landmarks_path = argv[++i];
        } else if (arg == "--auto-landmarks") {
            if (i + 1 < argc && argv[i + 1][0] != '-') {
                auto_landmarks_model = argv[++i];
            }
        } else if (arg == "--out-warp" && i + 1 < argc) {
            out_warp_path = argv[++i];
        } else if (arg == "--out-overlay" && i + 1 < argc) {
            out_overlay_path = argv[++i];
        } else if (arg == "--out-json" && i + 1 < argc) {
            out_json_path = argv[++i];
        } else if (arg == "--out-vlm-prompt" && i + 1 < argc) {
            out_vlm_prompt_path = argv[++i];
        }
    }

    if (image_path.empty()) {
        std::cout << "\nUsage: " << argv[0] << " --image <path> [options]\n\n"
                  << "Options:\n"
                  << "  --landmarks <path.json>      (custom 21-point landmarks JSON)\n"
                  << "  --auto-landmarks [model.onnx](auto landmark detection / heuristic)\n"
                  << "  --out-warp <canonical.png>   (default: canonical_palm.png)\n"
                  << "  --out-overlay <overlay.png>  (default: overlay_palm.png)\n"
                  << "  --out-vlm-prompt <prompt.json>(default: gemini_prompt.json)\n"
                  << "  --out-json <evidence.json>   (default: evidence.json)\n";
        return 0;
    }

    // 1. Ingest Image
    std::cout << "[STAGE 1] Loading image: " << image_path << std::endl;
    cv::Mat raw_image = cv::imread(image_path, cv::IMREAD_COLOR);
    if (raw_image.empty()) {
        std::cerr << "[ERROR] Could not load image: " << image_path << std::endl;
        return 1;
    }
    std::cout << "  - Input Resolution: " << raw_image.cols << "x" << raw_image.rows << std::endl;

    // 2. Extract or Load Hand Landmarks
    CurvEngine::geometry::HandLandmarks lms;
    lms.is_normalized = true;

    if (!landmarks_path.empty()) {
        std::cout << "[STAGE 2] Loading user landmarks: " << landmarks_path << std::endl;
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
        std::cout << "[STAGE 2] Running desktop hand landmark detector..." << std::endl;
        CurvEngine::adapters::HandLandmarkDetector detector(auto_landmarks_model);
        auto opt_lms = detector.detect(raw_image);
        if (opt_lms) {
            lms = *opt_lms;
            std::cout << "  - Landmarks established (Wrist: " << lms.points[0].x << ", " << lms.points[0].y << ")" << std::endl;
        }
    }

    // 3. Compute Canonical Palm Transform & Warp
    std::cout << "[STAGE 3] Warping to 512x512 canonical upright palm patch..." << std::endl;
    const int canonical_dim = 512;
    const auto transform = CurvEngine::geometry::computeCanonicalPalmTransform(
        lms,
        raw_image.cols,
        raw_image.rows,
        canonical_dim
    );

    cv::Mat canonical_patch = CurvEngine::geometry::warpToCanonical(
        raw_image,
        transform.M,
        canonical_dim
    );

    cv::imwrite(out_warp_path, canonical_patch);
    std::cout << "  - Canonical patch saved to: " << out_warp_path << std::endl;

    // 4. Extract Sub-Pixel Curvilinear Ridges with Adaptive Scale
    std::cout << "[STAGE 4] Extracting sub-pixel curvilinear ridges..." << std::endl;
    CurvEngine::Frame canonical_frame(canonical_patch, "canonical_palm", 1);

    CurvEngine::quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = 15.0;
    q_thresh.min_brightness = 5.0;
    auto quality_analyzer = std::make_shared<CurvEngine::quality::LaplacianQualityAnalyzer>(q_thresh);

    auto tuned_cfg = CurvEngine::domains::PalmParameters::computeFromLandmarks(
        lms,
        raw_image.cols,
        raw_image.rows
    );
    auto ridge_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(tuned_cfg);

    CurvEngine::pipeline::SyncPerceptionPipeline pipeline(quality_analyzer, ridge_extractor);
    auto result = pipeline.process(canonical_frame);

    std::cout << "  - Frame Usable: " << (result.is_usable ? "YES" : "NO") << std::endl;
    std::cout << "  - Blur Score: " << result.quality.blur_score << std::endl;
    std::cout << "  - Extracted Segments: " << result.ridge_graph.segments.size() << std::endl;

    // 5. Palm Domain Semantic Analysis & Zone Tagging
    std::cout << "[STAGE 5] Running PalmDomainPack semantic zone interpretation..." << std::endl;
    CurvEngine::domains::PalmDomainPack palm_pack;
    CurvEngine::DomainInput d_in{canonical_frame, result.ridge_graph, result.quality};
    auto d_out = palm_pack.process(d_in);

    // 6. Anti-Aliased Overlay Rendering
    std::cout << "[STAGE 6] Rendering sub-pixel overlay..." << std::endl;
    CurvEngine::adapters::OverlayConfig o_cfg;
    o_cfg.draw_subpixel_polylines = true;
    o_cfg.draw_normals = draw_whiskers;
    o_cfg.draw_hud = true;
    o_cfg.subpixel_shift = 4;

    cv::Mat overlay = CurvEngine::adapters::OverlayRenderer::render(
        canonical_patch,
        result.ridge_graph,
        result.quality,
        o_cfg
    );
    cv::imwrite(out_overlay_path, overlay);
    std::cout << "  - Overlay saved to: " << out_overlay_path << std::endl;

    // 7. Assemble Perception Evidence JSON
    nlohmann::json evidence_root;
    evidence_root["quality"] = {
        {"blur_score", result.quality.blur_score},
        {"brightness_score", result.quality.brightness_score},
        {"contrast_score", result.quality.contrast_score},
        {"is_usable", result.quality.is_usable}
    };
    evidence_root["domain_payload"] = d_out.domain_payload;

    nlohmann::json ev_array = nlohmann::json::array();
    for (const auto& ev : result.evidence) {
        ev_array.push_back(ev.to_json());
    }
    for (const auto& ev : d_out.domain_evidence) {
        ev_array.push_back(ev.to_json());
    }
    evidence_root["evidence"] = ev_array;

    std::ofstream ev_file(out_json_path);
    if (ev_file.is_open()) {
        ev_file << evidence_root.dump(2);
        std::cout << "  - Evidence JSON saved to: " << out_json_path << std::endl;
    }

    // 8. Generate Grounded Gemini / Gemma Prompt Payload
    std::cout << "[STAGE 7] Generating grounded VLM prompt payload..." << std::endl;
    nlohmann::json gemini_payload = CurvEngine::domains::VlmPromptFormatter::formatGeminiRequest(
        evidence_root,
        "Analyze the career and longevity indicators strictly according to Vedic Samudrika Shastra rules."
    );

    std::ofstream prompt_file(out_vlm_prompt_path);
    if (prompt_file.is_open()) {
        prompt_file << gemini_payload.dump(2);
        std::cout << "  - Grounded VLM prompt saved to: " << out_vlm_prompt_path << std::endl;
    }

    std::cout << "\n[PASS] Hand inspection pipeline completed successfully." << std::endl;
    return 0;
}
