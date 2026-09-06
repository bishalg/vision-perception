#include "curv/domains/GeospatialFloodPack.hpp"

#include <algorithm>
#include <cmath>

namespace CurvEngine::domains {

GeospatialFloodPack::GeospatialFloodPack(Config cfg) : cfg_(cfg) {}

DomainOutput GeospatialFloodPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    float max_length = 0.0f;
    float sum_length = 0.0f;
    int river_count = 0;
    int tributary_count = 0;
    int minor_count = 0;

    for (const auto& seg : input.ridges.segments) {
        max_length = std::max(max_length, seg.total_length);
        sum_length += seg.total_length;

        std::string feature_class;
        double feature_confidence = 0.60;

        if (seg.total_length >= cfg_.river_min_length_px) {
            feature_class = "river_channel";
            feature_confidence = 0.95;
            river_count++;
        } else if (seg.total_length >= cfg_.tributary_min_length_px) {
            feature_class = "tributary";
            feature_confidence = 0.80;
            tributary_count++;
        } else {
            feature_class = "minor_feature";
            feature_confidence = 0.50;
            minor_count++;
        }

        const nlohmann::json geom = {
            {"segment_id", seg.id}, {"point_count", seg.points.size()}, {"total_length", seg.total_length}};

        const nlohmann::json meta = {{"feature_class", feature_class}, {"confidence", feature_confidence}};

        output.domain_evidence.emplace_back("geo_feature_" + std::to_string(seg.id), feature_class, feature_confidence,
                                            geom, meta);
    }

    const int total_features = river_count + tributary_count + minor_count;
    const double mean_length =
        (total_features > 0) ? (static_cast<double>(sum_length) / static_cast<double>(total_features)) : 0.0;

    output.domain_payload = {
        {"total_features", total_features},       {"river_channel_count", river_count},
        {"tributary_count", tributary_count},     {"minor_feature_count", minor_count},
        {"max_feature_length_px", max_length},    {"mean_feature_length_px", mean_length},
        {"total_feature_length_px", sum_length},  {"junction_count", input.ridges.junctions.size()},
        {"frame_usable", input.quality.is_usable}};

    return output;
}

} // namespace CurvEngine::domains
