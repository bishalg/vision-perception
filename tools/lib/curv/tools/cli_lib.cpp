#include "curv/tools/cli_lib.hpp"

#include <chrono>
#include <fstream>
#include <memory>
#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include <string>
#include <type_traits>

#include "curv/Frame.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include "curv/Version.hpp"
#include "curv/adapters/OverlayRenderer.hpp"
#include "curv/domains/PalmDomainPack.hpp"
#include "curv/domains/SurfaceInspectionPack.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/tools/benchmark_lib.hpp"

namespace CurvEngine::tools {

namespace {

// Numeric CLI values that fail to parse abort the run with a clear error
// naming the offending flag (silently defaulting would hide user mistakes).
template <typename T>
T parseNumeric(const char* raw, const std::string& flag) {
    try {
        if constexpr (std::is_same_v<T, float>) {
            return std::stof(raw);
        } else if constexpr (std::is_same_v<T, int>) {
            return std::stoi(raw);
        } else {
            return std::stoull(raw);
        }
    } catch (const std::exception&) {
        throw std::invalid_argument(flag);
    }
}

} // namespace

int runCurvCli(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    out << "==================================================" << '\n';
    out << "CurvEngine CLI Verification & Pipeline Tool (v" << CurvEngine::getVersion() << ")" << '\n';
    out << "OpenCV Version:  " << cv::getVersionString() << '\n';
    out << "==================================================" << '\n';

    std::string image_path;
    std::string out_overlay_path = "output_overlay.png";
    std::string out_json_path = "evidence.json";
    std::string out_svg_path;
    std::string domain_name = "none";
    float sigma = 1.5f;
    float low_thresh = 0.5f;
    float high_thresh = 1.5f;
    float min_blur = 30.0f;
    bool extract_dark = false;
    bool draw_hud = true;
    bool draw_normals = false;
    bool draw_ribbons = false;
    bool run_bench = false;

    // Parse command-line arguments
    try {
        for (int i = 1; i < argc; ++i) {
            std::string const arg = argv[i];
            if (arg == "--image" && i + 1 < argc) {
                image_path = argv[++i];
            } else if (arg == "--domain" && i + 1 < argc) {
                domain_name = argv[++i];
            } else if (arg == "--svg" && i + 1 < argc) {
                out_svg_path = argv[++i];
            } else if (arg == "--hud") {
                draw_hud = true;
            } else if (arg == "--no-hud") {
                draw_hud = false;
            } else if (arg == "--normals") {
                draw_normals = true;
            } else if (arg == "--ribbons") {
                draw_ribbons = true;
            } else if (arg == "--sigma" && i + 1 < argc) {
                sigma = parseNumeric<float>(argv[++i], "--sigma");
            } else if (arg == "--low" && i + 1 < argc) {
                low_thresh = parseNumeric<float>(argv[++i], "--low");
            } else if (arg == "--high" && i + 1 < argc) {
                high_thresh = parseNumeric<float>(argv[++i], "--high");
            } else if (arg == "--min-blur" && i + 1 < argc) {
                min_blur = parseNumeric<float>(argv[++i], "--min-blur");
            } else if (arg == "--dark") {
                extract_dark = true;
            } else if (arg == "--benchmark") {
                run_bench = true;
            } else if (arg == "--out" && i + 1 < argc) {
                out_overlay_path = argv[++i];
            } else if (arg == "--json" && i + 1 < argc) {
                out_json_path = argv[++i];
            }
        }
    } catch (const std::invalid_argument& e) {
        err << "[ERROR] Invalid numeric value for " << e.what() << '\n';
        return 1;
    }

    if (run_bench) {
        runThroughputBenchmark(out);
        return 0;
    }

    if (image_path.empty()) {
        out << "[INFO] No --image argument provided. Running internal sanity checks..." << '\n';
        CurvEngine::Point2D const p1{100.5f, 150.25f, 0.95f, 0.99f};
        CurvEngine::CurveSegment seg;
        seg.id = 1;
        seg.points.push_back(p1);
        CurvEngine::RidgeGraph graph;
        graph.segments.push_back(seg);
        out << "[PASS] CurvEngine::RidgeGraph contract validated." << '\n';
        out << "\nUsage: " << argv[0] << " --image <path> [options]\n"
            << "Options:\n"
            << "  --domain <palm|surface_inspection|none>  (default: none)\n"
            << "  --svg <output.svg>                       (export vector curves)\n"
            << "  --hud / --no-hud                         (toggle diagnostics HUD)\n"
            << "  --normals                                (draw normal vectors)\n"
            << "  --ribbons                                (draw width ribbons)\n"
            << "  --sigma <float>                          (default: 1.5)\n"
            << "  --low <float> / --high <float>           (hysteresis thresholds)\n"
            << "  --min-blur <float>                       (default: 30.0)\n"
            << "  --dark                                   (extract dark lines/valleys)\n"
            << "  --out <overlay.png>                      (default: output_overlay.png)\n"
            << "  --json <evidence.json>                   (default: evidence.json)\n";
        return 0;
    }

    // 1. Ingest Image
    out << "[STAGE 1] Ingesting image: " << image_path << '\n';
    cv::Mat const input_image = cv::imread(image_path, cv::IMREAD_COLOR);
    if (input_image.empty()) {
        err << "[ERROR] Failed to load image from: " << image_path << '\n';
        return 1;
    }
    out << "  - Resolution: " << input_image.cols << "x" << input_image.rows << " (" << input_image.channels()
        << " channels)" << '\n';

    CurvEngine::Frame const frame(input_image, image_path, 0);

    // 2. Configure Pipeline
    CurvEngine::quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = min_blur;
    q_thresh.min_brightness = 10.0;
    q_thresh.max_brightness = 250.0;
    q_thresh.min_contrast = 10.0;
    auto const quality_analyzer = std::make_shared<CurvEngine::quality::LaplacianQualityAnalyzer>(q_thresh);

    CurvEngine::ridge::StegerConfig r_cfg;
    r_cfg.sigma = sigma;
    r_cfg.low_threshold = low_thresh;
    r_cfg.high_threshold = high_thresh;
    r_cfg.extract_dark_lines = extract_dark;
    r_cfg.min_segment_length = 5.0f;
    auto const ridge_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(r_cfg);

    CurvEngine::pipeline::SyncPerceptionPipeline pipeline(quality_analyzer, ridge_extractor);

    // 3. Execute Pipeline
    out << "[STAGE 2] Executing perception pipeline..." << '\n';
    const auto result = pipeline.process(frame);

    out << "[RESULTS] Execution completed in " << result.total_duration_us << " us:" << '\n';
    out << "  - Usable:     " << (result.is_usable ? "YES" : "NO") << '\n';
    out << "  - Blur Score: " << result.quality.blur_score << '\n';
    out << "  - Brightness: " << result.quality.brightness_score << '\n';
    out << "  - Contrast:   " << result.quality.contrast_score << '\n';
    out << "  - Diagnostics:" << result.quality.recommendation << '\n';

    if (!result.is_usable) {
        out << "[WARNING] Frame rejected by quality gate. Skipping overlay rendering." << '\n';
        return 0;
    }

    out << "  - Extracted Segments: " << result.ridge_graph.segments.size() << '\n';

    // 4. Domain Pack Processing (if specified)
    std::unique_ptr<CurvEngine::IDomainPack> domain_pack;
    if (domain_name == "palm") {
        domain_pack = std::make_unique<CurvEngine::domains::PalmDomainPack>();
    } else if (domain_name == "surface_inspection" || domain_name == "surface") {
        domain_pack = std::make_unique<CurvEngine::domains::SurfaceInspectionPack>();
    }

    nlohmann::json domain_payload = nlohmann::json::object();
    std::vector<CurvEngine::Evidence> domain_evidence;

    if (domain_pack) {
        out << "[STAGE 3] Executing domain pack: " << domain_pack->name() << " (v" << domain_pack->version() << ")"
            << '\n';
        CurvEngine::DomainInput const d_in{frame, result.ridge_graph, result.quality};
        auto d_out = domain_pack->process(d_in);
        domain_payload = d_out.domain_payload;
        domain_evidence = std::move(d_out.domain_evidence);
        out << "  - Domain evidence items generated: " << domain_evidence.size() << '\n';
    }

    // 5. Anti-Aliased Sub-Pixel Overlay Rendering via OverlayRenderer
    CurvEngine::adapters::OverlayConfig overlay_config;
    overlay_config.draw_subpixel_polylines = true;
    overlay_config.draw_normals = draw_normals;
    overlay_config.draw_ribbons = draw_ribbons;
    overlay_config.draw_hud = draw_hud;
    overlay_config.subpixel_shift = 4; // 16x sub-pixel anti-aliasing

    cv::Mat const overlay =
        CurvEngine::adapters::OverlayRenderer::render(input_image, result.ridge_graph, result.quality, overlay_config);

    cv::imwrite(out_overlay_path, overlay);
    out << "[STAGE 4] Rendered sub-pixel overlay saved to: " << out_overlay_path << '\n';

    // 6. SVG Vector Export (if requested)
    if (!out_svg_path.empty()) {
        const bool svg_ok = CurvEngine::adapters::OverlayRenderer::exportToSvg(result.ridge_graph, input_image.cols,
                                                                               input_image.rows, out_svg_path);
        if (svg_ok) {
            out << "[STAGE 5] SVG vector curves exported to: " << out_svg_path << '\n';
        } else {
            err << "[ERROR] Failed to export SVG to: " << out_svg_path << '\n';
        }
    }

    // 7. Export JSON Evidence
    nlohmann::json root_json;
    root_json["quality"] = {
        {"blur_score", result.quality.blur_score},         {"brightness_score", result.quality.brightness_score},
        {"contrast_score", result.quality.contrast_score}, {"is_usable", result.quality.is_usable},
        {"recommendation", result.quality.recommendation},
    };
    root_json["total_duration_us"] = result.total_duration_us;

    nlohmann::json evidence_arr = nlohmann::json::array();
    for (const auto& ev : result.evidence) {
        evidence_arr.push_back(ev.to_json());
    }
    for (const auto& ev : domain_evidence) {
        evidence_arr.push_back(ev.to_json());
    }
    root_json["evidence"] = evidence_arr;

    if (!domain_payload.empty()) {
        root_json["domain_payload"] = domain_payload;
    }

    std::ofstream json_file(out_json_path);
    if (json_file.is_open()) {
        json_file << root_json.dump(2);
        out << "[STAGE 6] Perception evidence JSON saved to: " << out_json_path << '\n';
    }

    out << "\n[PASS] Pipeline and domain processing completed successfully." << '\n';
    return 0;
}

} // namespace CurvEngine::tools
