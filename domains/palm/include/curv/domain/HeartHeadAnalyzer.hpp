#pragma once

#include <string>

#include <nlohmann/json.hpp>

#include "curv/domain/CreaseChainLinker.hpp"

namespace CurvEngine::domain {

// Heart-line report: structural flags only (Track 4). Meaning prose for
// non-structural cases lives in LineCaseTable as RESEARCH-NEEDED scaffold
// and must not ship as reading copy until cited.
struct HeartReport {
    bool present{false};
    bool is_short{false};
    bool is_long{false};
    bool broken{false};
    int gap_count{0};
    bool simian_merge{false};    // heart fused with head (single transverse crease)
    bool terminates_fate{false}; // a fate endpoint touches this heart line

    [[nodiscard]] nlohmann::json toJson() const;
};

// Head-line report: structural flags only (Track 4, same honesty contract).
struct HeadReport {
    bool present{false};
    bool is_short{false};
    bool is_long{false};
    bool broken{false};
    int gap_count{0};
    bool merged_heart{false};       // Simian crease, head side
    bool joined_life_origin{false}; // head start within 10px of life start

    [[nodiscard]] nlohmann::json toJson() const;
};

// Pure-geometry analyzers over already-chained major lines.
// Coordinates are canonical patch pixels (usually 512x512, origin top-left).
// palm_width_px is the measured palm width for horizontal-line normalization
// (Track 2 basis); patch_dim is the canonical patch edge (fate precedent).
class HeartLineAnalyzer {
public:
    [[nodiscard]] static HeartReport analyze(const ChainedCreaseLine& heart, const ChainedCreaseLine* head,
                                             const ChainedCreaseLine* fate, int patch_dim, float palm_width_px);
};

class HeadLineAnalyzer {
public:
    [[nodiscard]] static HeadReport analyze(const ChainedCreaseLine& head, const ChainedCreaseLine* heart,
                                            const ChainedCreaseLine* life, int patch_dim, float palm_width_px);
};

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using HeartReport = CurvEngine::domain::HeartReport;
using HeadReport = CurvEngine::domain::HeadReport;
using HeartLineAnalyzer = CurvEngine::domain::HeartLineAnalyzer;
using HeadLineAnalyzer = CurvEngine::domain::HeadLineAnalyzer;
} // namespace CurvEngine::domains
