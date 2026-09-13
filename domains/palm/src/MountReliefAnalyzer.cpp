#include "curv/domain/MountReliefAnalyzer.hpp"

#include "curv/surface/SurfaceRelief.hpp"

#include <algorithm>
#include <cmath>
#include <numeric>
#include <unordered_map>

namespace CurvEngine::domain {

namespace {

/// Mount -> MediaPipe-order landmark indices used as parallax probes.
/// Thenar/hypothenar eminences sit at the thumb/pinky bases; the four finger
/// mounts sit under their MCP/finger chains; Mars is the mid-palm plain.
const std::unordered_map<std::string, std::vector<int>>& mountAnchors() {
    static const std::unordered_map<std::string, std::vector<int>> kAnchors = {
        {"venus", {1, 2, 5}},   {"jupiter", {5, 6, 7}},  {"saturn", {9, 10, 11}},
        {"apollo", {13, 14, 15}}, {"mercury", {17, 18, 19}}, {"luna", {0, 17, 18}},
        {"mars", {5, 9, 13, 17}}};
    return kAnchors;
}

float gravityAngleRad(const std::vector<float>& a, const std::vector<float>& b) {
    if (a.size() < 3 || b.size() < 3) {
        return 0.0f;
    }
    const float dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    const float na = std::sqrt(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]);
    const float nb = std::sqrt(b[0] * b[0] + b[1] * b[1] + b[2] * b[2]);
    if (na < 1e-6f || nb < 1e-6f) {
        return 0.0f;
    }
    const float cos_theta = std::max(-1.0f, std::min(1.0f, dot / (na * nb)));
    return std::acos(cos_theta);
}

bool parseLandmarks(const nlohmann::json& j, std::vector<std::array<float, 2>>& out) {
    if (!j.is_array() || j.empty()) {
        return false;
    }
    out.clear();
    out.reserve(j.size());
    for (const auto& p : j) {
        if (!p.is_array() || p.size() < 2) {
            return false;
        }
        out.push_back({p[0].get<float>(), p[1].get<float>()});
    }
    return out.size() >= 21;
}

float palmWidthPxFrom(const std::vector<std::array<float, 2>>& landmarks, int image_width) {
    // Metacarpal breadth: index MCP (5) -> pinky MCP (17), in pixels.
    if (landmarks.size() < 18 || image_width <= 0) {
        return 0.0f;
    }
    const float dx = landmarks[17][0] - landmarks[5][0];
    const float dy = landmarks[17][1] - landmarks[5][1];
    return std::sqrt(dx * dx + dy * dy) * static_cast<float>(image_width);
}

MountProminence gradeFromRelative(float rel) {
    if (rel > 0.45f) {
        return MountProminence::Elevated;
    }
    if (rel < -0.45f) {
        return MountProminence::Deficient;
    }
    return MountProminence::Average;
}

const char* prominenceString(MountProminence p) {
    switch (p) {
    case MountProminence::Deficient:
        return "deficient";
    case MountProminence::Elevated:
        return "elevated";
    default:
        return "average";
    }
}

} // anonymous namespace

nlohmann::json MountReliefEntry::toJson() const {
    nlohmann::json j = {{"mount", mount},
                        {"height_relative", height_relative},
                        {"prominence", prominenceString(prominence)},
                        {"confidence", confidence},
                        {"view_count", view_count}};
    if (height_mm >= 0.0f) {
        j["height_mm"] = height_mm;
    }
    return j;
}

nlohmann::json MountReliefMap::toJson() const {
    nlohmann::json arr = nlohmann::json::array();
    for (const auto& m : mounts) {
        arr.push_back(m.toJson());
    }
    return {{"mounts", arr}, {"venus_elevated", venus_elevated}, {"source", source}};
}

