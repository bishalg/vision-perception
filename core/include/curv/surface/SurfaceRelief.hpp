#pragma once

#include <array>
#include <string>
#include <unordered_map>
#include <vector>

namespace CurvEngine::surface {

/**
 * @brief Domain-agnostic multi-view surface relief estimate from tilt proxies.
 * Palm domain maps mount polygons onto this; no palmistry vocabulary here.
 */
struct ReliefSample {
    std::string region_id;
    float height_relative{0.0f};
    float height_mm{-1.0f}; // -1 = uncalibrated (no metric/depth evidence)
    float confidence{0.0f};
    int view_count{0};
    bool measured_depth{false}; // true when height_mm comes from hardware depth
};

struct OrbitFrameMeta {
    std::string bin_id;
    std::vector<float> gravity_xyz; // x, y, z
    float device_roll_deg{0.0f};
    std::vector<std::string> primary_regions;
    // --- Metric extensions (all optional; empty/zero = unavailable) ---
    std::vector<std::array<float, 2>> landmarks; // normalized MediaPipe-order points
    int image_width{0};
    int image_height{0};
    /// Viewpoint rotation vs the flat reference view, radians (from gravity
    /// vectors). 0 = unknown; frame then contributes tilt exposure only.
    float tilt_rad_vs_ref{0.0f};
    /// Measured per-region depth in mm from hardware depth (LiDAR/TrueDepth).
    std::unordered_map<std::string, float> depth_mm;
};

struct MetricScale {
    /// Adult metacarpal-breadth prior (landmark 5 -> 17). Per-user calibration
    /// is future work; confidence is capped while the prior is in use.
    float palm_width_mm{80.0f};
    const char* source{"anatomical_prior_v1"};
};

class SurfaceRelief {
public:
    [[nodiscard]] static std::vector<ReliefSample> estimateFromOrbitFrames(const std::vector<OrbitFrameMeta>& frames,
                                                                           const std::vector<std::string>& region_ids);

    /**
     * @brief Metric relief from multi-view parallax + optional measured depth.
     *
     * reference_landmarks: normalized points of the flat reference view.
     *   Empty = tilt-proxy fallback (same output as estimateFromOrbitFrames).
     * palm_width_px: metacarpal breadth (landmark 5 -> 17) in pixels, measured
     *   per frame from its own landmarks. <= 0 disables the parallax path.
     * region_anchors: region id -> landmark indices used as parallax probes.
     *
     * Geometry note: under a viewpoint rotation of theta radians, a surface
     * point at height h above the local palm plane shows a differential image
     * disparity of ~h*theta relative to the plane median. height_mm is the
     * mean of that quotient over valid frames (magnitude from parallax;
     * polarity from the fused relative ranking), clamped to +/-6 mm.
     */
    [[nodiscard]] static std::vector<ReliefSample>
    estimateMetricRelief(const std::vector<OrbitFrameMeta>& frames,
                         const std::vector<std::array<float, 2>>& reference_landmarks, float palm_width_px,
                         const MetricScale& scale, const std::vector<std::string>& region_ids,
                         const std::unordered_map<std::string, std::vector<int>>& region_anchors);
};

} // namespace CurvEngine::surface
