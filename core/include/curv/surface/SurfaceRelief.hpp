#pragma once

#include <string>
#include <vector>

namespace CurvEngine::surface {

/**
 * @brief Domain-agnostic multi-view surface relief estimate from tilt proxies.
 * Palm domain maps mount polygons onto this; no palmistry vocabulary here.
 */
struct ReliefSample {
    std::string region_id;
    float height_relative{0.0f};
    float confidence{0.0f};
    int view_count{0};
};

struct OrbitFrameMeta {
    std::string bin_id;
    std::vector<float> gravity_xyz; // x, y, z
    float device_roll_deg{0.0f};
    std::vector<std::string> primary_regions;
};

class SurfaceRelief {
public:
    [[nodiscard]] static std::vector<ReliefSample>
    estimateFromOrbitFrames(const std::vector<OrbitFrameMeta>& frames, const std::vector<std::string>& region_ids);
};

} // namespace CurvEngine::surface
