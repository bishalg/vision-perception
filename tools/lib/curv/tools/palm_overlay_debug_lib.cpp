#include "curv/tools/palm_inspect_lib.hpp"

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <opencv2/highgui.hpp>
#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>
#include <sstream>
#include <string>
#include <vector>

namespace CurvEngine::tools {
namespace {

namespace fs = std::filesystem;

struct Button {
    std::string id;
    std::string label;
    cv::Rect rect;
    cv::Scalar bg;
    cv::Scalar text_color;
};

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

    std::vector<std::string> gallery;
    size_t gallery_idx{0};
    std::string status_msg{"Ready"};
    bool needs_rerun{false};
    std::vector<Button> buttons;
};

bool isSourcePhoto(const fs::path& p) {
    const std::string ext = p.extension().string();
    if (ext != ".jpg" && ext != ".jpeg" && ext != ".png" && ext != ".JPG" && ext != ".PNG") {
        return false;
    }
    const std::string stem = p.stem().string();
    if (stem.find("_overlay") != std::string::npos || stem.find("_canonical") != std::string::npos ||
        stem.find("_USER_CORRECTED") != std::string::npos || stem.find("_landmarks") != std::string::npos) {
        return false;
    }
    return true;
}

void discoverGallery(DebugState& st, const std::string& current_path) {
    st.gallery.clear();
    fs::path target_dir = "data/samples/mobile_captures";
    if (!current_path.empty()) {
        fs::path p(current_path);
        if (p.has_parent_path() && fs::exists(p.parent_path())) {
            target_dir = p.parent_path();
        }
    }
    if (fs::exists(target_dir) && fs::is_directory(target_dir)) {
        for (const auto& entry : fs::directory_iterator(target_dir)) {
            if (entry.is_regular_file() && isSourcePhoto(entry.path())) {
                st.gallery.push_back(entry.path().string());
            }
        }
    }
    std::sort(st.gallery.begin(), st.gallery.end());
    if (st.gallery.empty() && !current_path.empty()) {
        st.gallery.push_back(current_path);
    }
    st.gallery_idx = 0;
    for (size_t i = 0; i < st.gallery.size(); ++i) {
        if (!current_path.empty() && fs::equivalent(fs::path(st.gallery[i]), fs::path(current_path))) {
            st.gallery_idx = i;
            break;
        }
    }
}

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

bool rerun(DebugState& st, std::ostream& out);

void renderCanonicalHUD(DebugState& st) {
    const int canvas_w = 640;
    const int top_hud_h = 95;
    const int palm_h = 640;
    const int bottom_bar_h = 35;
    const int canvas_h = top_hud_h + palm_h + bottom_bar_h;

    cv::Mat canvas(canvas_h, canvas_w, CV_8UC3, cv::Scalar(22, 22, 26));

    // Draw palm overlay in center
    if (!st.last.canonical_overlay.empty()) {
        cv::Mat scaled_palm;
        cv::resize(st.last.canonical_overlay, scaled_palm, cv::Size(canvas_w, palm_h), 0, 0, cv::INTER_AREA);
        scaled_palm.copyTo(canvas(cv::Rect(0, top_hud_h, canvas_w, palm_h)));
    } else {
        cv::putText(canvas, "No overlay available", cv::Point(180, top_hud_h + 300), cv::FONT_HERSHEY_SIMPLEX, 0.8,
                    cv::Scalar(150, 150, 150), 1, cv::LINE_AA);
    }

    // Top HUD background banner
    cv::rectangle(canvas, cv::Rect(0, 0, canvas_w, top_hud_h), cv::Scalar(32, 32, 38), -1);
    cv::line(canvas, cv::Point(0, top_hud_h - 1), cv::Point(canvas_w, top_hud_h - 1), cv::Scalar(60, 60, 70), 1);

    // Setup interactive buttons
    st.buttons.clear();
    st.buttons.push_back({"hand", st.left_hint ? "Hand: LEFT" : "Hand: RIGHT", cv::Rect(10, 10, 115, 32),
                          st.left_hint ? cv::Scalar(170, 95, 30) : cv::Scalar(140, 50, 130),
                          cv::Scalar(255, 255, 255)});
    st.buttons.push_back({"mode", st.debug_ridges ? "Mode: RIDGES" : "Mode: LINES", cv::Rect(135, 10, 125, 32),
                          st.debug_ridges ? cv::Scalar(30, 110, 200) : cv::Scalar(40, 135, 45),
                          cv::Scalar(255, 255, 255)});
    st.buttons.push_back(
        {"prev", "< Prev", cv::Rect(270, 10, 75, 32), cv::Scalar(55, 55, 62), cv::Scalar(240, 240, 240)});
    st.buttons.push_back(
        {"next", "Next >", cv::Rect(355, 10, 75, 32), cv::Scalar(55, 55, 62), cv::Scalar(240, 240, 240)});
    st.buttons.push_back(
        {"save", "Save", cv::Rect(440, 10, 85, 32), cv::Scalar(120, 110, 30), cv::Scalar(255, 255, 255)});
    st.buttons.push_back(
        {"rerun", "Rerun", cv::Rect(535, 10, 95, 32), cv::Scalar(75, 75, 82), cv::Scalar(255, 255, 255)});

    for (const auto& btn : st.buttons) {
        cv::rectangle(canvas, btn.rect, btn.bg, -1, cv::LINE_AA);
        cv::rectangle(canvas, btn.rect, cv::Scalar(90, 90, 100), 1, cv::LINE_AA);
        int baseline = 0;
        cv::Size tsize = cv::getTextSize(btn.label, cv::FONT_HERSHEY_SIMPLEX, 0.45, 1, &baseline);
        cv::Point text_pos(btn.rect.x + (btn.rect.width - tsize.width) / 2,
                           btn.rect.y + (btn.rect.height + tsize.height) / 2);
        cv::putText(canvas, btn.label, text_pos, cv::FONT_HERSHEY_SIMPLEX, 0.45, btn.text_color, 1, cv::LINE_AA);
    }

    // Row 2: Live measurements & parameters
    std::ostringstream readout;
    readout << "Gap: " << st.gap_track << "px | Coli: " << std::fixed << std::setprecision(2)
            << (static_cast<float>(st.coli_track) / 100.0f);
    for (const auto& name : {"heart_line", "head_line", "life_line", "fate_line"}) {
        auto it = st.last.major_lines.find(name);
        if (it != st.last.major_lines.end() && it->second.is_valid) {
            std::string short_name = name;
            if (short_name == "heart_line")
                short_name = "Heart";
            else if (short_name == "head_line")
                short_name = "Head";
            else if (short_name == "life_line")
                short_name = "Life";
            else if (short_name == "fate_line")
                short_name = "Fate";
            readout << " | " << short_name << ":" << static_cast<int>(it->second.total_length) << "px";
        }
    }
    cv::putText(canvas, readout.str(), cv::Point(14, 72), cv::FONT_HERSHEY_SIMPLEX, 0.42, cv::Scalar(190, 210, 220), 1,
                cv::LINE_AA);

    // Bottom status footer
    const int bottom_y = top_hud_h + palm_h;
    cv::rectangle(canvas, cv::Rect(0, bottom_y, canvas_w, bottom_bar_h), cv::Scalar(30, 30, 35), -1);
    cv::line(canvas, cv::Point(0, bottom_y), cv::Point(canvas_w, bottom_y), cv::Scalar(60, 60, 70), 1);

    std::string filename = fs::path(st.image_path).filename().string();
    std::string index_str = "";
    if (!st.gallery.empty()) {
        index_str = " (" + std::to_string(st.gallery_idx + 1) + "/" + std::to_string(st.gallery.size()) + ")";
    }
    std::string footer_left = st.status_msg;
    std::string footer_right = filename + index_str;

    cv::putText(canvas, footer_left, cv::Point(12, bottom_y + 22), cv::FONT_HERSHEY_SIMPLEX, 0.42,
                cv::Scalar(110, 220, 150), 1, cv::LINE_AA);

    int bl = 0;
    cv::Size rsize = cv::getTextSize(footer_right, cv::FONT_HERSHEY_SIMPLEX, 0.42, 1, &bl);
    cv::putText(canvas, footer_right, cv::Point(canvas_w - rsize.width - 12, bottom_y + 22), cv::FONT_HERSHEY_SIMPLEX,
                0.42, cv::Scalar(160, 160, 170), 1, cv::LINE_AA);

    cv::imshow("canonical_overlay", canvas);
}

void doSave(DebugState& st, std::ostream& out) {
    if (!st.last.ok || st.image_path.empty()) {
        st.status_msg = "[WARN] Nothing to save";
        renderCanonicalHUD(st);
        return;
    }
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
    st.status_msg = "Saved overlays + evidence JSON (" + stem + ")";
    out << "[PASS] Saved:\n  " << canon << "\n  " << full << "\n  " << json << "\n  " << lms << '\n';
    renderCanonicalHUD(st);
}

bool loadImageFromDisk(DebugState& st, const std::string& path, std::ostream& out, std::ostream& err) {
    cv::Mat img = cv::imread(path, cv::IMREAD_COLOR);
    if (img.empty()) {
        err << "[ERROR] Cannot load " << path << '\n';
        st.status_msg = "Failed to load: " + fs::path(path).filename().string();
        renderCanonicalHUD(st);
        return false;
    }
    st.source = img;
    st.image_path = path;
    st.have_landmarks = false;
    // Automatically check for hand side hint from filename (e.g. 01_left)
    const std::string stem = fs::path(path).stem().string();
    if (stem.find("left") != std::string::npos) {
        st.left_hint = true;
    } else if (stem.find("right") != std::string::npos) {
        st.left_hint = false;
    }
    // Check if matching landmarks JSON exists next to image
    const fs::path lms_path = fs::path(path).parent_path() / (stem + "_landmarks.json");
    if (fs::exists(lms_path)) {
        std::string lerr;
        if (loadLandmarksJson(lms_path.string(), st.landmarks, &lerr)) {
            st.have_landmarks = true;
        }
    }
    st.status_msg = "Loaded " + fs::path(path).filename().string();
    out << "Loaded " << path << " (" << img.cols << "x" << img.rows << ")\n";
    return true;
}

bool rerun(DebugState& st, std::ostream& out) {
    if (st.source.empty()) {
        out << "[WARN] No image loaded\n";
        st.status_msg = "[WARN] No image loaded";
        renderCanonicalHUD(st);
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
        st.status_msg = "[ERROR] " + st.last.error;
        renderCanonicalHUD(st);
        return false;
    }
    if (!st.have_landmarks) {
        st.landmarks = st.last.landmarks;
        st.have_landmarks = true;
    }
    showScaled("full_overlay", st.last.full_overlay);
    renderCanonicalHUD(st);

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

void onMouseCanonical(int event, int x, int y, int /*flags*/, void* userdata) {
    if (event != cv::EVENT_LBUTTONDOWN) return;
    auto* st = static_cast<DebugState*>(userdata);
    if (!st) return;

    for (const auto& btn : st->buttons) {
        if (btn.rect.contains(cv::Point(x, y))) {
            if (btn.id == "hand") {
                st->left_hint = !st->left_hint;
                st->status_msg = st->left_hint ? "Hand set to LEFT" : "Hand set to RIGHT";
                rerun(*st, std::cout);
            } else if (btn.id == "mode") {
                st->debug_ridges = !st->debug_ridges;
                st->status_msg = st->debug_ridges ? "Mode: ALL RIDGES" : "Mode: MAJOR LINES";
                rerun(*st, std::cout);
            } else if (btn.id == "prev") {
                if (!st->gallery.empty()) {
                    st->gallery_idx = (st->gallery_idx == 0) ? st->gallery.size() - 1 : st->gallery_idx - 1;
                    loadImageFromDisk(*st, st->gallery[st->gallery_idx], std::cout, std::cerr);
                    rerun(*st, std::cout);
                }
            } else if (btn.id == "next") {
                if (!st->gallery.empty()) {
                    st->gallery_idx = (st->gallery_idx + 1) % st->gallery.size();
                    loadImageFromDisk(*st, st->gallery[st->gallery_idx], std::cout, std::cerr);
                    rerun(*st, std::cout);
                }
            } else if (btn.id == "save") {
                doSave(*st, std::cout);
            } else if (btn.id == "rerun") {
                st->status_msg = "Pipeline re-run";
                rerun(*st, std::cout);
            }
            break;
        }
    }
}

void onTrackbarChange(int /*val*/, void* userdata) {
    auto* st = static_cast<DebugState*>(userdata);
    if (st) {
        st->needs_rerun = true;
    }
}

void printHelp(std::ostream& out) {
    out << "=======================================================\n"
        << "CurvEngine Palm Overlay Debugger (Interactive UI)\n"
        << "=======================================================\n"
        << "UI Controls:\n"
        << "  Click buttons at top of 'canonical_overlay' window:\n"
        << "    [Hand: LEFT/RIGHT]  Toggle hand chirality\n"
        << "    [Mode: LINES/RIDGES] Toggle major lines vs raw ridges\n"
        << "    [< Prev] [Next >]   Browse sample captures\n"
        << "    [Save]              Save overlays + JSON to disk\n"
        << "    [Rerun]             Re-execute pipeline\n"
        << "  Trackbars on 'full_overlay': max_gap_px, min_coli_x100\n"
        << "Keyboard Shortcuts:\n"
        << "  h        Toggle left/right hand\n"
        << "  d / m    Toggle major lines vs all raw ridges\n"
        << "  p / n    Previous / Next image in gallery\n"
        << "  r        Re-run pipeline\n"
        << "  s        Save overlays and JSON evidence\n"
        << "  q / ESC  Quit\n"
        << "=======================================================\n";
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

    discoverGallery(st, image_path);

    cv::namedWindow("canonical_overlay", cv::WINDOW_NORMAL);
    cv::namedWindow("full_overlay", cv::WINDOW_NORMAL);
    cv::resizeWindow("canonical_overlay", 640, 770);

    cv::createTrackbar("max_gap_px", "full_overlay", &st.gap_track, 80, onTrackbarChange, &st);
    cv::createTrackbar("min_coli_x100", "full_overlay", &st.coli_track, 100, onTrackbarChange, &st);
    cv::setMouseCallback("canonical_overlay", onMouseCanonical, &st);

    printHelp(out);

    if (!image_path.empty()) {
        if (!loadImageFromDisk(st, image_path, out, err)) return 1;
    } else if (!st.gallery.empty()) {
        loadImageFromDisk(st, st.gallery[0], out, err);
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
    }

    while (true) {
        if (st.needs_rerun) {
            st.needs_rerun = false;
            rerun(st, out);
        }

        const int key = cv::waitKey(30) & 0xFF;
        if (key == 27 || key == 'q') {
            break;
        }
        if (key == 'h') {
            st.left_hint = !st.left_hint;
            st.status_msg = st.left_hint ? "Hand set to LEFT" : "Hand set to RIGHT";
            rerun(st, out);
        } else if (key == 'd' || key == 'm') {
            st.debug_ridges = !st.debug_ridges;
            st.status_msg = st.debug_ridges ? "Mode: ALL RIDGES" : "Mode: MAJOR LINES";
            rerun(st, out);
        } else if (key == 'n' || key == 'o' || key == 32) { // 32 = Space
            if (!st.gallery.empty()) {
                st.gallery_idx = (st.gallery_idx + 1) % st.gallery.size();
                loadImageFromDisk(st, st.gallery[st.gallery_idx], out, err);
                rerun(st, out);
            }
        } else if (key == 'p') {
            if (!st.gallery.empty()) {
                st.gallery_idx = (st.gallery_idx == 0) ? st.gallery.size() - 1 : st.gallery_idx - 1;
                loadImageFromDisk(st, st.gallery[st.gallery_idx], out, err);
                rerun(st, out);
            }
        } else if (key == 'r') {
            st.status_msg = "Pipeline re-run";
            rerun(st, out);
        } else if (key == 's') {
            doSave(st, out);
        }
    }

    cv::destroyAllWindows();
    return 0;
}

} // namespace CurvEngine::tools
