#include "curv/domains/PalmDomainPack.hpp"
#include "curv/domain/CreaseChainLinker.hpp"
#include "curv/domain/CreaseGeometry.hpp"
#include "curv/domain/FateLineAnalyzer.hpp"
#include "curv/domain/PalmZones.hpp"
#include <algorithm>
#include <cmath>
#include <limits>
#include <unordered_map>

namespace CurvEngine::domains {

namespace {

struct SegmentAnalysis {
    const CurveSegment* seg{nullptr};
    float min_x{std::numeric_limits<float>::max()};
    float max_x{-std::numeric_limits<float>::max()};
    float min_y{std::numeric_limits<float>::max()};
    float max_y{-std::numeric_limits<float>::max()};
    float mean_x{0.0f};
    float mean_y{0.0f};
};

SegmentAnalysis analyzeSegment(const CurveSegment& seg) {
    SegmentAnalysis sa;
    sa.seg = &seg;
    if (seg.points.empty()) return sa;

    float sum_x = 0.0f;
    float sum_y = 0.0f;
    for (const auto& pt : seg.points) {
        sa.min_x = std::min(sa.min_x, pt.x);
        sa.max_x = std::max(sa.max_x, pt.x);
        sa.min_y = std::min(sa.min_y, pt.y);
        sa.max_y = std::max(sa.max_y, pt.y);
        sum_x += pt.x;
        sum_y += pt.y;
    }
    const float n = static_cast<float>(seg.points.size());
    sa.mean_x = sum_x / n;
    sa.mean_y = sum_y / n;
    return sa;
}

} // anonymous namespace

DomainOutput PalmDomainPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    if (input.ridges.segments.empty()) {
        output.domain_payload = {{"status", "no_segments_detected"},
                                 {"heart_line", nullptr},
                                 {"head_line", nullptr},
                                 {"life_line", nullptr},
                                 {"heart_line_id", nullptr},
                                 {"head_line_id", nullptr},
                                 {"life_line_id", nullptr},
                                 {"fate_line_id", nullptr},
                                 {"major_lines", nlohmann::json::object()},
                                 {"palm_analysis", nlohmann::json::object()},
                                 {"intersections", 0},
                                 {"continuity_breaks", 0}};
        return output;
    }

    // 1. Compute global bounding box across all segments
    float global_min_y = std::numeric_limits<float>::max();
    float global_max_y = -std::numeric_limits<float>::max();

    std::vector<SegmentAnalysis> analyses;
    analyses.reserve(input.ridges.segments.size());

    for (const auto& seg : input.ridges.segments) {
        if (seg.points.size() < 2) continue;
        auto const sa = analyzeSegment(seg);
        global_min_y = std::min(global_min_y, sa.min_y);
        global_max_y = std::max(global_max_y, sa.max_y);
        analyses.push_back(sa);
    }

    const float palm_height = std::max(10.0f, global_max_y - global_min_y);

    bool is_left_hand = false;
    if (input.frame.metadata.contains("is_left_hand") && input.frame.metadata.at("is_left_hand") == "true") {
        is_left_hand = true;
    }
    const int zone_dim = (input.frame.width() > 0) ? input.frame.width() : 512;
    const auto palm_zones = PalmZones::getCanonicalPalmZones(zone_dim, is_left_hand);

    // Count zone distribution across segments
    std::unordered_map<std::string, int> zone_counts;
    for (const auto& sa : analyses) {
        const std::string primary_zone = geometry::ZoneIntersector::findPrimaryZone(*sa.seg, palm_zones);
        zone_counts[primary_zone]++;
    }

    // 2. Authoritative Major Line Extraction via CreaseChainLinker
    CurvEngine::domain::MajorLineLandmarks priors;
    if (input.landmarks != nullptr) {
        priors.landmarks = *input.landmarks;
        priors.valid = true;
    }
    const auto major_creases =
        CurvEngine::domain::CreaseChainLinker::extractMajorLines(input.ridges, is_left_hand, zone_dim, priors);

    const std::vector<std::pair<std::string, std::string>> line_defs = {{"heart_line", "crease_heart"},
                                                                        {"head_line", "crease_head"},
                                                                        {"life_line", "crease_life"},
                                                                        {"fate_line", "crease_fate"}};

