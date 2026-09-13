#pragma once

#include "curv/interfaces/IDomainPack.hpp"
#include "curv/temporal/GazeTypes.hpp"
#include "curv/temporal/OculomotorClassifier.hpp"
#include <nlohmann/json.hpp>
#include <string>
#include <vector>

namespace CurvEngine::domains {

/**
 * @brief Clinical configuration and trial parameters for a ResearchKit / CareKit active task.
 */
struct AsdSessionConfig {
    double stimulus_duration_ms{30000.0};
    double min_valid_ratio{0.60}; // Minimum valid gaze sample ratio required to prevent missing-data bias
    std::string stimulus_id{"stimulus_01"};
    std::string step_identifier{"social_attention_active_step"};
};

/**
 * @brief Complete suite of quantitative social visual engagement biomarkers.
 */
struct AsdBiomarkers {
    double eye_dwell_ratio{0.0};
    double mouth_dwell_ratio{0.0};
    double face_dwell_ratio{0.0};
    double social_dwell_ratio{0.0};
    double object_dwell_ratio{0.0};
    double background_dwell_ratio{0.0};
    double social_preference_score{0.0}; // GeoPref: (social - non_social) / (social + non_social)

    double first_eye_latency_ms{-1.0};
    double first_social_latency_ms{-1.0};

    double mean_fixation_duration_ms{0.0};
    double median_fixation_duration_ms{0.0};
    int fixation_count{0};

    int saccade_count{0};
    double mean_saccade_amplitude{0.0};

    double transition_entropy{0.0};
    double self_transition_ratio{0.0};
    double repeat_bigram_ratio{0.0};
    double recurrence_rate{0.0};
};

/**
 * @brief Full clinical session assessment and exportable Apple Health / ResearchKit results.
 */
struct AsdSessionReport {
    bool is_valid{false};
    double valid_sample_ratio{0.0};
    AsdBiomarkers biomarkers;
    std::string scanpath_aoi_token;
    std::string quality_verdict; // "optimal", "acceptable", "low_quality_reject"

    [[nodiscard]] nlohmann::json toResearchKitJson() const;
    [[nodiscard]] nlohmann::json toCareKitJson() const;
};

/**
 * @brief Domain pack for pediatric social attention screening, GeoPref measurement,
 * and ResearchKit / CareKit active task integration.
 */
class AsdBiomarkerPack final : public IDomainPack {
public:
    AsdBiomarkerPack() = default;
    ~AsdBiomarkerPack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.asd_screening"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;

    /**
     * @brief Evaluates an entire gaze session recorded during stimulus playback.
     */
    [[nodiscard]] static AsdSessionReport evaluateSession(const std::vector<temporal::GazeSample>& samples,
                                                          const std::vector<temporal::AreaOfInterest>& aois,
                                                          const AsdSessionConfig& config = {});

    /**
     * @brief Computes Shannon entropy of first-order Markov transitions between AOIs.
     */
    [[nodiscard]] static double computeTransitionEntropy(const std::vector<std::string>& aoi_sequence);

    /**
     * @brief Computes the ratio of repeated adjacent bigram pairs in the AOI sequence.
     */
    [[nodiscard]] static double computeRepeatBigramRatio(const std::vector<std::string>& aoi_sequence);

    /**
     * @brief Computes recurrence rate (RQA) measuring how frequently fixations return
     * to previously inspected spatial locations.
     */
    [[nodiscard]] static double computeRecurrenceRate(const std::vector<temporal::Fixation>& fixations,
                                                      double spatial_threshold_radius = 50.0);
};

} // namespace CurvEngine::domains
