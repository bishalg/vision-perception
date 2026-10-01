#pragma once

#include "curv/GeometryTypes.hpp"

#include <string>
#include <vector>

namespace CurvEngine::pattern {

/**
 * @brief Palm-width normalization so sign geometry thresholds survive device/resolution changes.
 *
 * All MicroSignRanges values are expressed as fractions of the palm width (or, for angles,
 * degrees). A candidate measured in pixels is converted with norm(); a normalized threshold
 * is converted back to pixels with px().
 */
struct SignScale {
    float palm_width_px{1.0f};

    [[nodiscard]] float norm(float px) const { return px / palm_width_px; }
    [[nodiscard]] float px(float norm_value) const { return norm_value * palm_width_px; }
};

/**
 * @brief Soft acceptance range: 1.0 inside [lo, hi], linear falloff to 0 over `margin` outside.
 *
 * Near-miss geometry yields a partial score instead of a hard accept/reject, so faint or
 * atypical marks rank below crisp ones instead of disappearing.
 */
[[nodiscard]] inline float softRange(float v, float lo, float hi, float margin) {
    if (v >= lo && v <= hi) {
        return 1.0F;
    }
    const float d = (v < lo) ? (lo - v) : (v - hi);
    const float m = (margin > 1e-6F) ? margin : 1e-6F;
    const float s = 1.0F - d / m;
    return (s > 0.0F) ? s : 0.0F;
}

/**
 * @brief Geometric descriptors of a closed loop of points (shoelace area, perimeter,
 *        2nd-moment ellipse elongation, circularity, mean ridge confidence).
 */
struct LoopStats {
    float area_px{0.0F};
    float perimeter_px{0.0F};
    float elongation{1.0F};  // major/minor axis of fitted 2nd-moment ellipse (>= 1)
    float circularity{0.0F}; // 4*pi*A/P^2 (1.0 = perfect circle)
    float mean_conf{0.0F};
};

/**
 * @brief Computes LoopStats for a point loop; the polygon is closed automatically.
 *        Returns zeros / elongation 1 for degenerate (< 3 points) inputs.
 */
[[nodiscard]] LoopStats computeLoopStats(const std::vector<Point2D>& pts);

/**
 * @brief Max perpendicular deviation of the polyline from its endpoint chord, divided by
 *        the chord length (0 = perfectly straight). Returns 0 for degenerate inputs.
 */
[[nodiscard]] float maxChordDeviation(const std::vector<Point2D>& pts);

/**
 * @brief Soft geometric acceptance ranges per sign, all normalized by palm width (PW)
 *        except angles (degrees). Tuned defaults follow docs/palm_fate_line_spec.md P2
 *        case geometry; tune per sign without touching detector code.
 *
 * Ranges exist only for the eight implemented topology finders. Circle / SupportingLine /
 * ChainedLine / Tassels ranges are added together with their detectors (missing-detector PR).
 */
struct MicroSignRanges {
    // Host-line attribution band (fraction of PW; centroid farther than this from every
    // major line is reported as an unattributed / mount-region mark).
    float host_band_norm{0.07F};

    // Island: two-arc loop between one shared junction pair (theta shape).
    float island_size_lo{0.012F};
    float island_size_hi{0.050F};
    float island_size_margin{0.004F};
    float island_elong_lo{1.5F};
    float island_elong_hi{5.0F};
    float island_elong_margin{0.3F};

    // Triangle: 3-cycle.
    float tri_size_lo{0.020F};
    float tri_size_hi{0.090F};
    float tri_size_margin{0.010F};
    float tri_angle_lo{20.0F};
    float tri_angle_hi{140.0F};
    float tri_angle_margin{10.0F};
    float tri_straight_hi{0.18F};
    float tri_straight_margin{0.08F};
    int tri_max_degree4_vertices{1}; // grid-cell guard (hard): >=2 corners of degree 4 => net cell

    // Square: 4-cycle.
    float sq_size_lo{0.020F};
    float sq_size_hi{0.090F};
    float sq_size_margin{0.010F};
    float sq_angle_lo{70.0F};
    float sq_angle_hi{110.0F};
    float sq_angle_margin{15.0F};
    float sq_side_ratio_hi{1.7F};
    float sq_side_ratio_margin{0.25F};
    int sq_max_degree4_vertices{1}; // grid-cell guard (hard)

    // Cross: two strokes crossing once (degree-4 junction). Angle expressed as the acute
    // angle between the two stroke axes: [70, 90] + margin == full-angle [70, 110] + margin.
    float cross_acute_lo{70.0F};
    float cross_acute_hi{90.0F};
    float cross_acute_margin{20.0F};
    float cross_arm_lo{0.020F};
    float cross_arm_hi{0.080F};
    float cross_arm_margin{0.010F};
    float cross_straight_hi{0.15F};
    float cross_straight_margin{0.10F};

    // Star: >= 5 strokes converging on one point.
    float star_arms_lo{6.0F};
    float star_arms_hi{12.0F};
    float star_arms_margin{1.5F};
    float star_pairwise_lo{25.0F};
    float star_pairwise_margin{10.0F};
    float star_arm_lo{0.015F};
    float star_arm_hi{0.060F};
    float star_arm_margin{0.010F};

    // Trident: three prongs fanning from a common point.
    float trident_pairwise_lo{15.0F};
    float trident_pairwise_hi{70.0F};
    float trident_pairwise_margin{8.0F};
    float trident_coherence_lo{0.60F};
    float trident_coherence_margin{0.25F};
    float trident_len_ratio_hi{2.0F};
    float trident_len_ratio_margin{0.5F};
    float trident_prong_lo{0.020F};
    float trident_prong_hi{0.120F};
    float trident_prong_margin{0.020F};

    // Net (grille): cluster of cross junctions connected through shared segments.
    float net_spacing_lo{0.020F};
    float net_spacing_hi{0.080F};
    float net_spacing_margin{0.010F};
    float net_cluster_radius_norm{0.10F};
    int net_min_junctions{4};
    int net_min_shared_links{3};

    // Fish: elongated body (island/triangle) with a nearby degree-4 "tail" junction.
    float fish_tail_dist_hi{0.12F};
    float fish_tail_dist_margin{0.03F};
    float fish_size_lo{0.020F};
    float fish_size_hi{0.090F};
    float fish_size_margin{0.015F};
    float fish_aspect_lo{1.8F};
    float fish_aspect_margin{0.4F};

    // Candidates whose validated confidence falls below this are dropped entirely.
    float min_confidence{0.20F};
};

/**
 * @brief Scale + ranges bundle passed to the validated MicroPatternDetector overloads.
 */
struct MicroSignConfig {
    SignScale scale;
    MicroSignRanges ranges;
};

} // namespace CurvEngine::pattern
