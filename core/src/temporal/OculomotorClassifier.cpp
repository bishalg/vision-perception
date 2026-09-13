#include "curv/temporal/OculomotorClassifier.hpp"
#include <algorithm>
#include <cmath>
#include <numeric>

namespace CurvEngine::temporal {

Scanpath OculomotorClassifier::classifyIvt(const std::vector<GazeSample>& samples, const IvtConfig& config) {
    Scanpath result;
    if (samples.size() < 2) {
        return result;
    }

    result.total_duration_ms = samples.back().timestamp_ms - samples.front().timestamp_ms;

    // 1. Calculate point-to-point velocities
    std::vector<bool> is_fixation_point(samples.size(), false);
    is_fixation_point[0] = true;

    for (size_t i = 1; i < samples.size(); ++i) {
        const double dt_sec = (samples[i].timestamp_ms - samples[i - 1].timestamp_ms) / 1000.0;
        if (dt_sec <= 1e-6) {
            is_fixation_point[i] = is_fixation_point[i - 1];
            continue;
        }

        const double dx = samples[i].x - samples[i - 1].x;
        const double dy = samples[i].y - samples[i - 1].y;
        const double dist = std::hypot(dx, dy);
        const double vel = dist / dt_sec;

        is_fixation_point[i] = (vel < config.velocity_threshold);
    }

    // 2. Aggregate contiguous fixation clusters
    size_t cluster_start = 0;
    bool in_cluster = false;

    for (size_t i = 0; i < samples.size(); ++i) {
        if (is_fixation_point[i]) {
            if (!in_cluster) {
                cluster_start = i;
                in_cluster = true;
            }
        } else {
            if (in_cluster) {
                // End of cluster
                const double duration = samples[i - 1].timestamp_ms - samples[cluster_start].timestamp_ms;
                if (duration >= config.min_fixation_duration_ms) {
                    Fixation fix;
                    fix.start_time_ms = samples[cluster_start].timestamp_ms;
                    fix.duration_ms = duration;
                    fix.sample_count = static_cast<int>(i - cluster_start);

                    double sum_x = 0.0;
                    double sum_y = 0.0;
                    for (size_t k = cluster_start; k < i; ++k) {
                        sum_x += samples[k].x;
                        sum_y += samples[k].y;
                    }
                    fix.x = sum_x / static_cast<double>(fix.sample_count);
                    fix.y = sum_y / static_cast<double>(fix.sample_count);

                    double max_disp = 0.0;
                    for (size_t k = cluster_start; k < i; ++k) {
                        max_disp = std::max(max_disp, std::hypot(samples[k].x - fix.x, samples[k].y - fix.y));
                    }
                    fix.dispersion_radius = max_disp;
                    result.fixations.push_back(fix);
                }
                in_cluster = false;
            }
        }
    }

    // Final cluster at stream boundary
    if (in_cluster) {
        const double duration = samples.back().timestamp_ms - samples[cluster_start].timestamp_ms;
        if (duration >= config.min_fixation_duration_ms) {
            Fixation fix;
            fix.start_time_ms = samples[cluster_start].timestamp_ms;
            fix.duration_ms = duration;
            fix.sample_count = static_cast<int>(samples.size() - cluster_start);

            double sum_x = 0.0;
            double sum_y = 0.0;
            for (size_t k = cluster_start; k < samples.size(); ++k) {
                sum_x += samples[k].x;
                sum_y += samples[k].y;
            }
            fix.x = sum_x / static_cast<double>(fix.sample_count);
            fix.y = sum_y / static_cast<double>(fix.sample_count);

            double max_disp = 0.0;
            for (size_t k = cluster_start; k < samples.size(); ++k) {
                max_disp = std::max(max_disp, std::hypot(samples[k].x - fix.x, samples[k].y - fix.y));
            }
            fix.dispersion_radius = max_disp;
            result.fixations.push_back(fix);
        }
    }

    // 3. Connect fixations with Saccades
    for (size_t j = 1; j < result.fixations.size(); ++j) {
        const auto& prev = result.fixations[j - 1];
        const auto& curr = result.fixations[j];

        Saccade sac;
        sac.start_x = prev.x;
        sac.start_y = prev.y;
        sac.end_x = curr.x;
        sac.end_y = curr.y;
        sac.start_time_ms = prev.start_time_ms + prev.duration_ms;
        sac.duration_ms = std::max(0.0, curr.start_time_ms - sac.start_time_ms);
        sac.amplitude = std::hypot(sac.end_x - sac.start_x, sac.end_y - sac.start_y);

        const double dt_sec = sac.duration_ms / 1000.0;
        sac.peak_velocity = (dt_sec > 1e-4) ? (sac.amplitude / dt_sec) : 0.0;

        result.saccades.push_back(sac);
    }

    return result;
}

Scanpath OculomotorClassifier::classifyIdt(const std::vector<GazeSample>& samples, const IdtConfig& config) {
    Scanpath result;
    if (samples.size() < 2) {
        return result;
    }

    result.total_duration_ms = samples.back().timestamp_ms - samples.front().timestamp_ms;

    size_t start_idx = 0;
    const size_t n = samples.size();

    while (start_idx < n) {
        // Find end index for temporal window
        size_t end_idx = start_idx;
        while (end_idx < n &&
               (samples[end_idx].timestamp_ms - samples[start_idx].timestamp_ms) < config.window_duration_ms) {
            end_idx++;
        }

        if (end_idx >= n) {
            break;
        }

        // Compute dispersion in initial window [start_idx, end_idx]
        double min_x = samples[start_idx].x;
        double max_x = samples[start_idx].x;
        double min_y = samples[start_idx].y;
        double max_y = samples[start_idx].y;

        for (size_t i = start_idx + 1; i <= end_idx; ++i) {
            min_x = std::min(min_x, samples[i].x);
            max_x = std::max(max_x, samples[i].x);
            min_y = std::min(min_y, samples[i].y);
            max_y = std::max(max_y, samples[i].y);
        }

        double dispersion = (max_x - min_x) + (max_y - min_y);

        if (dispersion <= config.dispersion_threshold) {
            // Expand window as long as dispersion remains under threshold
            while (end_idx + 1 < n) {
                const auto& next_pt = samples[end_idx + 1];
                const double cand_min_x = std::min(min_x, next_pt.x);
                const double cand_max_x = std::max(max_x, next_pt.x);
                const double cand_min_y = std::min(min_y, next_pt.y);
                const double cand_max_y = std::max(max_y, next_pt.y);
                const double cand_disp = (cand_max_x - cand_min_x) + (cand_max_y - cand_min_y);

                if (cand_disp <= config.dispersion_threshold) {
                    min_x = cand_min_x;
                    max_x = cand_max_x;
                    min_y = cand_min_y;
                    max_y = cand_max_y;
                    end_idx++;
                } else {
                    break;
                }
            }

            // Record fixation
            Fixation fix;
            fix.start_time_ms = samples[start_idx].timestamp_ms;
            fix.duration_ms = samples[end_idx].timestamp_ms - fix.start_time_ms;
            fix.sample_count = static_cast<int>(end_idx - start_idx + 1);

            double sum_x = 0.0;
            double sum_y = 0.0;
            for (size_t k = start_idx; k <= end_idx; ++k) {
                sum_x += samples[k].x;
                sum_y += samples[k].y;
            }
            fix.x = sum_x / static_cast<double>(fix.sample_count);
            fix.y = sum_y / static_cast<double>(fix.sample_count);
            fix.dispersion_radius = std::hypot(max_x - min_x, max_y - min_y) * 0.5;

            result.fixations.push_back(fix);
            start_idx = end_idx + 1;
        } else {
            // Shift window forward by 1
            start_idx++;
        }
    }

    // Connect fixations with Saccades
    for (size_t j = 1; j < result.fixations.size(); ++j) {
        const auto& prev = result.fixations[j - 1];
        const auto& curr = result.fixations[j];

        Saccade sac;
        sac.start_x = prev.x;
        sac.start_y = prev.y;
        sac.end_x = curr.x;
        sac.end_y = curr.y;
        sac.start_time_ms = prev.start_time_ms + prev.duration_ms;
        sac.duration_ms = std::max(0.0, curr.start_time_ms - sac.start_time_ms);
        sac.amplitude = std::hypot(sac.end_x - sac.start_x, sac.end_y - sac.start_y);

        const double dt_sec = sac.duration_ms / 1000.0;
        sac.peak_velocity = (dt_sec > 1e-4) ? (sac.amplitude / dt_sec) : 0.0;

        result.saccades.push_back(sac);
    }

    return result;
}

std::string OculomotorClassifier::encodeAoiSequence(const Scanpath& scanpath, const std::vector<AreaOfInterest>& aois,
                                                    bool deduplicate_consecutive) {
    std::string sequence;

    for (const auto& fix : scanpath.fixations) {
        std::string matched_id;
        for (const auto& aoi : aois) {
            if (aoi.contains(fix.x, fix.y)) {
                matched_id = aoi.id;
                break;
            }
        }

        if (!matched_id.empty()) {
            if (!deduplicate_consecutive || sequence.empty() || sequence.back() != matched_id.front()) {
                sequence.push_back(matched_id.front());
            }
        }
    }

    return sequence;
}

double OculomotorClassifier::computeOffTargetDwellRatio(const Scanpath& scanpath,
                                                        const AreaOfInterest& primary_target) {
    double total_dwell_ms = 0.0;
    double off_target_dwell_ms = 0.0;

    for (const auto& fix : scanpath.fixations) {
        total_dwell_ms += fix.duration_ms;
        if (!primary_target.contains(fix.x, fix.y)) {
            off_target_dwell_ms += fix.duration_ms;
        }
    }

    return (total_dwell_ms > 0.0) ? (off_target_dwell_ms / total_dwell_ms) : 0.0;
}

} // namespace CurvEngine::temporal
