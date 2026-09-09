#include "curv/tools/live_snapshot_demo_lib.hpp"

#include <chrono>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>
#include <type_traits>

#include "curv/Frame.hpp"
#include "curv/adapters/CctvHttpSnapshot.hpp"
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
    out << "Usage: live_snapshot_demo --url <url|camera_id|mock://id> [options]\n"
        << "  --url / -u URL       Snapshot URL, camera registry id, file path, or mock://id\n"
        << "  --interval-sec N     Poll interval seconds (default: 5)\n"
        << "  --duration-sec N     Total run duration seconds (default: 30; 0 = forever)\n"
        << "  --cache-dir PATH     Disk cache directory (default: data/fixtures/cctv/http_cache)\n"
        << "  --direct             Prefer live network over warm cache\n"
        << "  --timeout-ms N       HTTP timeout (default: 10000)\n"
        << "  --help               Show this help\n";
}

} // namespace

int runLiveSnapshotDemo(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    std::string url;
    int interval_sec = 5;
    int duration_sec = 30;
    std::string cache_dir = "data/fixtures/cctv/http_cache";
    bool prefer_cache = true;
    int timeout_ms = 10000;

    try {
        for (int i = 1; i < argc; ++i) {
            const std::string arg = argv[i];
            if ((arg == "--url" || arg == "-u") && i + 1 < argc) {
                url = argv[++i];
            } else if (arg == "--interval-sec" && i + 1 < argc) {
                interval_sec = parseNumeric<int>(argv[++i], "--interval-sec");
            } else if (arg == "--duration-sec" && i + 1 < argc) {
                duration_sec = parseNumeric<int>(argv[++i], "--duration-sec");
            } else if (arg == "--cache-dir" && i + 1 < argc) {
                cache_dir = argv[++i];
            } else if (arg == "--direct") {
                prefer_cache = false;
            } else if (arg == "--timeout-ms" && i + 1 < argc) {
                timeout_ms = parseNumeric<int>(argv[++i], "--timeout-ms");
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

    if (url.empty()) {
        err << "[ERROR] --url is required\n";
        printUsage(err);
        return 1;
    }
    if (interval_sec < 1) {
        err << "[ERROR] --interval-sec must be >= 1\n";
        return 1;
    }

    adapters::CctvMetadata meta;
    meta.camera_id = "live_snapshot";
    meta.stream_or_snapshot_url = url;

    adapters::CctvHttpSnapshot snap(meta);
    snap.setCacheDir(cache_dir);
    snap.setPreferCache(prefer_cache);
    snap.setTimeout(std::chrono::milliseconds(timeout_ms));

    quality::QualityThresholds thresholds;
    thresholds.min_blur_score = 20.0;
    thresholds.min_brightness = 10.0;
    thresholds.max_brightness = 250.0;
    thresholds.min_contrast = 5.0;
    const auto analyzer = std::make_shared<quality::LaplacianQualityAnalyzer>(thresholds);

    ridge::StegerConfig steger_cfg;
    steger_cfg.sigma = 1.5f;
    steger_cfg.low_threshold = 0.5f;
    steger_cfg.high_threshold = 1.5f;
    const auto extractor = std::make_shared<ridge::StegerRidgeExtractor>(steger_cfg);
    pipeline::SyncPerceptionPipeline pipeline(analyzer, extractor);

    out << "==================================================\n"
        << "live_snapshot_demo (Phase 3)\n"
        << "  url:          " << url << '\n'
        << "  resolved:     " << adapters::CctvHttpSnapshot::resolveCameraUrl(url) << '\n'
        << "  interval-sec: " << interval_sec << '\n'
        << "  duration-sec: " << duration_sec << '\n'
        << "  cache-dir:    " << cache_dir << '\n'
        << "  prefer-cache: " << (prefer_cache ? "yes" : "no (--direct)") << '\n'
        << "==================================================\n";

    const auto start = std::chrono::steady_clock::now();
    int tick = 0;
    while (true) {
        if (duration_sec > 0) {
            const auto elapsed =
                std::chrono::duration_cast<std::chrono::seconds>(std::chrono::steady_clock::now() - start).count();
            if (elapsed >= duration_sec) {
                break;
            }
        }

        ++tick;
        Frame frame;
        adapters::CctvHttpSnapshot::FetchStats stats;
        const auto t0 = std::chrono::steady_clock::now();
        const bool ok = snap.fetch(url, frame, &stats);
        const auto wall_ms =
            std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - t0).count();

        if (!ok) {
            out << "[" << tick << "] FAIL  err=" << stats.error << "  latency_ms=" << wall_ms << '\n';
        } else {
            const auto result = pipeline.process(frame);
            const char* src = "live";
            if (stats.from_mock) {
                src = "mock";
            } else if (stats.from_cache) {
                src = "cache";
            }
            out << "[" << tick << "] OK"
                << "  src=" << src << "  " << frame.width() << "x" << frame.height()
                << "  usable=" << (result.is_usable ? "Y" : "N") << "  blur=" << result.quality.blur_score
                << "  segs=" << result.ridge_graph.segments.size() << "  latency_ms=" << stats.latency.count() << '\n';
        }

        if (duration_sec > 0) {
            const auto elapsed =
                std::chrono::duration_cast<std::chrono::seconds>(std::chrono::steady_clock::now() - start).count();
            if (elapsed >= duration_sec) {
                break;
            }
        }
        std::this_thread::sleep_for(std::chrono::seconds(interval_sec));
    }

    out << "Done. ticks=" << tick << '\n';
    return 0;
}

} // namespace CurvEngine::tools