    nlohmann::json major_lines_json = nlohmann::json::object();
    int total_gaps = 0;
    float max_verified_arc = 0.0f;
    std::string dominant_line = "none";
    std::vector<float> verified_tortuosities;

    for (const auto& [name, ev_type] : line_defs) {
        auto it = major_creases.find(name);
        const bool found = (it != major_creases.end() && it->second.is_valid && it->second.total_length >= 15.0f);

        if (found) {
            const auto& chain = it->second;
            const auto metrics = CurvEngine::domain::CreaseGeometry::computeMetrics(chain, input.ridges, palm_zones);

            float conf = 0.85f;
            if (metrics.arc_length_px > 100.0f) conf += 0.05f;
            if (metrics.start_zone != "unknown") conf += 0.03f;
            conf = std::min(0.98f, conf);

            total_gaps += metrics.gap_count;
            verified_tortuosities.push_back(metrics.tortuosity);

            if (metrics.arc_length_px > max_verified_arc) {
                max_verified_arc = metrics.arc_length_px;
                dominant_line = name;
            }

            nlohmann::json points_canonical = nlohmann::json::array();
            nlohmann::json points_norm = nlohmann::json::array();
            const float inv_dim = 1.0f / static_cast<float>(std::max(1, zone_dim));
            // Subsample for mobile-friendly payloads while keeping shape.
            const size_t step = std::max<size_t>(1, chain.points.size() / 64);
            for (size_t i = 0; i < chain.points.size(); i += step) {
                const auto& pt = chain.points[i];
                points_canonical.push_back({pt.x, pt.y});
                points_norm.push_back({pt.x * inv_dim, pt.y * inv_dim});
            }
            if (!chain.points.empty() && (chain.points.size() - 1) % step != 0) {
                const auto& pt = chain.points.back();
                points_canonical.push_back({pt.x, pt.y});
                points_norm.push_back({pt.x * inv_dim, pt.y * inv_dim});
            }

            if (chain.confidence > 0.0f) {
                conf = std::min(0.98f, std::max(conf, chain.confidence));
            }

            nlohmann::json line_obj = {{"name", name},
                                       {"present", true},
                                       {"confidence", conf},
                                       {"segment_ids", chain.constituent_segment_ids},
                                       {"arc_length_px", metrics.arc_length_px},
                                       {"chord_length_px", metrics.chord_length_px},
                                       {"tortuosity", metrics.tortuosity},
                                       {"mean_angle_deg", metrics.mean_angle_deg},
                                       {"start_zone", metrics.start_zone},
                                       {"end_zone", metrics.end_zone},
                                       {"gap_count", metrics.gap_count},
                                       {"fork_count", metrics.fork_count},
                                       {"mean_width_px", metrics.mean_width_px},
                                       {"mean_intensity", metrics.mean_intensity},
                                       {"points_canonical_px", points_canonical},
                                       {"points_canonical_norm", points_norm},
                                       {"points_image_norm", nlohmann::json::array()}};
            major_lines_json[name] = line_obj;

            // Emit structured domain evidence
            output.domain_evidence.emplace_back(
                ev_type, name, conf,
                nlohmann::json{{"segment_ids", chain.constituent_segment_ids},
                               {"arc_length_px", metrics.arc_length_px},
                               {"chord_length_px", metrics.chord_length_px},
                               {"tortuosity", metrics.tortuosity},
                               {"start_zone", metrics.start_zone},
                               {"end_zone", metrics.end_zone},
                               {"gap_count", metrics.gap_count}},
                nlohmann::json{{"mean_angle_deg", metrics.mean_angle_deg}, {"fork_count", metrics.fork_count}});
        } else {
            major_lines_json[name] = {{"name", name},
                                      {"present", false},
                                      {"confidence", 0.0f},
                                      {"segment_ids", nlohmann::json::array()},
                                      {"arc_length_px", 0.0f},
                                      {"chord_length_px", 0.0f},
                                      {"tortuosity", 1.0f},
                                      {"mean_angle_deg", 0.0f},
                                      {"start_zone", "unknown"},
                                      {"end_zone", "unknown"},
                                      {"gap_count", 0},
                                      {"fork_count", 0},
                                      {"mean_width_px", 0.0f},
                                      {"mean_intensity", 0.0f},
                                      {"points_canonical_px", nlohmann::json::array()},
                                      {"points_canonical_norm", nlohmann::json::array()},
                                      {"points_image_norm", nlohmann::json::array()}};
        }
    }

