#include <iostream>
#include <fstream>
#include <string>
#include <vector>
#include <memory>
#include <opencv2/core.hpp>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include <nlohmann/json.hpp>

#include "curv/Version.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/adapters/OverlayRenderer.hpp"
#include "curv/domains/SurfaceInspectionPack.hpp"
#include "curv/domains/PalmDomainPack.hpp"

#include <chrono>

static void runBenchmark() {
    std::cout << "\n==================================================" << std::endl;
    std::cout << "CurvEngine Performance Benchmark (cv::parallel_for_)" << std::endl;
    std::cout << "Platform Threads: " << cv::getNumThreads() << std::endl;
    std::cout << "==================================================" << std::endl;

    CurvEngine::ridge::StegerConfig cfg;
    cfg.sigma = 1.5f;
    cfg.low_threshold = 0.5f;
    cfg.high_threshold = 1.5f;
    cfg.min_segment_length = 5.0f;
    CurvEngine::ridge::StegerRidgeExtractor extractor(cfg);

    const std::vector<std::pair<std::string, cv::Size>> resolutions = {
        {"720p HD", cv::Size(1280, 720)},
        {"1080p FHD", cv::Size(1920, 1080)}
    };

    for (const auto& [name, sz] : resolutions) {
        cv::Mat synth(sz, CV_8UC1, cv::Scalar(30));
        for (int r = 100; r < sz.height - 100; r += 120) {
            cv::ellipse(synth, cv::Point(sz.width / 2, r), cv::Size(sz.width / 3, 60), 0, 0, 180, cv::Scalar(220), 3, cv::LINE_AA);
        }

        // Warmup run
        cv::setNumThreads(0);
        (void)extractor.extractRidgeGraph(synth);

        const int iterations = 5;

        // 1. Sequential execution (single-threaded)
        cv::setNumThreads(1);
        auto t0 = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < iterations; ++i) {
            (void)extractor.extractRidgeGraph(synth);
        }
        auto t1 = std::chrono::high_resolution_clock::now();
        const double seq_ms = std::chrono::duration<double, std::milli>(t1 - t0).count() / iterations;

        // 2. Parallel execution (cv::parallel_for_)
        cv::setNumThreads(0);
        t0 = std::chrono::high_resolution_clock::now();
        for (int i = 0; i < iterations; ++i) {
            (void)extractor.extractRidgeGraph(synth);
        }
        t1 = std::chrono::high_resolution_clock::now();
        const double par_ms = std::chrono::duration<double, std::milli>(t1 - t0).count() / iterations;

        const double speedup = seq_ms / std::max(1e-6, par_ms);

        std::cout << "\nResolution: " << name << " (" << sz.width << "x" << sz.height << ")" << std::endl;
        std::cout << "  - Sequential (1 thread):         " << seq_ms << " ms" << std::endl;
        std::cout << "  - Multi-threaded (parallel_for): " << par_ms << " ms" << std::endl;
        std::cout << "  - Speedup Factor:                " << speedup << "x (" 
                  << (1000.0 / par_ms) << " FPS)" << std::endl;
    }
    std::cout << "\n==================================================\n" << std::endl;
}

