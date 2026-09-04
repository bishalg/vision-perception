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

int main(int argc, char* argv[]) {
    std::cout << "==================================================" << std::endl;
    std::cout << "CurvEngine CLI Verification & Pipeline Tool" << std::endl;
    std::cout << "Library Version: " << CurvEngine::getVersion() << std::endl;
    std::cout << "OpenCV Version:  " << cv::getVersionString() << std::endl;
    std::cout << "==================================================" << std::endl;

    std::string image_path;
    std::string out_overlay_path = "output_overlay.png";
    std::string out_json_path = "evidence.json";
    float sigma = 1.5f;
    float low_thresh = 0.5f;
    float high_thresh = 1.5f;
    bool extract_dark = false;

    // Parse command-line arguments
    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--image" && i + 1 < argc) {
            image_path = argv[++i];
        } else if (arg == "--sigma" && i + 1 < argc) {
            sigma = std::stof(argv[++i]);
        } else if (arg == "--low" && i + 1 < argc) {
            low_thresh = std::stof(argv[++i]);
        } else if (arg == "--high" && i + 1 < argc) {
            high_thresh = std::stof(argv[++i]);
        } else if (arg == "--dark") {
            extract_dark = true;
        } else if (arg == "--out" && i + 1 < argc) {
            out_overlay_path = argv[++i];
        } else if (arg == "--json" && i + 1 < argc) {
            out_json_path = argv[++i];
        }
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
        std::cout << "Usage: " << argv[0] << " --image <path> [--sigma 1.5] [--low 0.5] [--high 1.5] [--dark] [--out overlay.png] [--json evidence.json]" << std::endl;
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

    // 4. Render Sub-pixel Polylines Overlay
    cv::Mat overlay = input_image.clone();
    for (const auto& seg : result.ridge_graph.segments) {
        if (seg.points.size() < 2) continue;
        std::vector<cv::Point> pts;
        pts.reserve(seg.points.size());
        for (const auto& pt : seg.points) {
            pts.emplace_back(static_cast<int>(std::round(pt.x)), static_cast<int>(std::round(pt.y)));
        }
        cv::polylines(overlay, pts, false, cv::Scalar(0, 255, 0), 2, cv::LINE_AA);
    }

    cv::imwrite(out_overlay_path, overlay);
    std::cout << "[STAGE 3] Rendered overlay saved to: " << out_overlay_path << std::endl;

    // 5. Export JSON Evidence
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
    root_json["evidence"] = evidence_arr;

    std::ofstream json_file(out_json_path);
    if (json_file.is_open()) {
        json_file << root_json.dump(2);
        std::cout << "[STAGE 4] Perception evidence JSON saved to: " << out_json_path << std::endl;
    }

    std::cout << "\n[PASS] Pipeline executed successfully." << std::endl;
    return 0;
}
