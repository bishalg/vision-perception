#include "curv/tools/palm_inspect_lib.hpp"

#include <filesystem>
#include <fstream>
#include <iostream>
#include <opencv2/highgui.hpp>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include <string>

namespace CurvEngine::tools {
namespace {

struct DebugState {
    cv::Mat source;
    std::string image_path;
    geometry::HandLandmarks landmarks{};
    bool have_landmarks{false};
    bool debug_ridges{false};
    bool left_hint{false};
    PalmInspectResult last{};
    int gap_track{35};
    int coli_track{70}; // 0.70 * 100
};

void showScaled(const std::string& win, const cv::Mat& img, int max_side = 900) {
    if (img.empty()) return;
    cv::Mat view = img;
    const int side = std::max(img.cols, img.rows);
    if (side > max_side) {
        const double s = static_cast<double>(max_side) / static_cast<double>(side);
        cv::resize(img, view, cv::Size(), s, s, cv::INTER_AREA);
    }
    cv::imshow(win, view);
}

bool rerun(DebugState& st, std::ostream& out) {
    if (st.source.empty()) {
        out << "[WARN] No image loaded\n";
        return false;
    }
    PalmInspectOptions opts;
    opts.debug_all_ridges = st.debug_ridges;
    opts.is_left_hand_hint = st.left_hint;
    opts.link_options.max_gap_distance = static_cast<float>(st.gap_track);
    opts.link_options.min_collinearity = static_cast<float>(st.coli_track) / 100.0f;

    const geometry::HandLandmarks* ptr = st.have_landmarks ? &st.landmarks : nullptr;
    st.last = runPalmInspect(st.source, ptr, opts, &out);
    if (!st.last.ok) {
        out << "[ERROR] " << st.last.error << '\n';
        return false;
    }
    // Keep detected landmarks for subsequent saves.
    if (!st.have_landmarks) {
        st.landmarks = st.last.landmarks;
        st.have_landmarks = true;
    }
    showScaled("full_overlay", st.last.full_overlay);
    showScaled("canonical_overlay", st.last.canonical_overlay, 640);
    out << "Lines:";
    for (const auto& name : {"heart_line", "head_line", "life_line", "fate_line"}) {
        auto it = st.last.major_lines.find(name);
        if (it != st.last.major_lines.end() && it->second.is_valid) {
            out << " " << name << "(" << static_cast<int>(it->second.total_length) << "px)";
        }
    }
    out << '\n';
    return true;
}

void printHelp(std::ostream& out) {
    out << "Keys:\n"
        << "  o  open image path from stdin prompt\n"
        << "  l  load landmarks JSON path from stdin\n"
        << "  r  re-run pipeline\n"
        << "  d  toggle debug-all-ridges\n"
        << "  h  toggle left-hand hint\n"
        << "  s  save overlays + evidence JSON next to image\n"
        << "  q/ESC quit\n"
        << "Trackbars: max_gap_px, min_coli_x100\n";
}

} // namespace

int runPalmOverlayDebug(int argc, char* argv[], std::ostream& out, std::ostream& err) {
    DebugState st;
    std::string image_path;
    std::string landmarks_path;

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--image" && i + 1 < argc) {
            image_path = argv[++i];
        } else if (arg == "--landmarks" && i + 1 < argc) {
            landmarks_path = argv[++i];
        } else if (arg == "--left-hand") {
            st.left_hint = true;
        } else if (arg == "--debug-all-ridges") {
            st.debug_ridges = true;
        } else if (arg == "--help") {
            out << "Usage: " << argv[0]
                << " [--image path] [--landmarks path.json] [--left-hand] [--debug-all-ridges]\n";
            printHelp(out);
            return 0;
        }
    }

    cv::namedWindow("full_overlay", cv::WINDOW_NORMAL);
    cv::namedWindow("canonical_overlay", cv::WINDOW_NORMAL);
    cv::createTrackbar("max_gap_px", "full_overlay", &st.gap_track, 80);
    cv::createTrackbar("min_coli_x100", "full_overlay", &st.coli_track, 100);

    printHelp(out);

    auto loadImage = [&](const std::string& path) -> bool {
        cv::Mat img = cv::imread(path, cv::IMREAD_COLOR);
        if (img.empty()) {
            err << "[ERROR] Cannot load " << path << '\n';
            return false;
        }
        st.source = img;
        st.image_path = path;
        st.have_landmarks = false;
        out << "Loaded " << path << " (" << img.cols << "x" << img.rows << ")\n";
        return true;
    };

    if (!image_path.empty()) {
        if (!loadImage(image_path)) return 1;
    }
    if (!landmarks_path.empty()) {
        std::string lerr;
        if (!loadLandmarksJson(landmarks_path, st.landmarks, &lerr)) {
            err << "[ERROR] " << lerr << '\n';
            return 1;
        }
        st.have_landmarks = true;
        out << "Loaded landmarks " << landmarks_path << '\n';
    }

    if (!st.source.empty()) {
        rerun(st, out);
    } else {
        out << "Press 'o' then type an image path in the terminal.\n";
    }

    while (true) {
        const int key = cv::waitKey(30) & 0xFF;
        if (key == 27 || key == 'q') {
            break;
        }
        if (key == 'o') {
            out << "Image path: " << std::flush;
            std::string path;
            std::getline(std::cin, path);
            if (!path.empty() && loadImage(path)) {
                rerun(st, out);
            }
        } else if (key == 'l') {
            out << "Landmarks JSON path: " << std::flush;
            std::string path;
            std::getline(std::cin, path);
            std::string lerr;
            if (!path.empty() && loadLandmarksJson(path, st.landmarks, &lerr)) {
                st.have_landmarks = true;
                out << "Loaded landmarks\n";
                rerun(st, out);
            } else if (!lerr.empty()) {
                err << "[ERROR] " << lerr << '\n';
            }
        } else if (key == 'r') {
            rerun(st, out);
        } else if (key == 'd') {
            st.debug_ridges = !st.debug_ridges;
            out << "debug_all_ridges=" << (st.debug_ridges ? "on" : "off") << '\n';
            rerun(st, out);
        } else if (key == 'h') {
            st.left_hint = !st.left_hint;
            out << "left_hand_hint=" << (st.left_hint ? "on" : "off") << '\n';
            rerun(st, out);
        } else if (key == 's') {
            if (!st.last.ok || st.image_path.empty()) {
                out << "[WARN] Nothing to save\n";
                continue;
            }
            namespace fs = std::filesystem;
            const fs::path base = fs::path(st.image_path);
            const fs::path dir = base.parent_path();
            const std::string stem = base.stem().string();
            const auto canon = (dir / (stem + "_canonical_overlay.png")).string();
            const auto full = (dir / (stem + "_full_overlay.png")).string();
            const auto json = (dir / (stem + "_evidence.json")).string();
            const auto lms = (dir / (stem + "_landmarks.json")).string();
            cv::imwrite(canon, st.last.canonical_overlay);
            cv::imwrite(full, st.last.full_overlay);
            std::ofstream(json) << st.last.evidence.dump(2);
            saveLandmarksJson(lms, st.last.landmarks);
            out << "Saved:\n  " << canon << "\n  " << full << "\n  " << json << "\n  " << lms << '\n';
        }
    }

    cv::destroyAllWindows();
    return 0;
}

} // namespace CurvEngine::tools