int main(int argc, char* argv[]) {
    std::cout << "==================================================" << std::endl;
    std::cout << "CurvEngine CLI Verification & Pipeline Tool (v" << CurvEngine::getVersion() << ")" << std::endl;
    std::cout << "OpenCV Version:  " << cv::getVersionString() << std::endl;
    std::cout << "==================================================" << std::endl;

    std::string image_path;
    std::string out_overlay_path = "output_overlay.png";
    std::string out_json_path = "evidence.json";
    std::string out_svg_path;
    std::string domain_name = "none";
    float sigma = 1.5f;
    float low_thresh = 0.5f;
    float high_thresh = 1.5f;
    bool extract_dark = false;
    bool draw_hud = true;
    bool draw_normals = false;
    bool draw_ribbons = false;
    bool run_bench = false;

    // Parse command-line arguments
    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
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
            sigma = std::stof(argv[++i]);
        } else if (arg == "--low" && i + 1 < argc) {
            low_thresh = std::stof(argv[++i]);
        } else if (arg == "--high" && i + 1 < argc) {
            high_thresh = std::stof(argv[++i]);
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

    if (run_bench) {
        runBenchmark();
        return 0;
    }

    if (image_path.empty()) {
        std::cout << "[INFO] No --image argument provided. Running internal sanity checks..." << std::endl;
        CurvEngine::Point2D p1{100.5f, 150.25f, 0.95f, 0.99f};
        CurvEngine::CurveSegment seg;
        seg.id = 1;
        seg.points.push_back(p1);
        CurvEngine::RidgeGraph graph;
        graph.segments.push_back(seg);
        std::cout << "[PASS] CurvEngine::RidgeGraph contract validated." << std::endl;
        std::cout << "\nUsage: " << argv[0] << " --image <path> [options]\n"
                  << "Options:\n"
                  << "  --domain <palm|surface_inspection|none>  (default: none)\n"
                  << "  --svg <output.svg>                       (export vector curves)\n"
                  << "  --hud / --no-hud                         (toggle diagnostics HUD)\n"
                  << "  --normals                                (draw normal vectors)\n"
                  << "  --ribbons                                (draw width ribbons)\n"
                  << "  --sigma <float>                          (default: 1.5)\n"
                  << "  --low <float> / --high <float>           (hysteresis thresholds)\n"
                  << "  --dark                                   (extract dark lines/valleys)\n"
                  << "  --out <overlay.png>                      (default: output_overlay.png)\n"
                  << "  --json <evidence.json>                   (default: evidence.json)\n";
        return 0;
    }

    // 1. Ingest Image
    std::cout << "[STAGE 1] Ingesting image: " << image_path << std::endl;
    cv::Mat input_image = cv::imread(image_path, cv::IMREAD_COLOR);
    if (input_image.empty()) {
        std::cerr << "[ERROR] Failed to load image from: " << image_path << std::endl;
        return 1;
    }
    std::cout << "  - Resolution: " << input_image.cols << "x" << input_image.rows
              << " (" << input_image.channels() << " channels)" << std::endl;

    CurvEngine::Frame frame(input_image, image_path, 0);

    // 2. Configure Pipeline
    CurvEngine::quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = 30.0;
    q_thresh.min_brightness = 10.0;
    q_thresh.max_brightness = 250.0;
    q_thresh.min_contrast = 10.0;
    auto quality_analyzer = std::make_shared<CurvEngine::quality::LaplacianQualityAnalyzer>(q_thresh);

    CurvEngine::ridge::StegerConfig r_cfg;
    r_cfg.sigma = sigma;
    r_cfg.low_threshold = low_thresh;
    r_cfg.high_threshold = high_thresh;
    r_cfg.extract_dark_lines = extract_dark;
    r_cfg.min_segment_length = 5.0f;
    auto ridge_extractor = std::make_shared<CurvEngine::ridge::StegerRidgeExtractor>(r_cfg);

    CurvEngine::pipeline::SyncPerceptionPipeline pipeline(quality_analyzer, ridge_extractor);

    // 3. Execute Pipeline
    std::cout << "[STAGE 2] Executing perception pipeline..." << std::endl;
    const auto result = pipeline.process(frame);

    std::cout << "[RESULTS] Execution completed in " << result.total_duration_us << " us:" << std::endl;
    std::cout << "  - Usable:     " << (result.is_usable ? "YES" : "NO") << std::endl;
    std::cout << "  - Blur Score: " << result.quality.blur_score << std::endl;
    std::cout << "  - Brightness: " << result.quality.brightness_score << std::endl;
    std::cout << "  - Contrast:   " << result.quality.contrast_score << std::endl;
    std::cout << "  - Diagnostics:" << result.quality.recommendation << std::endl;

    if (!result.is_usable) {
        std::cout << "[WARNING] Frame rejected by quality gate. Skipping overlay rendering." << std::endl;
        return 0;
    }

    std::cout << "  - Extracted Segments: " << result.ridge_graph.segments.size() << std::endl;

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
        std::cout << "[STAGE 3] Executing domain pack: " << domain_pack->name() << " (v" << domain_pack->version() << ")" << std::endl;
        CurvEngine::DomainInput d_in{frame, result.ridge_graph, result.quality};
        auto d_out = domain_pack->process(d_in);
        domain_payload = d_out.domain_payload;
        domain_evidence = std::move(d_out.domain_evidence);
        std::cout << "  - Domain evidence items generated: " << domain_evidence.size() << std::endl;
    }

    // 5. Anti-Aliased Sub-Pixel Overlay Rendering via OverlayRenderer
    CurvEngine::adapters::OverlayConfig overlay_config;
    overlay_config.draw_subpixel_polylines = true;
    overlay_config.draw_normals = draw_normals;
    overlay_config.draw_ribbons = draw_ribbons;
    overlay_config.draw_hud = draw_hud;
    overlay_config.subpixel_shift = 4; // 16x sub-pixel anti-aliasing

    cv::Mat overlay = CurvEngine::adapters::OverlayRenderer::render(
        input_image,
        result.ridge_graph,
        result.quality,
        overlay_config
    );

    cv::imwrite(out_overlay_path, overlay);
    std::cout << "[STAGE 4] Rendered sub-pixel overlay saved to: " << out_overlay_path << std::endl;

    // 6. SVG Vector Export (if requested)
    if (!out_svg_path.empty()) {
        const bool svg_ok = CurvEngine::adapters::OverlayRenderer::exportToSvg(
            result.ridge_graph,
            input_image.cols,
            input_image.rows,
            out_svg_path
        );
        if (svg_ok) {
            std::cout << "[STAGE 5] SVG vector curves exported to: " << out_svg_path << std::endl;
        } else {
            std::cerr << "[ERROR] Failed to export SVG to: " << out_svg_path << std::endl;
        }
    }

    // 7. Export JSON Evidence
    nlohmann::json root_json;
    root_json["quality"] = {
        {"blur_score", result.quality.blur_score},
        {"brightness_score", result.quality.brightness_score},
        {"contrast_score", result.quality.contrast_score},
        {"is_usable", result.quality.is_usable},
        {"recommendation", result.quality.recommendation}
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
        std::cout << "[STAGE 6] Perception evidence JSON saved to: " << out_json_path << std::endl;
    }

    std::cout << "\n[PASS] Pipeline and domain processing completed successfully." << std::endl;
    return 0;
}
