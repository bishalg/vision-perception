#include "curv/tools/video_pipeline_demo_lib.hpp"

#include <algorithm>
#include <filesystem>
#include <memory>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include <opencv2/videoio.hpp>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <utility>

#include "curv/Frame.hpp"
#include "curv/adapters/CctvVideoSource.hpp"
#include "curv/adapters/OverlayRenderer.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

namespace CurvEngine::tools {

namespace {

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

void printUsage(std::ostream& out) {
    out << "Usage: video_pipeline_demo --input <video> --output <annotated.avi|mp4> [--fps-stride N]\n"
        << "  --input PATH       Recorded CCTV clip (mp4/avi/mkv)\n"
        << "  --output PATH      Annotated output video path\n"
        << "  --fps-stride N     Process every Nth frame (default: 1)\n"
        << "  --side-by-side     Write raw|annotated dual panel\n"
        << "  --sigma F          Steger sigma (default: 1.5)\n"
        << "  --min-blur F       Laplacian blur gate (default: 20)\n";
}

} // namespace

int runVideoPipelineDemo(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    std::string input_path;
    std::string output_path;
    int fps_stride = 1;
    bool side_by_side = false;
    float sigma = 1.5f;
    const float low_thresh = 0.5f;
    const float high_thresh = 1.5f;
    float min_blur = 20.0f;

    try {
        for (int i = 1; i < argc; ++i) {
            const std::string arg = argv[i];
            if ((arg == "--input" || arg == "-i") && i + 1 < argc) {
                input_path = argv[++i];
            } else if ((arg == "--output" || arg == "-o") && i + 1 < argc) {
                output_path = argv[++i];
            } else if (arg == "--fps-stride" && i + 1 < argc) {
                fps_stride = parseNumeric<int>(argv[++i], "--fps-stride");
            } else if (arg == "--side-by-side") {
                side_by_side = true;
            } else if (arg == "--sigma" && i + 1 < argc) {
                sigma = parseNumeric<float>(argv[++i], "--sigma");
            } else if (arg == "--min-blur" && i + 1 < argc) {
                min_blur = parseNumeric<float>(argv[++i], "--min-blur");
            } else if (arg == "--help" || arg == "-h") {
                printUsage(out);
                return 0;
            } else {
                err << "[ERROR] Unknown or incomplete argument: " << arg << '\n';
                printUsage(err);
                return 1;
            }
        }
    } catch (const std::invalid_argument& ex) {
        err << "[ERROR] Invalid value for " << ex.what() << '\n';
        return 1;
    }

    if (input_path.empty() || output_path.empty()) {
        err << "[ERROR] --input and --output are required\n";
        printUsage(err);
        return 1;
    }

    adapters::CctvMetadata meta;
    meta.camera_id = "video_pipeline_demo";
    meta.name = std::filesystem::path(input_path).stem().string();
    adapters::CctvVideoSource source(meta);
    if (!source.open(input_path)) {
        err << "[ERROR] Failed to open video: " << input_path << '\n';
        return 1;
    }
    source.setFpsStride(fps_stride);

    quality::QualityThresholds thresholds;
    thresholds.min_blur_score = static_cast<double>(min_blur);
    thresholds.min_brightness = 10.0;
    thresholds.max_brightness = 250.0;
    thresholds.min_contrast = 5.0;

    ridge::StegerConfig steger_cfg;
    steger_cfg.sigma = sigma;
    steger_cfg.low_threshold = low_thresh;
    steger_cfg.high_threshold = high_thresh;
    steger_cfg.extract_dark_lines = false;

    const auto analyzer = std::make_shared<quality::LaplacianQualityAnalyzer>(thresholds);
    const auto extractor = std::make_shared<ridge::StegerRidgeExtractor>(steger_cfg);
    pipeline::SyncPerceptionPipeline pipeline(analyzer, extractor);

    adapters::OverlayConfig overlay_cfg;
    overlay_cfg.draw_hud = true;

    Frame first;
    if (!source.readNext(first)) {
        err << "[ERROR] Video contained no readable frames\n";
        return 1;
    }

    const int width = first.width();
    const int height = first.height();
    const int out_w = side_by_side ? width * 2 : width;
    const int out_h = height;

    const std::filesystem::path out_path(output_path);
    if (out_path.has_parent_path()) {
        std::filesystem::create_directories(out_path.parent_path());
    }

    const double src_fps = source.sourceFps();
    const double write_fps = std::max(1.0, src_fps > 1.0 ? src_fps / static_cast<double>(fps_stride) : 10.0);

    // Prefer MJPG/AVI for cross-platform demos; fall back to mp4v if extension is .mp4.
    const std::string ext = out_path.extension().string();
    const int fourcc = (ext == ".mp4" || ext == ".MP4") ? cv::VideoWriter::fourcc('m', 'p', '4', 'v')
                                                        : cv::VideoWriter::fourcc('M', 'J', 'P', 'G');

    cv::VideoWriter writer(output_path, fourcc, write_fps, cv::Size(out_w, out_h));
    if (!writer.isOpened()) {
        err << "[ERROR] Failed to open VideoWriter for: " << output_path << '\n';
        return 1;
    }

    out << "==================================================\n"
        << "video_pipeline_demo (Phase 2)\n"
        << "  input:      " << input_path << '\n'
        << "  output:     " << output_path << '\n'
        << "  fps-stride: " << fps_stride << '\n'
        << "  source fps: " << src_fps << '\n'
        << "  write fps:  " << write_fps << '\n'
        << "  side-by-side: " << (side_by_side ? "yes" : "no") << '\n'
        << "==================================================\n";

    const auto process_and_write = [&](const Frame& frame) {
        const auto result = pipeline.process(frame);
        cv::Mat annotated =
            adapters::OverlayRenderer::render(frame.image, result.ridge_graph, result.quality, overlay_cfg);

        cv::Mat out_frame;
        if (side_by_side) {
            out_frame = cv::Mat(out_h, out_w, CV_8UC3);
            frame.image.copyTo(out_frame(cv::Rect(0, 0, width, height)));
            annotated.copyTo(out_frame(cv::Rect(width, 0, width, height)));
            cv::putText(out_frame, "RAW", cv::Point(12, 28), cv::FONT_HERSHEY_SIMPLEX, 0.7, {0, 200, 255}, 2);
            cv::putText(out_frame, "ANNOTATED", cv::Point(width + 12, 28), cv::FONT_HERSHEY_SIMPLEX, 0.7, {0, 255, 0},
                        2);
        } else {
            out_frame = std::move(annotated);
        }
        writer.write(out_frame);

        out << "frame " << source.framesEmitted() << "  usable=" << (result.is_usable ? "Y" : "N")
            << "  blur=" << result.quality.blur_score << "  segs=" << result.ridge_graph.segments.size() << '\n';
    };

    process_and_write(first);
    Frame frame;
    while (source.readNext(frame)) {
        process_and_write(frame);
    }

    writer.release();
    source.close();

    out << "Done. Emitted " << source.framesEmitted() << " frames (" << source.framesDecoded() << " decoded) → "
        << output_path << '\n';
    return 0;
}

} // namespace CurvEngine::tools
