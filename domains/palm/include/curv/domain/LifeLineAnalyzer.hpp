#pragma once

#include <string>
#include <vector>

#include <nlohmann/json.hpp>

#include "curv/domain/CreaseChainLinker.hpp"

namespace CurvEngine::domain {

// Life-line report. Structural sibling of FateReport, scoped per
// palm_prediction_roadmap §7: age-timeline reading is conventionally a
// fate-line and life-line concept — heart/head lines are read for quality,
// not chronology, so no age mapping is built for them.
struct LifeReport {
    bool present{false};
    bool is_short{false};
    bool is_long{false};
    bool broken{false};
    int gap_count{0};
    float age_at_end{0.0F};
    float age_at_break{0.0F};
    // Placeholder age anchors until classically sourced + data-calibrated.
    // Reading copy must not present ages as authoritative while false.
    bool age_calibrated{false};

    [[nodiscard]] nlohmann::json toJson() const;
};

// Pure-geometry life analysis over already-chained major lines.
// Coordinates are canonical patch pixels (usually 512x512, origin top-left).
class LifeLineAnalyzer {
public:
    [[nodiscard]] static LifeReport analyze(const ChainedCreaseLine& life, const ChainedCreaseLine* heart,
                                            const ChainedCreaseLine* head, int patch_dim = 512);

    // t in [0,1] along the arc from origin (index-thumb web) to wrist end.
    // head_t is the arc fraction of the head-line crossing (<0 if none).
    // PLACEHOLDER mapping (linear 0..90, head anchor at 35 mirroring the fate
    // convention): verify against a cited classical source before hard-coding
    // real anchors; see palm_prediction_roadmap §7. Do not invent the citation.
    [[nodiscard]] static float ageAtLifeArc(float t, float head_t = -1.0F);
};

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using LifeReport = CurvEngine::domain::LifeReport;
using LifeLineAnalyzer = CurvEngine::domain::LifeLineAnalyzer;
} // namespace CurvEngine::domains