    // 3. Cross-line relationships
    float heart_head_dist = -1.0f;
    if (major_creases.contains("heart_line") && major_creases.at("heart_line").is_valid &&
        major_creases.contains("head_line") && major_creases.at("head_line").is_valid) {
        heart_head_dist = CurvEngine::domain::CreaseGeometry::computeMinDistance(major_creases.at("heart_line").points,
                                                                                 major_creases.at("head_line").points);
    }

    int named_line_intersections = 0;
    const std::vector<std::string> names = {"heart_line", "head_line", "life_line", "fate_line"};
    for (size_t i = 0; i < names.size(); ++i) {
        for (size_t j = i + 1; j < names.size(); ++j) {
            if (major_creases.contains(names[i]) && major_creases.at(names[i]).is_valid &&
                major_creases.contains(names[j]) && major_creases.at(names[j]).is_valid) {
                named_line_intersections += CurvEngine::domain::CreaseGeometry::countIntersections(
                    major_creases.at(names[i]).points, major_creases.at(names[j]).points);
            }
        }
    }

    std::string palm_type_proxy = "balanced";
    if (!verified_tortuosities.empty()) {
        float sum_tort = 0.0f;
        for (float t : verified_tortuosities)
            sum_tort += t;
        const float mean_tort = sum_tort / static_cast<float>(verified_tortuosities.size());
        if (mean_tort > 1.20f) {
            palm_type_proxy = "curved";
        } else if (mean_tort < 1.08f) {
            palm_type_proxy = "straight";
        }
    }

    nlohmann::json palm_analysis = {
        {"lines", major_lines_json},
        {"heart_head_min_distance_px", (heart_head_dist >= 0.0f) ? nlohmann::json(heart_head_dist) : nullptr},
        {"named_line_intersections", named_line_intersections},
        {"dominant_line", dominant_line},
        {"palm_type_proxy", palm_type_proxy}};

    // Backward-compatible primary segment IDs
    auto get_primary_id = [&](const std::string& line_name) -> nlohmann::json {
        if (major_lines_json.contains(line_name) && major_lines_json[line_name]["present"].get<bool>()) {
            const auto& segs = major_lines_json[line_name]["segment_ids"];
            if (!segs.empty()) {
                return segs.front();
            }
        }
        return nullptr;
    };

    const int total_junctions = static_cast<int>(input.ridges.junctions.size());

    // Detailed fate analysis (optional line; absent is valid no_fate).
    const auto fate_it = major_creases.find("fate_line");
    const auto heart_it = major_creases.find("heart_line");
    const auto head_it = major_creases.find("head_line");
    const auto life_it = major_creases.find("life_line");
    const CurvEngine::domain::ChainedCreaseLine empty_line;
    const auto& fate_line = (fate_it != major_creases.end()) ? fate_it->second : empty_line;
    const auto* heart_ptr =
        (heart_it != major_creases.end() && heart_it->second.is_valid) ? &heart_it->second : nullptr;
    const auto* head_ptr = (head_it != major_creases.end() && head_it->second.is_valid) ? &head_it->second : nullptr;
    const auto* life_ptr = (life_it != major_creases.end() && life_it->second.is_valid) ? &life_it->second : nullptr;
    const auto fate_report =
        CurvEngine::domain::FateLineAnalyzer::analyze(fate_line, heart_ptr, head_ptr, life_ptr, input.ridges, zone_dim);

    output.domain_payload = {{"status", "analyzed"},
                             {"palm_height_px", palm_height},
                             {"is_left_hand", is_left_hand},
                             {"heart_line_id", get_primary_id("heart_line")},
                             {"head_line_id", get_primary_id("head_line")},
                             {"life_line_id", get_primary_id("life_line")},
                             {"fate_line_id", get_primary_id("fate_line")},
                             {"major_lines", major_lines_json},
                             {"palm_analysis", palm_analysis},
                             {"fate_analysis", fate_report.toJson()},
                             {"intersections", total_junctions},
                             {"continuity_breaks", total_gaps},
                             {"zone_distribution", zone_counts},
                             {"total_segments_analyzed", input.ridges.segments.size()}};

    return output;
}

} // namespace CurvEngine::domains
