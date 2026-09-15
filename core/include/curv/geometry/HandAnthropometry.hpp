#pragma once

#include <array>
#include <nlohmann/json.hpp>
#include <string>
#include <unordered_map>
#include <vector>

namespace CurvEngine::geometry {

/**
 * @brief Linear millimeter measurements for individual digits and phalanges.
 */
struct DigitMeasurement {
    std::string name; // "thumb", "index", "middle", "ring", "pinky"
    float total_length_mm{0.0f};
    float proximal_phalanx_mm{0.0f};
    float intermediate_phalanx_mm{0.0f}; // 0 for thumb
    float distal_phalanx_mm{0.0f};
    float length_relative_to_hand{0.0f};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Standard scientific digit ratios (widely utilized in developmental & biomarker research).
 */
struct DigitRatios {
    float ratio_2d_4d{0.0f}; // Index to Ring: 2D:4D
    float ratio_2d_3d{0.0f}; // Index to Middle
    float ratio_3d_4d{0.0f}; // Middle to Ring
    float ratio_4d_5d{0.0f}; // Ring to Pinky
    float thumb_to_index{0.0f};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Overall palmar dimensions and standard anthropometric indices.
 */
struct PalmDimensions {
    float metacarpal_breadth_mm{0.0f};           // Width (index MCP to pinky MCP)
    float palm_length_mm{0.0f};                  // Length (wrist to middle MCP)
    float total_hand_length_mm{0.0f};            // Total length (wrist to middle tip)
    float aspect_ratio{0.0f};                    // breadth / length
    float palm_surface_area_cm2{0.0f};           // Estimated 2D palm area
    std::string hand_shape_type{"proportional"}; // "broad", "elongated", "proportional"

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Interdigital spread angles and Euclidean spans between adjacent digits.
 */
struct InterdigitalSpan {
    float thumb_index_angle_deg{0.0f};
    float index_middle_angle_deg{0.0f};
    float middle_ring_angle_deg{0.0f};
    float ring_pinky_angle_deg{0.0f};
    float thumb_index_distance_mm{0.0f};
    float index_middle_distance_mm{0.0f};
    float middle_ring_distance_mm{0.0f};
    float ring_pinky_distance_mm{0.0f};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Morphometry of anatomical palmar pads (independent of astrological interpretations).
 */
struct AnatomicalPadMetric {
    std::string pad_name; // "thenar", "hypothenar", "radial_interdigital", "central_interdigital", "ring_interdigital",
                          // "ulnar_interdigital", "palmar_fossa"
    float height_mm{0.0f};
    float relative_prominence{0.0f};
    float area_mm2{0.0f};
    float curvature{0.0f};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Morphometry of palmar flexion creases using standard anatomical nomenclature.
 */
struct PalmarCreaseMetric {
    std::string anatomical_name;   // "distal_transverse_crease", "proximal_transverse_crease",
                                   // "thenar_longitudinal_crease", "median_longitudinal_crease"
    std::string traditional_alias; // "heart", "head", "life", "fate"
    float arc_length_mm{0.0f};
    float chord_length_mm{0.0f};
    float tortuosity{1.0f};
    bool is_simian_variant{false};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Comprehensive, domain-agnostic hand anthropometry report.
 */
struct HandAnthropometryReport {
    std::string schema_version{"curv_hand_anthropometry_v1"};
    PalmDimensions palm;
    std::vector<DigitMeasurement> digits;
    DigitRatios ratios;
    InterdigitalSpan spans;
    std::vector<AnatomicalPadMetric> pads;
    std::vector<PalmarCreaseMetric> creases;
    float mm_per_pixel{0.0f};
    std::string hand_side{"auto"};

    [[nodiscard]] nlohmann::json toJson() const;
};

/**
 * @brief Pure geometric calculator for hand anthropometry and morphology.
 */
class HandAnthropometer {
public:
    [[nodiscard]] static HandAnthropometryReport
    compute(const std::vector<std::array<float, 2>>& landmarks_21, int image_width = 1000, int image_height = 1000,
            float reference_palm_width_mm = 80.0f, const std::unordered_map<std::string, float>& pad_depth_mm = {},
            const std::string& hand_side = "auto");
};

} // namespace CurvEngine::geometry
