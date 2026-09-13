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
 */
class MountReliefAnalyzer {
public:
    [[nodiscard]] static MountReliefMap analyzeOrbitSession(const nlohmann::json& orbit_session_json);
};

} // namespace CurvEngine::domain
