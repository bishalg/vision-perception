#include "curv/surface/SurfaceRelief.hpp"

#include <algorithm>
#include <cmath>
#include <numeric>
#include <unordered_map>

namespace CurvEngine::surface {

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

} // namespace CurvEngine::surface
