#include "curv/domain/MountReliefAnalyzer.hpp"

#include "curv/surface/SurfaceRelief.hpp"

#include <numeric>
#include <unordered_map>

namespace CurvEngine::domain {

namespace {

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
    for (const auto& frame : orbit_session_json["frames"]) {
        CurvEngine::surface::OrbitFrameMeta meta;
        meta.bin_id = frame.value("bin", "");
        meta.device_roll_deg = frame.value("device_roll_deg", 0.0f);
        if (frame.contains("gravity") && frame["gravity"].is_array()) {
            for (const auto& g : frame["gravity"]) {
                meta.gravity_xyz.push_back(g.get<float>());
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

    const auto relief = CurvEngine::surface::SurfaceRelief::estimateFromOrbitFrames(frames, kMounts);
    for (const auto& r : relief) {
        MountReliefEntry entry;
        entry.mount = r.region_id;
        entry.height_relative = r.height_relative;
        entry.prominence = gradeFromRelative(r.height_relative);
        entry.confidence = r.confidence;
        entry.view_count = r.view_count;
        result.mounts.push_back(entry);
    }

    for (const auto& m : result.mounts) {
        if (m.mount == "venus" && m.prominence == MountProminence::Elevated) {
            result.venus_elevated = true;
            break;
        }
    }
    return result;
}

} // namespace CurvEngine::domain
