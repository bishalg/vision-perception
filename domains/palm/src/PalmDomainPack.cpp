#include "curv/domains/PalmDomainPack.hpp"
#include <algorithm>
#include <cmath>
#include <limits>

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
    float net_dx{0.0f};
    float net_dy{0.0f};
    float angle_rad{0.0f};
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

    sa.net_dx = seg.points.back().x - seg.points.front().x;
    sa.net_dy = seg.points.back().y - seg.points.front().y;
    sa.angle_rad = std::atan2(std::abs(sa.net_dy), std::abs(sa.net_dx));
    return sa;
}

} // anonymous namespace

DomainOutput PalmDomainPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    if (input.ridges.segments.empty()) {
        output.domain_payload = {
            {"status", "no_segments_detected"},
            {"heart_line", nullptr},
            {"head_line", nullptr},
            {"life_line", nullptr},
            {"intersections", 0},
            {"continuity_breaks", 0}
        };
        return output;
    }

    // 1. Compute global bounding box across all segments
    float global_min_y = std::numeric_limits<float>::max();
    float global_max_y = -std::numeric_limits<float>::max();

    std::vector<SegmentAnalysis> analyses;
    analyses.reserve(input.ridges.segments.size());

    for (const auto& seg : input.ridges.segments) {
        if (seg.points.size() < 2) continue;
        auto sa = analyzeSegment(seg);
        global_min_y = std::min(global_min_y, sa.min_y);
        global_max_y = std::max(global_max_y, sa.max_y);
        analyses.push_back(sa);
    }

    const float palm_height = std::max(10.0f, global_max_y - global_min_y);

    // 2. Sort candidates by length (longest segments are primary creases)
    std::sort(analyses.begin(), analyses.end(), [](const SegmentAnalysis& a, const SegmentAnalysis& b) {
        return a.seg->total_length > b.seg->total_length;
    });

    int heart_line_id = -1;
    int head_line_id = -1;
    int life_line_id = -1;

    for (const auto& sa : analyses) {
        const float rel_y = (sa.mean_y - global_min_y) / palm_height;

        // Top third, primarily transverse/horizontal -> Heart line
        if (heart_line_id == -1 && rel_y <= 0.42f && sa.angle_rad < 0.70f) {
            heart_line_id = sa.seg->id;
            output.domain_evidence.emplace_back(
                "crease_heart",
                "heart_line",
                0.90,
                nlohmann::json{{"segment_id", sa.seg->id}, {"length", sa.seg->total_length}},
                nlohmann::json{{"mean_y_rel", rel_y}}
            );
        }
        // Middle vertical tier, diagonal transverse -> Head line
        else if (head_line_id == -1 && rel_y > 0.30f && rel_y <= 0.68f) {
            head_line_id = sa.seg->id;
            output.domain_evidence.emplace_back(
                "crease_head",
                "head_line",
                0.88,
                nlohmann::json{{"segment_id", sa.seg->id}, {"length", sa.seg->total_length}},
                nlohmann::json{{"mean_y_rel", rel_y}}
            );
        }
        // Longitudinal curvature running down towards wrist -> Life line
        else if (life_line_id == -1 && (rel_y > 0.45f || sa.angle_rad >= 0.70f)) {
            life_line_id = sa.seg->id;
            output.domain_evidence.emplace_back(
                "crease_life",
                "life_line",
                0.85,
                nlohmann::json{{"segment_id", sa.seg->id}, {"length", sa.seg->total_length}},
                nlohmann::json{{"mean_y_rel", rel_y}}
            );
        }
    }

    // 3. Structural Features: Count junctions / intersections and continuity breaks
    const int intersections_count = static_cast<int>(input.ridges.junctions.size());

    // Continuity breaks: check gaps between nearby segment endpoints (< 15 px)
    int continuity_breaks = 0;
    for (size_t i = 0; i < analyses.size(); ++i) {
        for (size_t j = i + 1; j < analyses.size(); ++j) {
            const auto& pt_a = analyses[i].seg->points.back();
            const auto& pt_b = analyses[j].seg->points.front();
            const float dist = std::hypot(pt_a.x - pt_b.x, pt_a.y - pt_b.y);
            if (dist > 2.0f && dist < 15.0f) {
                continuity_breaks++;
            }
        }
    }

    output.domain_payload = {
        {"status", "analyzed"},
        {"palm_height_px", palm_height},
        {"heart_line_id", (heart_line_id != -1) ? nlohmann::json(heart_line_id) : nullptr},
        {"head_line_id", (head_line_id != -1) ? nlohmann::json(head_line_id) : nullptr},
        {"life_line_id", (life_line_id != -1) ? nlohmann::json(life_line_id) : nullptr},
        {"intersections", intersections_count},
        {"continuity_breaks", continuity_breaks},
        {"total_segments_analyzed", input.ridges.segments.size()}
    };

    return output;
}

} // namespace CurvEngine::domains
