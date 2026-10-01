#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/pattern/PatternValidation.hpp"
#include <string>
#include <vector>

namespace CurvEngine::pattern {

/**
 * @brief Categorization of morphological signs & micro-patterns on crease curves.
 */
enum class PatternType {
    Square,
    Triangle,
    Star,
    Circle,
    Island,
    Cross,
    Trident,
    SupportingLine,
    ChainedLine,
    Tassels,
    Net,
    Fish
};

/**
 * @brief One named soft-filter contribution to a candidate's confidence
 *        (populated by the validated overloads; serialized only in debug builds).
 */
struct FilterScore {
    std::string name;
    float score{1.0F};
};

/**
 * @brief Detected geometric sign or micro-pattern.
 */
struct DetectedPattern {
    PatternType type{PatternType::Square};
    std::string name;
    float confidence{1.0f};
    Point2D centroid;
    std::vector<Point2D> points;
    std::vector<int> source_segment_ids;
    std::vector<FilterScore> filter_trace;

    // Optional host line attachment
    int host_curve_id{-1};
    float host_arc_s{0.0f};       // Relative position [0.0, 1.0] along host curve
    float estimated_scalar{0.0f}; // e.g. age at pattern location
};

/**
 * @brief Canonical display name for a pattern type (stable across JSON consumers).
 */
[[nodiscard]] const char* patternTypeName(PatternType type);

/**
 * @brief Topological graph analyzer that detects micro-patterns from RidgeGraph primitives.
 *
 * Two API tiers:
 *  - Legacy topology-only finders (fixed confidences): kept byte-compatible for existing
 *    consumers; they fire on graph structure alone and ignore geometry.
 *  - Validated overloads taking a MicroSignConfig: topology proposes, soft geometric
 *    filters (size/angle/straightness, normalized by palm width) score, and candidates
 *    below the config confidence floor are dropped. See docs/palm_fate_line_spec.md P2.
 */
class MicroPatternDetector {
public:
    [[nodiscard]] static std::vector<DetectedPattern> detectAll(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> detectAll(const RidgeGraph& graph, const MicroSignConfig& cfg);

    [[nodiscard]] static std::vector<DetectedPattern> findTriangles(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findSquares(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findIslands(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findCrosses(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findStars(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findTridents(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findNets(const RidgeGraph& graph);
    [[nodiscard]] static std::vector<DetectedPattern> findFish(const RidgeGraph& graph);

    [[nodiscard]] static std::vector<DetectedPattern> findTriangles(const RidgeGraph& graph,
                                                                    const MicroSignConfig& cfg);
    [[nodiscard]] static std::vector<DetectedPattern> findSquares(const RidgeGraph& graph, const MicroSignConfig& cfg);
    [[nodiscard]] static std::vector<DetectedPattern> findIslands(const RidgeGraph& graph, const MicroSignConfig& cfg);
    [[nodiscard]] static std::vector<DetectedPattern> findCrosses(const RidgeGraph& graph, const MicroSignConfig& cfg);
    [[nodiscard]] static std::vector<DetectedPattern> findStars(const RidgeGraph& graph, const MicroSignConfig& cfg);
    [[nodiscard]] static std::vector<DetectedPattern> findTridents(const RidgeGraph& graph, const MicroSignConfig& cfg);
    [[nodiscard]] static std::vector<DetectedPattern> findNets(const RidgeGraph& graph, const MicroSignConfig& cfg);

    /// Validated fish: elongated island/triangle body with an adjacent degree-4 tail
    /// junction. This is the explicitly encoded topology — the chart's
    /// 2-strokes/2-crossings fish IS the island body, so it reports as Island unless the
    /// tail junction is present. detectAll dedupes candidates sharing support and keeps
    /// the higher-confidence sign, so an island and its fish never both surface for one
    /// physical mark.
    [[nodiscard]] static std::vector<DetectedPattern> findFish(const RidgeGraph& graph, const MicroSignConfig& cfg);
};

} // namespace CurvEngine::pattern