MountReliefMap MountReliefAnalyzer::analyzeOrbitSession(const nlohmann::json& orbit_session_json) {
    MountReliefMap result;
    if (!orbit_session_json.contains("frames") || !orbit_session_json["frames"].is_array()) {
        return result;
    }

    static const std::vector<std::string> kMounts = {"jupiter", "saturn", "apollo", "mercury",
                                                     "venus",   "luna",   "mars"};

    std::vector<CurvEngine::surface::OrbitFrameMeta> frames;
    std::vector<float> ref_gravity;
    if (orbit_session_json.contains("reference_gravity") && orbit_session_json["reference_gravity"].is_array()) {
        for (const auto& g : orbit_session_json["reference_gravity"]) {
            ref_gravity.push_back(g.get<float>());
        }
    }
    std::vector<std::array<float, 2>> ref_landmarks;
    if (orbit_session_json.contains("reference_landmarks")) {
        parseLandmarks(orbit_session_json["reference_landmarks"], ref_landmarks);
    }
    CurvEngine::surface::MetricScale metric_scale;
    if (orbit_session_json.contains("metric") && orbit_session_json["metric"].is_object()) {
        metric_scale.palm_width_mm = orbit_session_json["metric"].value("palm_width_mm", metric_scale.palm_width_mm);
    }
    for (const auto& frame : orbit_session_json["frames"]) {
        CurvEngine::surface::OrbitFrameMeta meta;
        meta.bin_id = frame.value("bin", "");
        meta.device_roll_deg = frame.value("device_roll_deg", 0.0f);
        if (frame.contains("gravity") && frame["gravity"].is_array()) {
            for (const auto& g : frame["gravity"]) {
                meta.gravity_xyz.push_back(g.get<float>());
            }
        }
        if (frame.contains("landmarks")) {
            parseLandmarks(frame["landmarks"], meta.landmarks);
        }
        // Swift (snake_case after the v1 key fix) and Kotlin both emit these.
        meta.image_width = frame.value("image_width", frame.value("imageWidth", 0));
        meta.image_height = frame.value("image_height", frame.value("imageHeight", 0));
        meta.tilt_rad_vs_ref = gravityAngleRad(meta.gravity_xyz, ref_gravity);
        if (frame.contains("depth_mm") && frame["depth_mm"].is_object()) {
            for (const auto& kv : frame["depth_mm"].items()) {
                if (kv.value().is_number()) {
                    meta.depth_mm[kv.key()] = kv.value().get<float>();
                }
            }
        }
        // Primary mounts keyed by bin id prefix mapping.
        static const std::unordered_map<std::string, std::vector<std::string>> kBinMounts = {
            {"flat_center", {"venus", "jupiter", "saturn", "apollo", "mercury", "luna", "mars"}},
            {"tilt_left", {"venus", "jupiter", "mars"}},
            {"tilt_right", {"luna", "mercury", "apollo"}},
            {"thumb_toward", {"venus", "jupiter"}},
            {"thumb_away", {"luna", "mars"}},
            {"tilt_forward", {"venus", "saturn", "apollo"}}};
        if (kBinMounts.count(meta.bin_id) > 0) {
            meta.primary_regions = kBinMounts.at(meta.bin_id);
        }
        frames.push_back(meta);
    }

    // Per-frame pixel scale from the frame's own metacarpal breadth; fall back
    // to the reference view mapped onto the first known image width.
    float palm_width_px = 0.0f;
    for (const auto& frame : frames) {
        const float w = palmWidthPxFrom(frame.landmarks, frame.image_width);
        if (w > palm_width_px) {
            palm_width_px = w;
        }
    }
    if (palm_width_px <= 0.0f && !ref_landmarks.empty()) {
        int w = 0;
        for (const auto& frame : frames) {
            w = std::max(w, frame.image_width);
        }
        palm_width_px = palmWidthPxFrom(ref_landmarks, w);
    }

    const auto relief = CurvEngine::surface::SurfaceRelief::estimateMetricRelief(frames, ref_landmarks, palm_width_px,
                                                                                 metric_scale, kMounts, mountAnchors());
    bool any_metric = false;
    bool all_measured = !relief.empty();
    for (const auto& r : relief) {
        MountReliefEntry entry;
        entry.mount = r.region_id;
        entry.height_relative = r.height_relative;
        entry.height_mm = r.height_mm;
        entry.prominence = gradeFromRelative(r.height_relative);
        entry.confidence = r.confidence;
        entry.view_count = r.view_count;
        result.mounts.push_back(entry);
        // NOTE: -1.0f is the exact uncalibrated sentinel (see ReliefSample);
        // a computed height equaling it bit-for-bit is measure-zero.
        any_metric = any_metric || r.height_mm != -1.0f || r.measured_depth;
        all_measured = all_measured && r.measured_depth;
    }
    result.source = all_measured ? "orbit_depth_v1" : (any_metric ? "orbit_metric_v1" : "orbit_tilt_proxy_v1");

    for (const auto& m : result.mounts) {
        if (m.mount == "venus" && m.prominence == MountProminence::Elevated) {
            result.venus_elevated = true;
            break;
        }
    }
    return result;
}

} // namespace CurvEngine::domain
