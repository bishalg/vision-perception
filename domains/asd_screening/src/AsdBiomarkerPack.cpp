#include "curv/domains/AsdBiomarkerPack.hpp"
#include <algorithm>
#include <cmath>
#include <map>
#include <numeric>
#include <unordered_map>

namespace CurvEngine::domains {

nlohmann::json AsdSessionReport::toResearchKitJson() const {
    return {{"identifier", "ORKActiveStepResult"},
            {"type", "social_attention_gaze_result"},
            {"is_valid", is_valid},
            {"valid_sample_ratio", valid_sample_ratio},
            {"quality_verdict", quality_verdict},
            {"scanpath_aoi_token", scanpath_aoi_token},
            {"biomarkers",
             {{"eye_dwell_ratio", biomarkers.eye_dwell_ratio},
              {"mouth_dwell_ratio", biomarkers.mouth_dwell_ratio},
              {"face_dwell_ratio", biomarkers.face_dwell_ratio},
              {"social_dwell_ratio", biomarkers.social_dwell_ratio},
              {"object_dwell_ratio", biomarkers.object_dwell_ratio},
              {"background_dwell_ratio", biomarkers.background_dwell_ratio},
              {"social_preference_score", biomarkers.social_preference_score},
              {"first_eye_latency_ms", biomarkers.first_eye_latency_ms},
              {"first_social_latency_ms", biomarkers.first_social_latency_ms},
              {"mean_fixation_duration_ms", biomarkers.mean_fixation_duration_ms},
              {"median_fixation_duration_ms", biomarkers.median_fixation_duration_ms},
              {"fixation_count", biomarkers.fixation_count},
              {"saccade_count", biomarkers.saccade_count},
              {"mean_saccade_amplitude", biomarkers.mean_saccade_amplitude},
              {"transition_entropy", biomarkers.transition_entropy},
              {"self_transition_ratio", biomarkers.self_transition_ratio},
              {"repeat_bigram_ratio", biomarkers.repeat_bigram_ratio},
              {"recurrence_rate", biomarkers.recurrence_rate}}}};
}

nlohmann::json AsdSessionReport::toCareKitJson() const {
    return {{"care_plan_element", "OCKCarePlanActivity"},
            {"task_title", "Social Attention Assessment"},
            {"completion_status", is_valid ? "completed" : "incomplete_low_quality"},
            {"summary_metrics",
             {{"social_preference_index", biomarkers.social_preference_score},
              {"eye_contact_ratio", biomarkers.eye_dwell_ratio},
              {"fixation_stability_ms", biomarkers.mean_fixation_duration_ms}}},
            {"notes", is_valid ? "Assessment completed successfully. Ready for clinical review."
                               : "Excessive motion or tracking loss. Re-administration recommended."}};
}

double AsdBiomarkerPack::computeTransitionEntropy(const std::vector<std::string>& aoi_sequence) {
    if (aoi_sequence.size() < 2) {
        return 0.0;
    }

    // First-order Markov transition counts: C[from][to]
    std::map<std::string, std::map<std::string, int>> trans_counts;
    std::map<std::string, int> from_totals;

    for (size_t i = 0; i + 1 < aoi_sequence.size(); ++i) {
        const auto& from = aoi_sequence[i];
        const auto& to = aoi_sequence[i + 1];
        trans_counts[from][to]++;
        from_totals[from]++;
    }

    const double total_transitions = static_cast<double>(aoi_sequence.size() - 1);
    double conditional_entropy = 0.0;

    for (const auto& [from, targets] : trans_counts) {
        const double p_from = static_cast<double>(from_totals[from]) / total_transitions;
        double state_entropy = 0.0;

        for (const auto& [to, count] : targets) {
            const double p_trans = static_cast<double>(count) / static_cast<double>(from_totals[from]);
            if (p_trans > 1e-9) {
                state_entropy -= p_trans * std::log2(p_trans);
            }
        }

        conditional_entropy += p_from * state_entropy;
    }

    return conditional_entropy;
}

double AsdBiomarkerPack::computeRepeatBigramRatio(const std::vector<std::string>& aoi_sequence) {
    if (aoi_sequence.size() < 4) {
        return 0.0;
    }

    // Count consecutive repeat bigrams: (A -> B) followed immediately by (A -> B)
    int repeat_count = 0;
    const size_t bigram_count = aoi_sequence.size() - 1;

    for (size_t i = 0; i + 2 < bigram_count; ++i) {
        if (aoi_sequence[i] == aoi_sequence[i + 2] && aoi_sequence[i + 1] == aoi_sequence[i + 3]) {
            repeat_count++;
        }
    }

    return (bigram_count > 2) ? (static_cast<double>(repeat_count) / static_cast<double>(bigram_count - 2)) : 0.0;
}

double AsdBiomarkerPack::computeRecurrenceRate(const std::vector<temporal::Fixation>& fixations,
                                               double spatial_threshold_radius) {
    const size_t n = fixations.size();
    if (n < 2) {
        return 0.0;
    }

    int recurrence_hits = 0;
    for (size_t i = 0; i < n; ++i) {
        for (size_t j = i + 1; j < n; ++j) {
            const double dx = fixations[i].x - fixations[j].x;
            const double dy = fixations[i].y - fixations[j].y;
            if (std::hypot(dx, dy) <= spatial_threshold_radius) {
                recurrence_hits++;
            }
        }
    }

    const double total_pairs = static_cast<double>(n * (n - 1)) / 2.0;
    return (total_pairs > 0.0) ? (static_cast<double>(recurrence_hits) / total_pairs) : 0.0;
}

AsdSessionReport AsdBiomarkerPack::evaluateSession(const std::vector<temporal::GazeSample>& samples,
                                                   const std::vector<temporal::AreaOfInterest>& aois,
                                                   const AsdSessionConfig& config) {
    AsdSessionReport report;
    if (samples.empty()) {
        report.quality_verdict = "no_data";
        return report;
    }

    // 1. Quality & Validity Gating
    size_t valid_count = 0;
    for (const auto& s : samples) {
        if (s.confidence >= 0.5) {
            valid_count++;
        }
    }
    report.valid_sample_ratio = static_cast<double>(valid_count) / static_cast<double>(samples.size());
    report.is_valid = (report.valid_sample_ratio >= config.min_valid_ratio);

    if (report.valid_sample_ratio >= 0.85) {
        report.quality_verdict = "optimal";
    } else if (report.is_valid) {
        report.quality_verdict = "acceptable";
    } else {
        report.quality_verdict = "low_quality_reject";
    }

    // 2. Parse Fixations & Saccades via I-VT
    temporal::IvtConfig ivt_cfg;
    ivt_cfg.velocity_threshold = 150.0;
    ivt_cfg.min_fixation_duration_ms = 80.0;

    const auto scanpath = temporal::OculomotorClassifier::classifyIvt(samples, ivt_cfg);
    report.biomarkers.fixation_count = static_cast<int>(scanpath.fixations.size());
    report.biomarkers.saccade_count = static_cast<int>(scanpath.saccades.size());

    if (scanpath.fixations.empty()) {
        return report;
    }

    // 3. Saccadic Metrics
    double sum_saccade_amp = 0.0;
    for (const auto& sac : scanpath.saccades) {
        sum_saccade_amp += sac.amplitude;
    }
    report.biomarkers.mean_saccade_amplitude =
        scanpath.saccades.empty() ? 0.0 : (sum_saccade_amp / static_cast<double>(scanpath.saccades.size()));

    // 4. Fixation Durations
    std::vector<double> durations;
    durations.reserve(scanpath.fixations.size());
    double total_dwell_ms = 0.0;

    for (const auto& fix : scanpath.fixations) {
        durations.push_back(fix.duration_ms);
        total_dwell_ms += fix.duration_ms;
    }
    std::sort(durations.begin(), durations.end());

    report.biomarkers.mean_fixation_duration_ms = total_dwell_ms / static_cast<double>(scanpath.fixations.size());
    report.biomarkers.median_fixation_duration_ms = durations[durations.size() / 2];

    // 5. AOI Allocation & Dynamic Scanpath Sequence
    double eye_dwell = 0.0;
    double mouth_dwell = 0.0;
    double face_dwell = 0.0;
    double social_dwell = 0.0;
    double object_dwell = 0.0;
    double background_dwell = 0.0;

    std::vector<std::string> aoi_sequence;
    int self_transition_hits = 0;

    for (const auto& fix : scanpath.fixations) {
        std::string matched_aoi = "background";
        for (const auto& aoi : aois) {
            if (aoi.contains(fix.x, fix.y)) {
                matched_aoi = aoi.id;
                break;
            }
        }

        // Lowercase comparison helper
        std::string lower_id = matched_aoi;
        std::transform(lower_id.begin(), lower_id.end(), lower_id.begin(), ::tolower);

        if (lower_id.find("eye") != std::string::npos) {
            eye_dwell += fix.duration_ms;
            face_dwell += fix.duration_ms;
            social_dwell += fix.duration_ms;
            if (report.biomarkers.first_eye_latency_ms < 0.0) {
                report.biomarkers.first_eye_latency_ms = fix.start_time_ms;
            }
        } else if (lower_id.find("mouth") != std::string::npos) {
            mouth_dwell += fix.duration_ms;
            face_dwell += fix.duration_ms;
            social_dwell += fix.duration_ms;
        } else if (lower_id.find("face") != std::string::npos) {
            face_dwell += fix.duration_ms;
            social_dwell += fix.duration_ms;
        } else if (lower_id.find("social") != std::string::npos) {
            social_dwell += fix.duration_ms;
        } else if (lower_id.find("geo") != std::string::npos || lower_id.find("object") != std::string::npos) {
            object_dwell += fix.duration_ms;
        } else {
            background_dwell += fix.duration_ms;
        }

        if (lower_id.find("social") != std::string::npos || lower_id.find("face") != std::string::npos ||
            lower_id.find("eye") != std::string::npos || lower_id.find("mouth") != std::string::npos) {
            if (report.biomarkers.first_social_latency_ms < 0.0) {
                report.biomarkers.first_social_latency_ms = fix.start_time_ms;
            }
        }

        if (!aoi_sequence.empty() && aoi_sequence.back() == matched_aoi) {
            self_transition_hits++;
        }
        aoi_sequence.push_back(matched_aoi);
    }

    if (total_dwell_ms > 0.0) {
        report.biomarkers.eye_dwell_ratio = eye_dwell / total_dwell_ms;
        report.biomarkers.mouth_dwell_ratio = mouth_dwell / total_dwell_ms;
        report.biomarkers.face_dwell_ratio = face_dwell / total_dwell_ms;
        report.biomarkers.social_dwell_ratio = social_dwell / total_dwell_ms;
        report.biomarkers.object_dwell_ratio = object_dwell / total_dwell_ms;
        report.biomarkers.background_dwell_ratio = background_dwell / total_dwell_ms;
    }

    // GeoPref: (social - nonsocial) / (social + nonsocial)
    const double combined_pref = social_dwell + object_dwell;
    if (combined_pref > 0.0) {
        report.biomarkers.social_preference_score = (social_dwell - object_dwell) / combined_pref;
    }

    // 6. Markov Transitions, Entropy & Repetition
    report.biomarkers.transition_entropy = computeTransitionEntropy(aoi_sequence);
    report.biomarkers.repeat_bigram_ratio = computeRepeatBigramRatio(aoi_sequence);
    report.biomarkers.self_transition_ratio =
        (aoi_sequence.size() > 1)
            ? (static_cast<double>(self_transition_hits) / static_cast<double>(aoi_sequence.size() - 1))
            : 0.0;
    report.biomarkers.recurrence_rate = computeRecurrenceRate(scanpath.fixations, 50.0);

    // 7. Symbolic AOI Token
    report.scanpath_aoi_token = temporal::OculomotorClassifier::encodeAoiSequence(scanpath, aois, true);

    return report;
}

DomainOutput AsdBiomarkerPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    const double quality_metric = input.quality.is_usable ? 1.0 : 0.0;
    const nlohmann::json geom = {{"feature_count", input.ridges.segments.size()}};
    const nlohmann::json meta = {{"is_usable", input.quality.is_usable}, {"blur_score", input.quality.blur_score}};

    output.domain_evidence.emplace_back("asd_screening_frame", "social_attention_frame", quality_metric, geom, meta);

    output.domain_payload = {{"domain", name()}, {"version", version()}, {"frame_usable", input.quality.is_usable}};

    return output;
}

} // namespace CurvEngine::domains
