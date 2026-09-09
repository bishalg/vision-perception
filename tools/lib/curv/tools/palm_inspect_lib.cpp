#include "curv/tools/palm_inspect_lib.hpp"

#include <fstream>
#include <memory>

#include <opencv2/imgcodecs.hpp>
#include <opencv2/imgproc.hpp>

#include "curv/Frame.hpp"
#include "curv/adapters/HandBoundaryDetector.hpp"
#include "curv/adapters/HandLandmarkDetector.hpp"
#include "curv/adapters/OverlayRenderer.hpp"
#include "curv/domain/PalmParameters.hpp"
#include "curv/domains/PalmDomainPack.hpp"
#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

namespace CurvEngine::tools {
namespace {

void logLine(std::ostream* log, const std::string& msg) {
    if (log) {
        (*log) << msg << '\n';
    }
}

cv::Point2f applyAffine(const cv::Mat& M, float x, float y) {
    cv::Mat M64;
    if (M.type() != CV_64F) {
        M.convertTo(M64, CV_64F);
    } else {
        M64 = M;
    }
    const double* m = M64.ptr<double>();
    return cv::Point2f(static_cast<float>(m[0] * x + m[1] * y + m[2]),
                       static_cast<float>(m[3] * x + m[4] * y + m[5]));
}

void enrichImageSpacePoints(nlohmann::json& domain_payload, const cv::Mat& M_inv, int img_w, int img_h) {
    if (!domain_payload.contains("major_lines") || !domain_payload["major_lines"].is_object()) {
        return;
    }
    const float inv_w = 1.0f / static_cast<float>(std::max(1, img_w));
    const float inv_h = 1.0f / static_cast<float>(std::max(1, img_h));

    for (auto& [name, line] : domain_payload["major_lines"].items()) {
        (void)name;
        if (!line.value("present", false) || !line.contains("points_canonical_px")) {
            line["points_image_norm"] = nlohmann::json::array();
            continue;
        }
        nlohmann::json image_pts = nlohmann::json::array();
        for (const auto& pt : line["points_canonical_px"]) {
            if (!pt.is_array() || pt.size() < 2) continue;
            const auto mapped = applyAffine(M_inv, pt[0].get<float>(), pt[1].get<float>());
            image_pts.push_back({mapped.x * inv_w, mapped.y * inv_h});
        }
        line["points_image_norm"] = std::move(image_pts);
    }
}

void drawLandmarkDots(cv::Mat& img, const geometry::HandLandmarks& lms, int img_w, int img_h) {
    const float sx = lms.is_normalized ? static_cast<float>(img_w) : 1.0f;
    const float sy = lms.is_normalized ? static_cast<float>(img_h) : 1.0f;
    for (const auto& p : lms.points) {
        cv::circle(img, cv::Point(static_cast<int>(std::round(p.x * sx)), static_cast<int>(std::round(p.y * sy))), 3,
                   cv::Scalar(255, 255, 255), -1, cv::LINE_AA);
    }
}

adapters::OverlayConfig::ContinuousCreaseItem makeCreaseItem(const std::string& name,
                                                             const domain::ChainedCreaseLine& line, const cv::Mat* M_inv,
                                                             int thickness) {
    adapters::OverlayConfig::ContinuousCreaseItem item;
    item.thickness = thickness;
    item.points.reserve(line.points.size());
    for (const auto& pt : line.points) {
        if (M_inv != nullptr && !M_inv->empty()) {
            const auto mapped = applyAffine(*M_inv, pt.x, pt.y);
            item.points.emplace_back(static_cast<int>(std::round(mapped.x)), static_cast<int>(std::round(mapped.y)));
        } else {
            item.points.emplace_back(static_cast<int>(std::round(pt.x)), static_cast<int>(std::round(pt.y)));
        }
    }
    if (name == "heart_line") {
        item.color = cv::Scalar(80, 50, 245);
        item.label = "Heart";
    } else if (name == "head_line") {
        item.color = cv::Scalar(245, 160, 20);
        item.label = "Head";
    } else if (name == "life_line") {
        item.color = cv::Scalar(50, 205, 50);
        item.label = "Life";
    } else if (name == "fate_line") {
        item.color = cv::Scalar(220, 50, 180);
        item.label = "Fate";
    }
    return item;
}

} // namespace

bool loadLandmarksJson(const std::string& path, geometry::HandLandmarks& out, std::string* err) {
    std::ifstream f(path);
    if (!f.is_open()) {
        if (err) *err = "Cannot open landmarks file: " + path;
        return false;
    }
    nlohmann::json js;
    try {
        f >> js;
    } catch (const std::exception& e) {
        if (err) *err = e.what();
        return false;
    }
    if (!js.is_array() || js.size() < 21) {
        if (err) *err = "Landmarks JSON must be an array of 21 points";
        return false;
    }
    out.is_normalized = true;
    for (size_t i = 0; i < 21; ++i) {
        if (js[i].is_object()) {
            out.points[i].x = js[i].value("x", 0.0f);
            out.points[i].y = js[i].value("y", 0.0f);
        } else if (js[i].is_array() && js[i].size() >= 2) {
            out.points[i].x = js[i][0].get<float>();
            out.points[i].y = js[i][1].get<float>();
        } else {
            if (err) *err = "Invalid landmark entry at index " + std::to_string(i);
            return false;
        }
    }
    return true;
}

bool saveLandmarksJson(const std::string& path, const geometry::HandLandmarks& lms, std::string* err) {
    nlohmann::json arr = nlohmann::json::array();
    for (const auto& p : lms.points) {
        arr.push_back({{"x", p.x}, {"y", p.y}});
    }
    std::ofstream f(path);
    if (!f.is_open()) {
        if (err) *err = "Cannot write landmarks file: " + path;
        return false;
    }
    f << arr.dump(2);
    return true;
}

geometry::HandLandmarks warpLandmarksToCanonical(const geometry::HandLandmarks& src, const geometry::RoiTransform& xf,
                                                   int img_w, int img_h) {
    geometry::HandLandmarks out;
    out.is_normalized = false;
    const float sx = src.is_normalized ? static_cast<float>(img_w) : 1.0f;
    const float sy = src.is_normalized ? static_cast<float>(img_h) : 1.0f;
    for (size_t i = 0; i < 21; ++i) {
        const float px = src.points[i].x * sx;
        const float py = src.points[i].y * sy;
        const auto mapped = applyAffine(xf.M, px, py);
        out.points[i].x = mapped.x;
        out.points[i].y = mapped.y;
    }
    return out;
}

PalmInspectResult runPalmInspect(const cv::Mat& bgr, const geometry::HandLandmarks* landmarks_or_null,
                                 const PalmInspectOptions& options, std::ostream* log) {
    PalmInspectResult res;
    if (bgr.empty()) {
        res.error = "Empty input image";
        return res;
    }
    res.source_bgr = bgr.clone();

    // Landmarks
    if (landmarks_or_null != nullptr) {
        res.landmarks = *landmarks_or_null;
        logLine(log, "[STAGE 2] Using provided landmarks");
    } else {
        logLine(log, "[STAGE 2] Running desktop hand landmark detector...");
        if (options.is_left_hand_hint && options.auto_landmarks_model.empty()) {
            res.landmarks = adapters::HandLandmarkDetector::generateHeuristicLandmarks(bgr.cols, bgr.rows, true);
        } else {
            adapters::HandLandmarkDetector detector(options.auto_landmarks_model);
            auto opt_lms = detector.detect(bgr);
            if (opt_lms) {
                res.landmarks = *opt_lms;
            } else {
                res.error = "Landmark detection failed";
                return res;
            }
        }
    }

    res.is_left_hand = options.is_left_hand_hint || geometry::detectIsLeftHand(res.landmarks);
    const int canonical_dim = options.canonical_dim;

    logLine(log, "[STAGE 3] Warping to canonical upright palm patch...");
    res.transform = geometry::computeCanonicalPalmTransform(res.landmarks, bgr.cols, bgr.rows, canonical_dim);
    res.canonical_bgr = geometry::warpToCanonical(bgr, res.transform.M, canonical_dim);
    res.landmarks_canonical = warpLandmarksToCanonical(res.landmarks, res.transform, bgr.cols, bgr.rows);

    logLine(log, "[STAGE 4] Extracting sub-pixel ridges...");
    Frame canonical_frame(res.canonical_bgr, "canonical_palm", 1);
    canonical_frame.metadata["is_left_hand"] = res.is_left_hand ? "true" : "false";

    quality::QualityThresholds q_thresh;
    q_thresh.min_blur_score = 15.0;
    q_thresh.min_brightness = 5.0;
    auto const quality_analyzer = std::make_shared<quality::LaplacianQualityAnalyzer>(q_thresh);
    auto const tuned_cfg = domains::PalmParameters::computeFromLandmarks(res.landmarks, bgr.cols, bgr.rows);
    auto const ridge_extractor = std::make_shared<ridge::StegerRidgeExtractor>(tuned_cfg);
    pipeline::SyncPerceptionPipeline pipeline(quality_analyzer, ridge_extractor);
    auto result = pipeline.process(canonical_frame);
    res.quality = result.quality;
    res.ridge_graph = result.ridge_graph;

    logLine(log, "[STAGE 4b] Filtering by hand mask...");
    auto const canonical_boundary = adapters::HandBoundaryDetector::detectBoundary(res.canonical_bgr);
    if (canonical_boundary.is_valid) {
        adapters::HandBoundaryDetector::filterGraphByMask(res.ridge_graph, canonical_boundary.mask,
                                                          options.mask_min_inside);
    }
    res.segments_after_mask = res.ridge_graph.segments.size();

    logLine(log, "[STAGE 4c] Anatomy + palm-disc ridge filter...");
    res.anatomy_raw =
        domain::HandAnatomyDecomposer::decompose(res.landmarks, static_cast<float>(bgr.cols), static_cast<float>(bgr.rows));
    res.anatomy_canonical = domain::HandAnatomyDecomposer::decompose(res.landmarks_canonical, 1.0f, 1.0f);
    if (res.anatomy_canonical.is_valid) {
        const size_t removed = domain::CreaseChainLinker::filterGraphByPalmDisc(
            res.ridge_graph, res.anatomy_canonical.central_palm_disc, options.palm_disc_min_inside);
        logLine(log, "  - Palm-disc purge removed " + std::to_string(removed) + " segments");
    }
    res.segments_after_palm_disc = res.ridge_graph.segments.size();

    logLine(log, "[STAGE 5] PalmDomainPack + landmark-relative major lines...");
    domains::PalmDomainPack palm_pack;
    DomainInput const d_in{canonical_frame, res.ridge_graph, res.quality, &res.landmarks_canonical};
    auto const d_out = palm_pack.process(d_in);

    domain::MajorLineLandmarks priors;
    priors.landmarks = res.landmarks_canonical;
    priors.valid = true;
    res.major_lines = domain::CreaseChainLinker::extractMajorLines(
        res.ridge_graph, res.is_left_hand, canonical_dim, priors, options.link_options);

    // Prefer linker chains (with confidence) already mirrored into domain payload.
    res.evidence = nlohmann::json::object();
    res.evidence["quality"] = {{"blur_score", res.quality.blur_score},
                               {"brightness_score", res.quality.brightness_score},
                               {"contrast_score", res.quality.contrast_score},
                               {"is_usable", res.quality.is_usable}};
    res.evidence["domain_payload"] = d_out.domain_payload;
    enrichImageSpacePoints(res.evidence["domain_payload"], res.transform.M_inv, bgr.cols, bgr.rows);

    nlohmann::json ev_array = nlohmann::json::array();
    if (options.debug_all_ridges) {
        for (const auto& ev : result.evidence) {
            ev_array.push_back(ev.to_json());
        }
    }
    for (const auto& ev : d_out.domain_evidence) {
        ev_array.push_back(ev.to_json());
    }
    res.evidence["evidence"] = ev_array;
    res.evidence["landmarks_source"] = landmarks_or_null ? "provided" : "detector";
    res.evidence["is_left_hand"] = res.is_left_hand;
    res.evidence["segments_after_mask"] = res.segments_after_mask;
    res.evidence["segments_after_palm_disc"] = res.segments_after_palm_disc;

    // Canonical overlay
    adapters::OverlayConfig o_cfg;
    o_cfg.draw_subpixel_polylines = options.debug_all_ridges;
    o_cfg.draw_normals = false;
    o_cfg.draw_hud = true;
    o_cfg.subpixel_shift = 4;
    o_cfg.line_color = cv::Scalar(100, 220, 150);
    if (canonical_boundary.is_valid) {
        o_cfg.draw_boundary = true;
        o_cfg.boundary_contours = canonical_boundary.contours;
        o_cfg.boundary_color = cv::Scalar(255, 200, 50);
        o_cfg.boundary_thickness = 2;
    }
    o_cfg.draw_continuous_creases = true;
    for (const auto& [name, line] : res.major_lines) {
        if (!line.is_valid || line.points.size() < 2) continue;
        o_cfg.continuous_creases.push_back(makeCreaseItem(name, line, nullptr, 3));
    }
    res.canonical_overlay = adapters::OverlayRenderer::render(res.canonical_bgr, res.ridge_graph, res.quality, o_cfg);
    if (options.draw_landmarks) {
        drawLandmarkDots(res.canonical_overlay, res.landmarks_canonical, canonical_dim, canonical_dim);
    }

    // Full-image overlay
    auto const raw_boundary = adapters::HandBoundaryDetector::detectBoundary(bgr, 0.25f);
    adapters::OverlayConfig full_cfg;
    full_cfg.draw_subpixel_polylines = options.debug_all_ridges;
    full_cfg.draw_normals = false;
    full_cfg.draw_hud = false;
    full_cfg.line_thickness = std::max(3, bgr.cols / 600);
    full_cfg.subpixel_shift = 4;
    full_cfg.line_color = cv::Scalar(100, 220, 150);
    if (raw_boundary.is_valid) {
        full_cfg.draw_boundary = true;
        full_cfg.boundary_contours = raw_boundary.contours;
        full_cfg.boundary_color = cv::Scalar(255, 200, 50);
        full_cfg.boundary_thickness = std::max(4, bgr.cols / 400);
    }
    if (res.anatomy_raw.is_valid) {
        full_cfg.draw_anatomy = true;
        full_cfg.anatomy_color = cv::Scalar(30, 160, 255);
        full_cfg.anatomy_thickness = std::max(2, bgr.cols / 500);
        for (const auto& digit : res.anatomy_raw.digits) {
            std::vector<cv::Point> ray;
            for (const auto& pt : digit.joint_chain) {
                ray.emplace_back(static_cast<int>(std::round(pt.x)), static_cast<int>(std::round(pt.y)));
            }
            full_cfg.anatomy_contours.push_back(std::move(ray));
        }
    }
    full_cfg.draw_continuous_creases = true;
    const int crease_thick = std::max(5, bgr.cols / 300);
    for (const auto& [name, line] : res.major_lines) {
        if (!line.is_valid || line.points.size() < 2) continue;
        full_cfg.continuous_creases.push_back(makeCreaseItem(name, line, &res.transform.M_inv, crease_thick));
    }

    RidgeGraph full_graph = res.ridge_graph;
    geometry::invertGraphCoordinates(full_graph, res.transform.M_inv);
    if (raw_boundary.is_valid) {
        adapters::HandBoundaryDetector::filterGraphByMask(full_graph, raw_boundary.mask, options.mask_min_inside);
    }
    res.full_overlay = adapters::OverlayRenderer::render(bgr, full_graph, res.quality, full_cfg);
    if (options.draw_landmarks) {
        drawLandmarkDots(res.full_overlay, res.landmarks, bgr.cols, bgr.rows);
    }

    res.ok = true;
    logLine(log, "[PASS] Palm inspect complete");
    return res;
}

} // namespace CurvEngine::tools
