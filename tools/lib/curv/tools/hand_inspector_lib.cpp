#include "curv/tools/hand_inspector_lib.hpp"
#include "curv/tools/palm_inspect_lib.hpp"

#include <fstream>
#include <opencv2/imgcodecs.hpp>
#include <string>

#include "curv/Version.hpp"
#include "curv/domain/VlmPromptFormatter.hpp"

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

    if (image_path.empty()) {
        out << "\nUsage: " << argv[0] << " --image <path> [options]\n\n"
            << "Options:\n"
            << "  --landmarks <path.json>      (custom 21-point landmarks JSON)\n"
            << "  --auto-landmarks [model.onnx](auto landmark detection / heuristic)\n"
            << "  --left-hand                  (hint left hand for landmarks/zones)\n"
            << "  --debug-all-ridges           (render all raw ridge fragments)\n"
            << "  --out-warp <canonical.png>   (default: canonical_palm.png)\n"
            << "  --out-overlay <overlay.png>  (default: overlay_palm.png)\n"
            << "  --out-full-overlay <full.png>\n"
            << "  --out-vlm-prompt <prompt.json>(default: gemini_prompt.json)\n"
            << "  --out-json <evidence.json>   (default: evidence.json)\n";
        return 0;
    }

    out << "[STAGE 1] Loading image: " << image_path << '\n';
    cv::Mat const raw_image = cv::imread(image_path, cv::IMREAD_COLOR);
    if (raw_image.empty()) {
        err << "[ERROR] Could not load image: " << image_path << '\n';
        return 1;
    }
    out << "  - Input Resolution: " << raw_image.cols << "x" << raw_image.rows << '\n';

    geometry::HandLandmarks lms;
    const geometry::HandLandmarks* lms_ptr = nullptr;
    if (!landmarks_path.empty()) {
        std::string lerr;
        if (!loadLandmarksJson(landmarks_path, lms, &lerr)) {
            err << "[ERROR] " << lerr << '\n';
            return 1;
        }
        lms_ptr = &lms;
        out << "[STAGE 2] Loaded landmarks from " << landmarks_path << '\n';
    }

    PalmInspectOptions opts;
    opts.is_left_hand_hint = is_left_hand_cli;
    opts.debug_all_ridges = debug_all_ridges;
    opts.auto_landmarks_model = auto_landmarks_model;
    // When --auto-landmarks is passed without a file, force detector path even for left-hand.
    if (landmarks_path.empty()) {
        // Match prior CLI: bare --auto-landmarks uses detector; left-hand alone used heuristic.
        // If user passed --auto-landmarks (even empty model), prefer detector unless only --left-hand.
        bool auto_flag = false;
        for (int i = 1; i < argc; ++i) {
            if (std::string(argv[i]) == "--auto-landmarks") {
                auto_flag = true;
                break;
            }
        }
        if (auto_flag) {
            opts.auto_landmarks_model = auto_landmarks_model; // may be empty = default
        } else if (!is_left_hand_cli) {
            opts.auto_landmarks_model = ""; // detector default
        }
    }

    auto result = runPalmInspect(raw_image, lms_ptr, opts, &out);
    if (!result.ok) {
        err << "[ERROR] " << result.error << '\n';
        return 1;
    }

    cv::imwrite(out_warp_path, result.canonical_bgr);
    out << "  - Canonical patch saved to: " << out_warp_path << '\n';
    cv::imwrite(out_overlay_path, result.canonical_overlay);
    out << "  - Canonical overlay saved to: " << out_overlay_path << '\n';

    if (!out_full_overlay_path.empty()) {
        cv::imwrite(out_full_overlay_path, result.full_overlay);
        out << "  - Full image overlay saved to: " << out_full_overlay_path << '\n';
    }

    for (const auto& [name, line] : result.major_lines) {
        if (line.is_valid) {
            out << "  - " << name << ": " << line.constituent_segment_ids.size() << " fragments, "
                << static_cast<int>(line.total_length) << " px, conf=" << line.confidence << '\n';
        }
    }

    std::ofstream ev_file(out_json_path);
    if (ev_file.is_open()) {
        ev_file << result.evidence.dump(2);
        out << "  - Evidence JSON saved to: " << out_json_path << '\n';
    }

    {
        std::string lms_path = out_json_path;
        const auto dot = lms_path.find_last_of('.');
        if (dot != std::string::npos) {
            lms_path = lms_path.substr(0, dot) + "_landmarks.json";
        } else {
            lms_path += "_landmarks.json";
        }
        if (saveLandmarksJson(lms_path, result.landmarks)) {
            out << "  - Landmarks JSON saved to: " << lms_path << '\n';
        }
    }

    out << "[STAGE 7] Generating grounded VLM prompt payload..." << '\n';
    nlohmann::json const gemini_payload = domains::VlmPromptFormatter::formatGeminiRequest(
        result.evidence, "Analyze the career and longevity indicators strictly according to Vedic Samudrika Shastra rules.");
    std::ofstream prompt_file(out_vlm_prompt_path);
    if (prompt_file.is_open()) {
        prompt_file << gemini_payload.dump(2);
        out << "  - Grounded VLM prompt saved to: " << out_vlm_prompt_path << '\n';
    }

    out << "\n[PASS] Hand inspection pipeline completed successfully." << '\n';
    return 0;
}

} // namespace CurvEngine::tools
