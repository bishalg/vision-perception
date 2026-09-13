#include "curv/domains/DriverGazePack.hpp"
#include <algorithm>
#include <numeric>

namespace CurvEngine::domains {

DriverAttentionReport DriverGazePack::evaluateVisualAttention(const std::vector<temporal::GazeSample>& samples,
                                                              const temporal::AreaOfInterest& forward_windshield_aoi,
                                                              const std::vector<temporal::AreaOfInterest>& cabin_aois) {
    DriverAttentionReport report;
    if (samples.empty()) {
        report.attention_state = "no_data";
        return report;
    }

    const auto scanpath = temporal::OculomotorClassifier::classifyIvt(samples);
    report.total_fixations = static_cast<int>(scanpath.fixations.size());
    report.total_saccades = static_cast<int>(scanpath.saccades.size());

    if (scanpath.fixations.empty()) {
        report.attention_state = "insufficient_fixations";
        return report;
    }

    report.off_road_dwell_ratio =
        temporal::OculomotorClassifier::computeOffTargetDwellRatio(scanpath, forward_windshield_aoi);

    double total_fixation_dur = 0.0;
    for (const auto& fix : scanpath.fixations) {
        total_fixation_dur += fix.duration_ms;
        if (!forward_windshield_aoi.contains(fix.x, fix.y) && fix.duration_ms >= 2000.0) {
            report.critical_off_road_glances++;
        }
    }

    report.mean_fixation_duration_ms = total_fixation_dur / static_cast<double>(scanpath.fixations.size());

    // Classification according to NHTSA / Euro NCAP safety heuristics
    if (report.critical_off_road_glances > 0 || report.off_road_dwell_ratio >= 0.40) {
        report.attention_state = "critical_distraction";
    } else if (report.off_road_dwell_ratio >= 0.20) {
        report.attention_state = "minor_distraction";
    } else if (report.mean_fixation_duration_ms >= 1500.0) {
        report.attention_state = "drowsy_stare";
    } else {
        report.attention_state = "alert_scanning";
    }

    // Symbolic AOI scanpath string
    std::vector<temporal::AreaOfInterest> all_aois;
    all_aois.push_back(forward_windshield_aoi);
    all_aois.insert(all_aois.end(), cabin_aois.begin(), cabin_aois.end());
    report.aoi_scanpath_token = temporal::OculomotorClassifier::encodeAoiSequence(scanpath, all_aois, true);

    return report;
}

DomainOutput DriverGazePack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    output.domain_payload = {{"domain", name()},
                             {"version", version()},
                             {"frame_usable", input.quality.is_usable},
                             {"detected_features_count", input.ridges.segments.size()}};

    const nlohmann::json geom = {{"feature_count", input.ridges.segments.size()}};
    const double quality_metric = input.quality.is_usable ? 1.0 : 0.0;
    const nlohmann::json meta = {{"is_usable", input.quality.is_usable}, {"blur_score", input.quality.blur_score}};

    output.domain_evidence.emplace_back("driver_monitoring_frame", "cabin_inspection", quality_metric, geom, meta);

    return output;
}

} // namespace CurvEngine::domains
