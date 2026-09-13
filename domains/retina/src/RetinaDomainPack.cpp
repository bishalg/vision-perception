#include "curv/domains/RetinaDomainPack.hpp"
#include <algorithm>
#include <cmath>
#include <numeric>
#include <opencv2/imgproc.hpp>

namespace CurvEngine::domains {

RetinaDomainPack::RetinaDomainPack(float min_vessel_length_px) : min_vessel_length_px_(min_vessel_length_px) {}

cv::Mat RetinaDomainPack::prepareGreenChannel(const cv::Mat& bgr, bool apply_clahe) {
    if (bgr.empty()) {
        return {};
    }

    cv::Mat green;
    if (bgr.channels() >= 3) {
        std::vector<cv::Mat> channels;
        cv::split(bgr, channels);
        green = channels[1]; // Green channel has maximum hemoglobin absorption contrast
    } else {
        green = bgr.clone();
    }

    // Invert so dark vessels become bright curvilinear ridges suitable for Steger extraction
    cv::Mat inverted;
    cv::bitwise_not(green, inverted);

    if (apply_clahe) {
        cv::Mat enhanced;
        auto clahe = cv::createCLAHE(2.5, cv::Size(8, 8));
        clahe->apply(inverted, enhanced);
        return enhanced;
    }

    return inverted;
}

double RetinaDomainPack::computeArcChordTortuosity(const CurveSegment& seg) {
    if (seg.points.size() < 2 || seg.total_length <= 1e-6f) {
        return 1.0;
    }

    const double dx = static_cast<double>(seg.points.back().x - seg.points.front().x);
    const double dy = static_cast<double>(seg.points.back().y - seg.points.front().y);
    const double chord = std::hypot(dx, dy);

    if (chord < 1e-3) {
        return (seg.total_length > 0.0f) ? static_cast<double>(seg.total_length) : 1.0;
    }

    const double tau = static_cast<double>(seg.total_length) / chord;
    return std::max(1.0, tau);
}

DomainOutput RetinaDomainPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    float total_vessel_length = 0.0f;
    double sum_tortuosity = 0.0;
    double sum_width = 0.0;
    int primary_arcade_count = 0;
    int secondary_branch_count = 0;
    int microvascular_count = 0;

    int valid_vessel_count = 0;

    for (const auto& seg : input.ridges.segments) {
        if (seg.total_length < min_vessel_length_px_) {
            continue;
        }

        valid_vessel_count++;
        total_vessel_length += seg.total_length;

        const double tortuosity = computeArcChordTortuosity(seg);
        sum_tortuosity += tortuosity;

        double seg_conf_sum = 0.0;
        for (const auto& pt : seg.points) {
            seg_conf_sum += static_cast<double>(pt.confidence);
        }
        const double avg_confidence =
            seg.points.empty() ? 0.9 : (seg_conf_sum / static_cast<double>(seg.points.size()));

        // Caliber estimation from segment length and point density
        const double avg_width = (seg.total_length >= 60.0f) ? 3.2 : (seg.total_length >= 30.0f ? 2.2 : 1.5);
        sum_width += avg_width;

        std::string vessel_type;
        if (seg.total_length >= 60.0f) {
            vessel_type = "primary_arcade";
            primary_arcade_count++;
        } else if (seg.total_length >= 30.0f) {
            vessel_type = "secondary_branch";
            secondary_branch_count++;
        } else {
            vessel_type = "microvascular_capillary";
            microvascular_count++;
        }

        const nlohmann::json geom = {{"segment_id", seg.id},
                                     {"point_count", seg.points.size()},
                                     {"total_length_px", seg.total_length},
                                     {"mean_width_px", avg_width},
                                     {"tortuosity_index", tortuosity}};

        const nlohmann::json meta = {{"vessel_type", vessel_type}, {"confidence", avg_confidence}};

        output.domain_evidence.emplace_back("vessel_" + std::to_string(seg.id), "retinal_vessel", avg_confidence, geom,
                                            meta);
    }

    // Count bifurcation junctions (degree >= 3)
    int bifurcation_count = 0;
    for (const auto& junc : input.ridges.junctions) {
        if (junc.connected_segment_ids.size() >= 3) {
            bifurcation_count++;
        }
    }

    const double mean_tortuosity =
        (valid_vessel_count > 0) ? (sum_tortuosity / static_cast<double>(valid_vessel_count)) : 1.0;
    const double mean_width = (valid_vessel_count > 0) ? (sum_width / static_cast<double>(valid_vessel_count)) : 0.0;

    const int img_w = input.frame.width();
    const int img_h = input.frame.height();
    const double total_img_area = static_cast<double>(img_w * img_h);
    const double estimated_vessel_area = static_cast<double>(total_vessel_length) * mean_width;
    const double vessel_density_ratio = (total_img_area > 0.0) ? (estimated_vessel_area / total_img_area) : 0.0;

    output.domain_payload = {
        {"total_vessels_detected", valid_vessel_count}, {"primary_arcades", primary_arcade_count},
        {"secondary_branches", secondary_branch_count}, {"microvascular_capillaries", microvascular_count},
        {"bifurcation_count", bifurcation_count},       {"total_vessel_length_px", total_vessel_length},
        {"mean_vessel_width_px", mean_width},           {"mean_tortuosity_index", mean_tortuosity},
        {"vessel_density_ratio", vessel_density_ratio}, {"frame_usable", input.quality.is_usable}};

    return output;
}

} // namespace CurvEngine::domains
