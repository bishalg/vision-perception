#pragma once

#include "curv/interfaces/IDomainPack.hpp"
#include "curv/temporal/GazeTypes.hpp"
#include "curv/temporal/OculomotorClassifier.hpp"
#include <string>
#include <vector>

namespace CurvEngine::domains {

/**
 * @brief Analysis result of driver visual attention and oculomotor scanpath dynamics.
 */
struct DriverAttentionReport {
    double off_road_dwell_ratio{0.0};
    int critical_off_road_glances{0}; // Count of off-road fixations > 2.0 sec (NHTSA threshold)
    double mean_fixation_duration_ms{0.0};
    int total_fixations{0};
    int total_saccades{0};
    std::string attention_state; // "alert_scanning", "minor_distraction", "critical_distraction", "drowsy_stare"
    std::string aoi_scanpath_token;
};

/**
 * @brief Domain pack for automotive in-cabin driver monitoring, gaze distraction,
 * and oculomotor fatigue analysis.
 */
class DriverGazePack final : public IDomainPack {
public:
    DriverGazePack() = default;
    ~DriverGazePack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.driver_monitoring"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;

    /**
     * @brief Performs temporal scanpath analysis on a stream of gaze samples.
     * @param samples Stream of (x, y, timestamp_ms) samples.
     * @param forward_windshield_aoi Primary on-road visual zone.
     * @param cabin_aois Secondary zones (mirrors, instrument cluster, infotainment).
     */
    [[nodiscard]] static DriverAttentionReport
    evaluateVisualAttention(const std::vector<temporal::GazeSample>& samples,
                            const temporal::AreaOfInterest& forward_windshield_aoi,
                            const std::vector<temporal::AreaOfInterest>& cabin_aois = {});
};

} // namespace CurvEngine::domains
