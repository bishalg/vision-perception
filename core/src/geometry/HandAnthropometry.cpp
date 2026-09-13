#include "curv/geometry/HandAnthropometry.hpp"
#include <algorithm>
#include <cmath>

namespace CurvEngine::geometry {

namespace {

float distPx(const std::array<float, 2>& a, const std::array<float, 2>& b, float w, float h) {
    const float dx = (a[0] - b[0]) * w;
    const float dy = (a[1] - b[1]) * h;
    return std::sqrt(dx * dx + dy * dy);
}

float angleDeg(const std::array<float, 2>& a, const std::array<float, 2>& b,
               const std::array<float, 2>& c, const std::array<float, 2>& d,
               float w, float h) {
    const float v1x = (b[0] - a[0]) * w;
    const float v1y = (b[1] - a[1]) * h;
    const float v2x = (d[0] - c[0]) * w;
    const float v2y = (d[1] - c[1]) * h;
    const float len1 = std::sqrt(v1x * v1x + v1y * v1y);
    const float len2 = std::sqrt(v2x * v2x + v2y * v2y);
    if (len1 < 1e-4f || len2 < 1e-4f) return 0.0f;
    const float dot = (v1x * v2x + v1y * v2y) / (len1 * len2);
    const float clamped = std::max(-1.0f, std::min(1.0f, dot));
    return std::acos(clamped) * 180.0f / 3.141592653589793f;
}

} // anonymous namespace

nlohmann::json DigitMeasurement::toJson() const {
    return {
        {"name", name},
        {"total_length_mm", total_length_mm},
        {"proximal_phalanx_mm", proximal_phalanx_mm},
        {"intermediate_phalanx_mm", intermediate_phalanx_mm},
        {"distal_phalanx_mm", distal_phalanx_mm},
        {"length_relative_to_hand", length_relative_to_hand}
    };
}

nlohmann::json DigitRatios::toJson() const {
    return {
        {"ratio_2d_4d", ratio_2d_4d},
        {"ratio_2d_3d", ratio_2d_3d},
        {"ratio_3d_4d", ratio_3d_4d},
        {"ratio_4d_5d", ratio_4d_5d},
        {"thumb_to_index", thumb_to_index}
    };
}

nlohmann::json PalmDimensions::toJson() const {
    return {
        {"metacarpal_breadth_mm", metacarpal_breadth_mm},
        {"palm_length_mm", palm_length_mm},
        {"total_hand_length_mm", total_hand_length_mm},
        {"aspect_ratio", aspect_ratio},
        {"palm_surface_area_cm2", palm_surface_area_cm2},
        {"hand_shape_type", hand_shape_type}
    };
}

nlohmann::json InterdigitalSpan::toJson() const {
    return {
        {"thumb_index_angle_deg", thumb_index_angle_deg},
        {"index_middle_angle_deg", index_middle_angle_deg},
        {"middle_ring_angle_deg", middle_ring_angle_deg},
        {"ring_pinky_angle_deg", ring_pinky_angle_deg},
        {"thumb_index_distance_mm", thumb_index_distance_mm},
        {"index_middle_distance_mm", index_middle_distance_mm},
        {"middle_ring_distance_mm", middle_ring_distance_mm},
        {"ring_pinky_distance_mm", ring_pinky_distance_mm}
    };
}

nlohmann::json AnatomicalPadMetric::toJson() const {
    return {
        {"pad_name", pad_name},
        {"height_mm", height_mm},
        {"relative_prominence", relative_prominence},
        {"area_mm2", area_mm2},
        {"curvature", curvature}
    };
}

nlohmann::json HandAnthropometryReport::toJson() const {
    nlohmann::json digits_arr = nlohmann::json::array();
    for (const auto& d : digits) {
        digits_arr.push_back(d.toJson());
    }
    nlohmann::json pads_arr = nlohmann::json::array();
    for (const auto& p : pads) {
        pads_arr.push_back(p.toJson());
    }
    return {
        {"schema_version", schema_version},
        {"palm", palm.toJson()},
        {"digits", digits_arr},
        {"ratios", ratios.toJson()},
        {"spans", spans.toJson()},
        {"pads", pads_arr},
        {"mm_per_pixel", mm_per_pixel},
        {"hand_side", hand_side}
    };
}

HandAnthropometryReport HandAnthropometer::compute(
    const std::vector<std::array<float, 2>>& lm,
    int image_width,
    int image_height,
    float reference_palm_width_mm,
    const std::unordered_map<std::string, float>& pad_depth_mm,
    const std::string& hand_side
) {
    HandAnthropometryReport report;
    report.hand_side = hand_side;

    if (lm.size() < 21) {
        return report;
    }

    const float w = static_cast<float>(std::max(1, image_width));
    const float h = static_cast<float>(std::max(1, image_height));

    // Metacarpal breadth in pixels (index MCP 5 to pinky MCP 17)
    const float breadth_px = distPx(lm[5], lm[17], w, h);
    if (breadth_px <= 1e-4f) {
        return report;
    }

    const float mm_per_px = reference_palm_width_mm / breadth_px;
    report.mm_per_pixel = mm_per_px;

    // Palm dimensions
    report.palm.metacarpal_breadth_mm = reference_palm_width_mm;
    report.palm.palm_length_mm = distPx(lm[0], lm[9], w, h) * mm_per_px;
    report.palm.total_hand_length_mm = distPx(lm[0], lm[12], w, h) * mm_per_px;

    if (report.palm.palm_length_mm > 1e-3f) {
        report.palm.aspect_ratio = report.palm.metacarpal_breadth_mm / report.palm.palm_length_mm;
    }

    if (report.palm.aspect_ratio > 1.05f) {
        report.palm.hand_shape_type = "broad";
    } else if (report.palm.aspect_ratio < 0.85f) {
        report.palm.hand_shape_type = "elongated";
    } else {
        report.palm.hand_shape_type = "proportional";
    }

    // Estimated palm surface area (cm^2): trapezoid over wrist and MCPs
    const float wrist_width_px = distPx(lm[1], lm[17], w, h) * 0.75f;
    const float area_px2 = 0.5f * (breadth_px + wrist_width_px) * (report.palm.palm_length_mm / mm_per_px);
    report.palm.palm_surface_area_cm2 = (area_px2 * mm_per_px * mm_per_px) / 100.0f;

    // Digits
    struct ChainDef {
        std::string name;
        size_t p0, p1, p2, p3;
    };
    static const ChainDef chains[5] = {
        {"thumb", 1UL, 2UL, 3UL, 4UL},
        {"index", 5UL, 6UL, 7UL, 8UL},
        {"middle", 9UL, 10UL, 11UL, 12UL},
        {"ring", 13UL, 14UL, 15UL, 16UL},
        {"pinky", 17UL, 18UL, 19UL, 20UL}
    };

    float total_hand = std::max(1e-3f, report.palm.total_hand_length_mm);

    for (const auto& c : chains) {
        DigitMeasurement dm;
        dm.name = c.name;
        dm.proximal_phalanx_mm = distPx(lm[c.p0], lm[c.p1], w, h) * mm_per_px;
        dm.intermediate_phalanx_mm = distPx(lm[c.p1], lm[c.p2], w, h) * mm_per_px;
        dm.distal_phalanx_mm = distPx(lm[c.p2], lm[c.p3], w, h) * mm_per_px;
        dm.total_length_mm = dm.proximal_phalanx_mm + dm.intermediate_phalanx_mm + dm.distal_phalanx_mm;
        dm.length_relative_to_hand = dm.total_length_mm / total_hand;
        report.digits.push_back(dm);
    }

    // Digit ratios
    const float len_index = report.digits[1].total_length_mm;
    const float len_middle = report.digits[2].total_length_mm;
    const float len_ring = report.digits[3].total_length_mm;
    const float len_pinky = report.digits[4].total_length_mm;
    const float len_thumb = report.digits[0].total_length_mm;

    if (len_ring > 1e-3f) {
        report.ratios.ratio_2d_4d = len_index / len_ring;
        report.ratios.ratio_3d_4d = len_middle / len_ring;
    }
    if (len_middle > 1e-3f) {
        report.ratios.ratio_2d_3d = len_index / len_middle;
    }
    if (len_pinky > 1e-3f) {
        report.ratios.ratio_4d_5d = len_ring / len_pinky;
    }
    if (len_index > 1e-3f) {
        report.ratios.thumb_to_index = len_thumb / len_index;
    }

    // Spans and spread angles
    report.spans.thumb_index_angle_deg = angleDeg(lm[1], lm[4], lm[5], lm[8], w, h);
    report.spans.index_middle_angle_deg = angleDeg(lm[5], lm[8], lm[9], lm[12], w, h);
    report.spans.middle_ring_angle_deg = angleDeg(lm[9], lm[12], lm[13], lm[16], w, h);
    report.spans.ring_pinky_angle_deg = angleDeg(lm[13], lm[16], lm[17], lm[20], w, h);

    report.spans.thumb_index_distance_mm = distPx(lm[4], lm[8], w, h) * mm_per_px;
    report.spans.index_middle_distance_mm = distPx(lm[8], lm[12], w, h) * mm_per_px;
    report.spans.middle_ring_distance_mm = distPx(lm[12], lm[16], w, h) * mm_per_px;
    report.spans.ring_pinky_distance_mm = distPx(lm[16], lm[20], w, h) * mm_per_px;

    // Anatomical pads
    static const std::vector<std::pair<std::string, std::string>> kPadMapping = {
        {"thenar", "venus"},
        {"hypothenar", "luna"},
        {"radial_interdigital", "jupiter"},
        {"central_interdigital", "saturn"},
        {"ring_interdigital", "apollo"},
        {"ulnar_interdigital", "mercury"},
        {"palmar_fossa", "mars"}
    };

    for (const auto& pad_pair : kPadMapping) {
        AnatomicalPadMetric m;
        m.pad_name = pad_pair.first;
        // Check if measured depth exists under anatomical name or legacy mount name
        if (pad_depth_mm.count(pad_pair.first) > 0) {
            m.height_mm = pad_depth_mm.at(pad_pair.first);
        } else if (pad_depth_mm.count(pad_pair.second) > 0) {
            m.height_mm = pad_depth_mm.at(pad_pair.second);
        }
        report.pads.push_back(m);
    }

    return report;
}

} // namespace CurvEngine::geometry
