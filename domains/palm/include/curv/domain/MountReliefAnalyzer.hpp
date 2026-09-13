#pragma once

#include <nlohmann/json.hpp>
#include <string>
#include <vector>

namespace CurvEngine::domain {

enum class MountProminence { Deficient, Average, Elevated };

struct MountReliefEntry {
    std::string mount;
    float height_mm{-1.0f}; // -1 = uncalibrated
    float height_relative{0.0f};
    MountProminence prominence{MountProminence::Average};
    float confidence{0.0f};
    int view_count{0};

    [[nodiscard]] nlohmann::json toJson() const;
};

struct MountReliefMap {
    std::vector<MountReliefEntry> mounts;
    bool venus_elevated{false};
    std::string source{"orbit_tilt_proxy_v1"};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Maps orbit session metadata to Samudrika mount prominence grades.
 *
 * Consumes `curv_orbit_session_v1` JSON. Additive optional fields enable the
 * metric path without breaking legacy sessions:
 * - `reference_landmarks`: 21 normalized points of the flat reference view.
 * - `reference_gravity`: device gravity at the reference view.
 * - `metric: {"palm_width_mm"}`: anatomical scale prior (default 80 mm).
 * - per frame `landmarks`, `image_width`, `depth_mm: {mount: mm}`.
 *
 * Result `source` is `orbit_depth_v1` (hardware depth), `orbit_metric_v1`
 * (parallax + mm scale), or `orbit_tilt_proxy_v1` (legacy fallback).
 */
class MountReliefAnalyzer {
public:
    [[nodiscard]] static MountReliefMap analyzeOrbitSession(const nlohmann::json& orbit_session_json);
};

} // namespace CurvEngine::domain
