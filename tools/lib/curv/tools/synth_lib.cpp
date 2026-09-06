#include "curv/tools/synth_lib.hpp"

#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <nlohmann/json.hpp>
#include <numbers>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>

namespace CurvEngine::tools {

namespace {

constexpr int kFixedShift = 8; // cv::line fixed-point sub-pixel shift (1/256 px)

} // namespace

std::vector<cv::Point2d> sampleGroundTruth(const std::string& type, const SyntheticParams& p) {
    std::vector<cv::Point2d> pts;

    if (type == "line") {
        // Slightly tilted line with a margin from the borders.
        const double margin = 0.2 * static_cast<double>(std::min(p.width, p.height));
        const cv::Point2d a(margin, 0.3 * p.height);
        const cv::Point2d b(p.width - margin, 0.7 * p.height);
        const double len = std::hypot(b.x - a.x, b.y - a.y);
        const int steps = std::max(2, static_cast<int>(len / p.sample_step_px));
        pts.reserve(static_cast<size_t>(steps) + 1);
        for (int i = 0; i <= steps; ++i) {
            const double t = static_cast<double>(i) / steps;
            pts.emplace_back(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        }
    } else if (type == "parabola") {
        // y = y0 + a * (x - x0)^2, sagging downward across the frame.
        const double x0 = 0.5 * p.width;
        const double y0 = 0.25 * p.height;
        const double half_width = 0.35 * p.width;
        const double sag = 0.35 * p.height;
        const double a = sag / (half_width * half_width);
        const double x_start = x0 - half_width;
        const double x_end = x0 + half_width;
        const int steps = std::max(2, static_cast<int>((x_end - x_start) / p.sample_step_px));
        pts.reserve(static_cast<size_t>(steps) + 1);
        for (int i = 0; i <= steps; ++i) {
            const double x = x_start + (x_end - x_start) * (static_cast<double>(i) / steps);
            const double y = y0 + a * (x - x0) * (x - x0);
            pts.emplace_back(x, y);
        }
    } else if (type == "spiral") {
        // Archimedean spiral r = b * theta centered in the frame.
        const double cx = 0.5 * p.width;
        const double cy = 0.5 * p.height;
        const double theta_max = 6.0 * std::numbers::pi;
        const double b = (0.38 * std::min(p.width, p.height)) / theta_max;
        const double arc = 0.5 * b * theta_max * theta_max; // arc length of r = b*theta
        const int steps = std::max(2, static_cast<int>(arc / p.sample_step_px));
        pts.reserve(static_cast<size_t>(steps) + 1);
        for (int i = 0; i <= steps; ++i) {
            const double theta = theta_max * (static_cast<double>(i) / steps);
            const double r = b * theta;
            pts.emplace_back(cx + r * std::cos(theta), cy + r * std::sin(theta));
        }
    }

    return pts;
}

void drawCurve(cv::Mat& img, const std::vector<cv::Point2d>& pts, const SyntheticParams& p) {
    const cv::Scalar color(p.curve_brightness, p.curve_brightness, p.curve_brightness);
    const int thickness = std::max(1, static_cast<int>(std::lround(p.thickness_px)));
    const int scale = static_cast<int>(1u << static_cast<unsigned>(kFixedShift));
    for (size_t i = 1; i < pts.size(); ++i) {
        const cv::Point a(static_cast<int>(std::lround(pts[i - 1].x * scale)),
                          static_cast<int>(std::lround(pts[i - 1].y * scale)));
        const cv::Point b(static_cast<int>(std::lround(pts[i].x * scale)),
                          static_cast<int>(std::lround(pts[i].y * scale)));
        cv::line(img, a, b, color, thickness, cv::LINE_AA, kFixedShift);
    }
}

void addGaussianNoise(cv::Mat& img, double sigma, uint64_t seed) {
    if (sigma <= 0.0) {
        return;
    }
    cv::theRNG().state = seed;
    cv::Mat noise(img.size(), CV_32F);
    cv::randn(noise, 0.0, sigma);
    cv::Mat acc;
    img.convertTo(acc, CV_32F);
    acc += noise;
    acc.convertTo(img, img.type());
}

bool writeOutputs(const std::string& type, const std::filesystem::path& out_dir, const cv::Mat& img,
                  const std::vector<cv::Point2d>& pts, const SyntheticParams& p, std::ostream* err) {
    const std::string image_name = type + "_image.png";
    const std::string truth_name = type + "_truth.json";
    std::ostream& sink = err != nullptr ? *err : std::cerr;

    if (!cv::imwrite((out_dir / image_name).string(), img)) {
        sink << "[ERROR] Failed to write " << (out_dir / image_name) << '\n';
        return false;
    }

    nlohmann::json gt = nlohmann::json::array();
    for (const auto& pt : pts) {
        gt.push_back({pt.x, pt.y});
    }

    nlohmann::json truth;
    truth["image"] = image_name;
    truth["curve_type"] = type;
    truth["width"] = p.width;
    truth["height"] = p.height;
    truth["curve_brightness"] = p.curve_brightness;
    truth["background_brightness"] = p.background_brightness;
    truth["thickness_px"] = p.thickness_px;
    truth["noise_sigma"] = p.noise_sigma;
    truth["seed"] = p.seed;
    truth["sample_step_px"] = p.sample_step_px;
    truth["ground_truth_points"] = gt;

    std::ofstream out(out_dir / truth_name);
    if (!out) {
        sink << "[ERROR] Failed to write " << (out_dir / truth_name) << '\n';
        return false;
    }
    out << truth.dump(2);
    return true;
}

int runSynthGenerator(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    std::string type = "all";
    std::string out_dir_str = "synthetic";
    SyntheticParams p;

    // Numeric CLI values that fail to parse abort the run with a clear error
    // naming the offending flag (silently defaulting would hide user mistakes).
    const auto parse_double = [](const char* raw, const std::string& flag) -> double {
        try {
            return std::stod(raw);
        } catch (const std::exception&) {
            throw std::invalid_argument(flag);
        }
    };
    const auto parse_int = [](const char* raw, const std::string& flag) -> int {
        try {
            return std::stoi(raw);
        } catch (const std::exception&) {
            throw std::invalid_argument(flag);
        }
    };
    const auto parse_ull = [](const char* raw, const std::string& flag) -> uint64_t {
        try {
            return std::stoull(raw);
        } catch (const std::exception&) {
            throw std::invalid_argument(flag);
        }
    };

    try {
        for (int i = 1; i < argc; ++i) {
            const std::string arg = argv[i];
            if (arg == "--type" && i + 1 < argc) {
                type = argv[++i];
            } else if (arg == "--width" && i + 1 < argc) {
                p.width = parse_int(argv[++i], "--width");
            } else if (arg == "--height" && i + 1 < argc) {
                p.height = parse_int(argv[++i], "--height");
            } else if (arg == "--thickness" && i + 1 < argc) {
                p.thickness_px = parse_double(argv[++i], "--thickness");
            } else if (arg == "--noise" && i + 1 < argc) {
                p.noise_sigma = parse_double(argv[++i], "--noise");
            } else if (arg == "--seed" && i + 1 < argc) {
                p.seed = parse_ull(argv[++i], "--seed");
            } else if (arg == "--sample-step" && i + 1 < argc) {
                p.sample_step_px = parse_double(argv[++i], "--sample-step");
            } else if (arg == "--out-dir" && i + 1 < argc) {
                out_dir_str = argv[++i];
            } else {
                out << "Usage: " << argv[0]
                    << " [--type line|parabola|spiral|all] [--width W] [--height H]"
                       " [--thickness px] [--noise sigma] [--seed n] [--sample-step px] [--out-dir dir]\n";
                return arg == "--help" || arg == "-h" ? 0 : 1;
            }
        }
    } catch (const std::invalid_argument& e) {
        err << "[ERROR] Invalid numeric value for " << e.what() << '\n';
        return 1;
    }

    if (p.width <= 0 || p.height <= 0) {
        err << "[ERROR] width and height must be positive\n";
        return 1;
    }

    std::filesystem::create_directories(out_dir_str);

    std::vector<std::string> types;
    if (type == "all") {
        types = {"line", "parabola", "spiral"};
    } else {
        types = {type};
    }

    for (const auto& t : types) {
        const auto pts = sampleGroundTruth(t, p);
        if (pts.empty()) {
            err << "[ERROR] Unknown curve type: " << t << '\n';
            return 1;
        }

        cv::Mat img(p.height, p.width, CV_8UC1, cv::Scalar(static_cast<int>(p.background_brightness)));
        drawCurve(img, pts, p);
        addGaussianNoise(img, p.noise_sigma, p.seed);

        if (!writeOutputs(t, out_dir_str, img, pts, p, &err)) {
            return 1;
        }
        out << "[PASS] Generated " << t << ": " << (std::filesystem::path(out_dir_str) / (t + "_image.png")) << " ("
            << pts.size() << " ground-truth samples)\n";
    }

    return 0;
}

} // namespace CurvEngine::tools
