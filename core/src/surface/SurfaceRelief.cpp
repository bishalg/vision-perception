#include "curv/surface/SurfaceRelief.hpp"

#include <algorithm>
#include <cmath>
#include <numeric>
#include <unordered_map>

namespace CurvEngine::surface {

namespace {

// Minimum viewpoint rotation for a frame to contribute parallax (radians).
constexpr float kMinTiltRad = 0.05f;
// Sanity clamp for parallax-derived relief height (mm).
constexpr float kMaxReliefMm = 6.0f;
// Fusion weights when image-measured parallax is available.
constexpr float kTiltWeight = 0.35f;
constexpr float kParallaxWeight = 0.65f;

float medianOf(std::vector<float> v) {
    if (v.empty()) {
        return 0.0f;
    }
    const size_t mid = v.size() / 2;
    std::nth_element(v.begin(), v.begin() + static_cast<ptrdiff_t>(mid), v.end());
    if (v.size() % 2 == 1) {
        return v[mid];
    }
    const float upper = v[mid];
    std::nth_element(v.begin(), v.begin() + static_cast<ptrdiff_t>(mid - 1), v.end());
    return (upper + v[mid - 1]) * 0.5f;
}

float tiltExposure(const OrbitFrameMeta& frame) {
    const float gz = (frame.gravity_xyz.size() >= 3) ? frame.gravity_xyz[2] : 0.0f;
    return std::abs(gz) + std::abs(frame.device_roll_deg) * 0.01f;
}

float zscore(float v, float mean, float std_dev) {
    return (std_dev > 1e-4f) ? (v - mean) / std_dev : 0.0f;
}

void accumulateMeanStd(const std::unordered_map<std::string, float>& acc,
                       const std::unordered_map<std::string, int>& counts, const std::vector<std::string>& region_ids,
                       float& mean, float& std_dev) {
    std::vector<float> values;
    values.reserve(region_ids.size());
    for (const auto& id : region_ids) {
        const auto count_it = counts.find(id);
        if (count_it != counts.end() && count_it->second > 0) {
            values.push_back(acc.at(id) / static_cast<float>(count_it->second));
        }
    }
    mean = values.empty() ? 0.0f
                          : std::accumulate(values.begin(), values.end(), 0.0f) / static_cast<float>(values.size());
    float var = 0.0f;
    for (float v : values) {
        const float d = v - mean;
        var += d * d;
    }
    std_dev = values.empty() ? 1.0f : std::sqrt(var / static_cast<float>(values.size()));
}

} // anonymous namespace

std::vector<ReliefSample> SurfaceRelief::estimateFromOrbitFrames(const std::vector<OrbitFrameMeta>& frames,
                                                                 const std::vector<std::string>& region_ids) {
    std::unordered_map<std::string, float> height_acc;
    std::unordered_map<std::string, int> view_counts;

    for (const auto& frame : frames) {
        const float gz = (frame.gravity_xyz.size() >= 3) ? frame.gravity_xyz[2] : 0.0f;
        const float tilt = std::abs(gz) + std::abs(frame.device_roll_deg) * 0.01f;
        for (const auto& region : frame.primary_regions) {
            height_acc[region] += tilt + 0.15f;
            view_counts[region] += 1;
        }
    }

    std::vector<float> values;
    values.reserve(region_ids.size());
    for (const auto& id : region_ids) {
        if (view_counts.count(id) > 0) {
            values.push_back(height_acc[id] / static_cast<float>(view_counts[id]));
        }
    }

    const float mean = values.empty() ? 0.0f
                                      : std::accumulate(values.begin(), values.end(), 0.0f) /
                                            static_cast<float>(values.size());
    float var = 0.0f;
    for (float v : values) {
        const float d = v - mean;
        var += d * d;
    }
    const float std_dev = (values.empty()) ? 1.0f : std::sqrt(var / static_cast<float>(values.size()));

    std::vector<ReliefSample> out;
    out.reserve(region_ids.size());
    for (const auto& id : region_ids) {
        ReliefSample sample;
        sample.region_id = id;
        sample.view_count = view_counts.count(id) > 0 ? view_counts[id] : 0;
        if (sample.view_count == 0) {
            sample.height_relative = 0.0f;
            sample.confidence = 0.0f;
        } else {
            const float h = height_acc[id] / static_cast<float>(view_counts[id]);
            sample.height_relative = (std_dev > 1e-4f) ? (h - mean) / std_dev : 0.0f;
            sample.confidence = std::min(0.95f, 0.35f + static_cast<float>(sample.view_count) * 0.1f);
        }
        out.push_back(sample);
    }
    return out;
}

std::vector<ReliefSample> SurfaceRelief::estimateMetricRelief(
    const std::vector<OrbitFrameMeta>& frames, const std::vector<std::array<float, 2>>& reference_landmarks,
    float palm_width_px, const MetricScale& scale, const std::vector<std::string>& region_ids,
    const std::unordered_map<std::string, std::vector<int>>& region_anchors) {
    // Tilt-exposure prior (identical to estimateFromOrbitFrames).
    std::unordered_map<std::string, float> tilt_acc;
    std::unordered_map<std::string, int> view_counts;
    for (const auto& frame : frames) {
        const float tilt = tiltExposure(frame);
        for (const auto& region : frame.primary_regions) {
            tilt_acc[region] += tilt + 0.15f;
            view_counts[region] += 1;
        }
    }
    float tilt_mean = 0.0f;
    float tilt_std = 1.0f;
    accumulateMeanStd(tilt_acc, view_counts, region_ids, tilt_mean, tilt_std);

    // Parallax path needs a reference view and a pixel scale.
    const bool metric_ready = !reference_landmarks.empty() && palm_width_px > 1e-3f && scale.palm_width_mm > 1e-3f;
    std::array<float, 2> ref_center{0.5f, 0.5f};
    if (metric_ready) {
        float cx = 0.0f;
        float cy = 0.0f;
        for (const auto& p : reference_landmarks) {
            cx += p[0];
            cy += p[1];
        }
        ref_center = {cx / static_cast<float>(reference_landmarks.size()),
                      cy / static_cast<float>(reference_landmarks.size())};
    }

    std::unordered_map<std::string, float> par_acc; // |height| cue in mm
    std::unordered_map<std::string, int> par_counts;
    std::unordered_map<std::string, float> depth_acc; // measured depth means in mm
    std::unordered_map<std::string, int> depth_counts;

    for (const auto& frame : frames) {
        for (const auto& kv : frame.depth_mm) {
            depth_acc[kv.first] += kv.second;
            depth_counts[kv.first] += 1;
        }
        if (!metric_ready || frame.landmarks.size() != reference_landmarks.size() || reference_landmarks.size() < 21 ||
            frame.image_width <= 0 || std::abs(frame.tilt_rad_vs_ref) < kMinTiltRad) {
            continue;
        }
        // Rigid translation between frame and reference (median displacement).
        std::vector<float> dxs;
        std::vector<float> dys;
        dxs.reserve(reference_landmarks.size());
        dys.reserve(reference_landmarks.size());
        for (size_t i = 0; i < reference_landmarks.size(); ++i) {
            dxs.push_back(frame.landmarks[i][0] - reference_landmarks[i][0]);
            dys.push_back(frame.landmarks[i][1] - reference_landmarks[i][1]);
        }
        const float tx = medianOf(dxs);
        const float ty = medianOf(dys);
        // Palm-plane motion = median radial residual over all landmarks.
        std::vector<float> radials;
        radials.reserve(reference_landmarks.size());
        for (size_t i = 0; i < reference_landmarks.size(); ++i) {
            float dirx = reference_landmarks[i][0] - ref_center[0];
            float diry = reference_landmarks[i][1] - ref_center[1];
            const float len = std::sqrt(dirx * dirx + diry * diry);
            if (len < 1e-6f) {
                continue;
            }
            dirx /= len;
            diry /= len;
            radials.push_back(((dxs[i] - tx) * dirx + (dys[i] - ty) * diry) *
                              static_cast<float>(frame.image_width));
        }
        if (radials.empty()) {
            continue;
        }
        const float plane_px = medianOf(radials);
        const float theta = std::abs(frame.tilt_rad_vs_ref);
        const float mm_per_px = scale.palm_width_mm / palm_width_px;
        for (const auto& region : frame.primary_regions) {
            const auto anchors_it = region_anchors.find(region);
            if (anchors_it == region_anchors.end() || anchors_it->second.empty()) {
                continue;
            }
            float sum_px = 0.0f;
            int n = 0;
            for (int idx : anchors_it->second) {
                if (idx < 0 || static_cast<size_t>(idx) >= reference_landmarks.size()) {
                    continue;
                }
                float dirx = reference_landmarks[static_cast<size_t>(idx)][0] - ref_center[0];
                float diry = reference_landmarks[static_cast<size_t>(idx)][1] - ref_center[1];
                const float len = std::sqrt(dirx * dirx + diry * diry);
                if (len < 1e-6f) {
                    continue;
                }
                dirx /= len;
                diry /= len;
                const size_t k = static_cast<size_t>(idx);
                sum_px += ((dxs[k] - tx) * dirx + (dys[k] - ty) * diry) * static_cast<float>(frame.image_width);
                ++n;
            }
            if (n == 0) {
                continue;
            }
            // Differential disparity vs the palm plane, converted to mm of
            // relief height via the small-angle relation h ~= disparity/theta.
            const float rel_mm = (sum_px / static_cast<float>(n) - plane_px) * mm_per_px;
            par_acc[region] += std::abs(rel_mm) / theta;
            par_counts[region] += 1;
        }
    }

    float par_mean = 0.0f;
    float par_std = 1.0f;
    accumulateMeanStd(par_acc, par_counts, region_ids, par_mean, par_std);

    // Measured-depth plane = median of per-region measured means.
    std::vector<float> measured_means;
    for (const auto& id : region_ids) {
        const auto it = depth_counts.find(id);
        if (it != depth_counts.end() && it->second > 0) {
            measured_means.push_back(depth_acc[id] / static_cast<float>(it->second));
        }
    }
    const float depth_plane = medianOf(measured_means);

    // z-scores of measured heights so prominence grading on the depth path
    // reflects measured millimeters, not the tilt prior.
    std::unordered_map<std::string, float> measured_z;
    {
        std::unordered_map<std::string, float> measured_acc;
        std::unordered_map<std::string, int> measured_counts;
        for (const auto& id : region_ids) {
            const auto it = depth_counts.find(id);
            if (it != depth_counts.end() && it->second > 0) {
                measured_acc[id] = (depth_acc[id] / static_cast<float>(it->second)) - depth_plane;
                measured_counts[id] = 1;
            }
        }
        float m_mean = 0.0f;
        float m_std = 1.0f;
        accumulateMeanStd(measured_acc, measured_counts, region_ids, m_mean, m_std);
        for (const auto& kv : measured_acc) {
            measured_z[kv.first] = zscore(kv.second, m_mean, m_std);
        }
    }

    std::vector<ReliefSample> out;
    out.reserve(region_ids.size());
    for (const auto& id : region_ids) {
        ReliefSample sample;
        sample.region_id = id;
        sample.view_count = view_counts.count(id) > 0 ? view_counts[id] : 0;
        if (sample.view_count == 0) {
            out.push_back(sample);
            continue;
        }
        const float tilt_z = zscore(tilt_acc[id] / static_cast<float>(sample.view_count), tilt_mean, tilt_std);
        const auto par_it = par_counts.find(id);
        const bool has_parallax = par_it != par_counts.end() && par_it->second > 0;
        float fused = tilt_z;
        float height_mag_mm = -1.0f;
        if (has_parallax) {
            const float par_z =
                zscore(par_acc[id] / static_cast<float>(par_it->second), par_mean, par_std);
            fused = kTiltWeight * tilt_z + kParallaxWeight * par_z;
            height_mag_mm =
                std::min(kMaxReliefMm, par_acc[id] / static_cast<float>(par_it->second));
        }
        sample.height_relative = fused;
        if (height_mag_mm >= 0.0f) {
            // Magnitude from parallax geometry; polarity from relative ranking.
            sample.height_mm = (fused >= 0.0f ? 1.0f : -1.0f) * height_mag_mm;
        }
        const auto depth_it = depth_counts.find(id);
        if (depth_it != depth_counts.end() && depth_it->second > 0) {
            // Hardware depth wins over the parallax estimate when present.
            sample.height_mm = (depth_acc[id] / static_cast<float>(depth_it->second)) - depth_plane;
            sample.measured_depth = true;
            if (measured_z.count(id) > 0) {
                sample.height_relative = measured_z.at(id);
            }
        }
        sample.confidence = std::min(0.95f, 0.35f + static_cast<float>(sample.view_count) * 0.1f);
        out.push_back(sample);
    }
    return out;
}

} // namespace CurvEngine::surface
