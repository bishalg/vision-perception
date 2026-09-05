#include "curv/domains/SurfaceInspectionPack.hpp"
#include <algorithm>
#include <cmath>

namespace CurvEngine::domains {

SurfaceInspectionPack::SurfaceInspectionPack(float min_length_px) : min_length_px_(min_length_px) {}

DomainOutput SurfaceInspectionPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    float max_length = 0.0f;
    float sum_length = 0.0f;
    int severe_count = 0;
    int moderate_count = 0;
    int minor_count = 0;

    for (const auto& seg : input.ridges.segments) {
        if (seg.total_length < min_length_px_) {
            continue;
        }

        max_length = std::max(max_length, seg.total_length);
        sum_length += seg.total_length;

        std::string defect_class;
        double defect_severity = 0.5;

        if (seg.total_length > 80.0f) {
            defect_class = "severe_crack";
            defect_severity = 0.95;
            severe_count++;
        } else if (seg.total_length > 45.0f) {
            defect_class = "moderate_crack";
            defect_severity = 0.70;
            moderate_count++;
        } else {
            defect_class = "micro_scratch";
            defect_severity = 0.40;
            minor_count++;
        }

        nlohmann::json geom = {
            {"segment_id", seg.id}, {"point_count", seg.points.size()}, {"total_length", seg.total_length}};

        nlohmann::json meta = {{"defect_class", defect_class}, {"severity", defect_severity}};

        output.domain_evidence.emplace_back("defect_" + std::to_string(seg.id), defect_class, defect_severity, geom,
                                            meta);
    }

    const int total_defects = severe_count + moderate_count + minor_count;
    const double mean_length =
        (total_defects > 0) ? (static_cast<double>(sum_length) / static_cast<double>(total_defects)) : 0.0;

    output.domain_payload = {{"total_defects_found", total_defects},   {"severe_cracks", severe_count},
                             {"moderate_cracks", moderate_count},      {"micro_scratches", minor_count},
                             {"max_defect_length", max_length},        {"mean_defect_length", mean_length},
                             {"frame_usable", input.quality.is_usable}};

    return output;
}

} // namespace CurvEngine::domains
