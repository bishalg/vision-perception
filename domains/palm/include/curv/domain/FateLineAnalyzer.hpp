#pragma once

#include <string>
#include <vector>

#include <nlohmann/json.hpp>

#include "curv/domain/CreaseChainLinker.hpp"

namespace CurvEngine::domain {

// Detailed fate-line report covering the segregated blog figure groups.
// See FateLineCases.hpp (AllFateCases) and data/fixtures/palm_fate/manifest.json.
// Fate is optional: absent/faint is a valid `no_fate` result.
struct FateReport {
    bool present{false};
    bool no_fate{true};
    // Origin group
    std::string origin{"unknown"}; // moon | life_joint | head_origin | unknown
    bool joined_life_start{false};
    // Course group
    bool is_short{false};
    bool is_long{false};
    bool broken{false};
    int gap_count{0};
    bool double_line{false};
    bool y_shaped{false};
    bool forked_up{false};
    bool branches_up{false};
    bool toward_mars_negative{false};
    bool parallel_life{false};
    // Termination group
    bool stops_at_heart{false};
    bool stops_at_head{false};
    bool merged_head{false};
    bool cross_sun{false};
    bool cross_life{false};
    // Marks group (P2 heuristics)
    bool has_island_fish{false};
    bool has_triangle{false};
    bool has_cross{false};
    bool has_square{false};
    // Mount-mark negation (Track 5: MountMarkDetector, wired in PalmDomainPack).
    bool bad_saturn_mount{false};
    bool bad_mercury_mount{false};
    // Age group
    float age_at_break{0.0F};
    float age_at_end{0.0F};

    [[nodiscard]] nlohmann::json toJson() const;
};

// Pure-geometry fate analysis over already-chained major lines.
// All coordinates are canonical patch pixels (usually 512x512, origin top-left).
class FateLineAnalyzer {
public:
    [[nodiscard]] static FateReport analyze(const ChainedCreaseLine& fate, const ChainedCreaseLine* heart,
                                            const ChainedCreaseLine* head, const ChainedCreaseLine* life,
                                            const RidgeGraph& graph, int patch_dim = 512);

    // y_norm in [0,1] top->bottom; head_y/heart_y are normalized y of those
    // intersections (blog: head=35, heart=53). Falls back to linear 20..70.
    [[nodiscard]] static float ageAtY(float y_norm, float head_y_norm, float heart_y_norm);

    [[nodiscard]] static bool stopsAt(const std::vector<Point2D>& fate_pts, const std::vector<Point2D>& other_pts);

    [[nodiscard]] static bool isParallelPair(const std::vector<Point2D>& a, const std::vector<Point2D>& b,
                                             float min_dist = 8.0F, float max_dist = 25.0F);
};

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using FateReport = CurvEngine::domain::FateReport;
using FateLineAnalyzer = CurvEngine::domain::FateLineAnalyzer;
} // namespace CurvEngine::domains
