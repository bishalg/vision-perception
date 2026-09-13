#pragma once

#include <string>
#include <vector>

namespace CurvEngine::domain {

// Generic case-table row generalizing the FateLineCases.hpp pattern to
// heart/head/life lines.
//
// HONESTY CONTRACT (palm_prediction_roadmap §7/§9): rows whose classical
// meaning still needs a research pass carry classical_note starting with
// "RESEARCH NEEDED" and implemented=false. Only pure-geometry structural
// cases (presence, short/long, broken, merges) ship implemented=true.
// Mobile reading copy must use structural analyzer flags, never the
// scaffold `use` prose of unresearched rows.
struct LineCaseDef {
    std::string id;             // stable snake_case id, e.g. "heart_simian_merge"
    std::string line_id;        // "heart_line" | "head_line" | "life_line"
    std::string group;          // "Presence" | "Course" | "Termination" | "Marks" | "Age"
    std::string primitive;      // CV rule, as text
    std::string use;            // interpretation trigger (scaffold wording unless cited)
    std::string classical_note; // citation, or "RESEARCH NEEDED: ..."
    int priority{3};            // 0 = P0 (highest) .. 3 = P3
    bool implemented{false};
};

// Heart line: transverse crease under the fingers. P0 = presence, short/long
// (width basis), broken, Simian merge (heart+head fused — the single most
// classical heart-line item, structural and citation-stable).
[[nodiscard]] inline std::vector<LineCaseDef> HeartLineCases() {
    return {
        {"heart_present", "heart_line", "Presence", "chained heart_line valid, arc>=40px, conf>=0.55",
         "line exists (scaffold)", "Structural.", 0, true},
        {"heart_short", "heart_line", "Course", "arc <= 0.55 * palm_width (LineLengthTable)",
         "short transverse span (scaffold)", "Mirrors fate short/long precedent; ratios are placeholders.", 0, true},
        {"heart_long", "heart_line", "Course", "arc >= 0.80 * palm_width (LineLengthTable)",
         "long transverse span (scaffold)", "Mirrors fate short/long precedent; ratios are placeholders.", 0, true},
        {"heart_broken", "heart_line", "Course", "gap_count >= 1 from constituent segments",
         "interrupted span (scaffold)", "Structural.", 1, true},
        {"heart_simian_merge", "heart_line", "Course", "min distance heart<->head < 6px with Y overlap",
         "Simian crease (fused heart+head) (scaffold)", "RESEARCH NEEDED: cite classical source before shipping copy.",
         0, true},
        {"heart_terminates_fate", "heart_line", "Termination", "fate endpoint touches heart polyline",
         "fate terminated by heart (scaffold)", "RESEARCH NEEDED: cite classical source before shipping copy.", 2,
         true},
    };
}

// Head line: mid diagonal. P0 = presence, short/long, broken, life-origin
// joint (head and life commonly share origin — structural).
[[nodiscard]] inline std::vector<LineCaseDef> HeadLineCases() {
    return {
        {"head_present", "head_line", "Presence", "chained head_line valid, arc>=40px, conf>=0.55",
         "line exists (scaffold)", "Structural.", 0, true},
        {"head_short", "head_line", "Course", "arc <= 0.45 * palm_width (LineLengthTable)", "short span (scaffold)",
         "Mirrors fate short/long precedent; ratios are placeholders.", 0, true},
        {"head_long", "head_line", "Course", "arc >= 0.75 * palm_width (LineLengthTable)", "long span (scaffold)",
         "Mirrors fate short/long precedent; ratios are placeholders.", 0, true},
        {"head_broken", "head_line", "Course", "gap_count >= 1 from constituent segments",
         "interrupted span (scaffold)", "Structural.", 1, true},
        {"head_joined_life_origin", "head_line", "Course", "head start within 10px of life start",
         "shared origin with life (scaffold)", "RESEARCH NEEDED: cite classical source before shipping copy.", 0, true},
        {"head_merged_heart", "head_line", "Course", "min distance head<->heart < 6px with Y overlap",
         "Simian crease, head side (scaffold)", "RESEARCH NEEDED: cite classical source before shipping copy.", 1,
         true},
    };
}

// Life line: C-curve around thenar. P0 = presence, short/long (height basis),
// broken, age mapping (LifeLineAnalyzer). Meaning rows need research.
[[nodiscard]] inline std::vector<LineCaseDef> LifeLineCases() {
    return {
        {"life_present", "life_line", "Presence", "chained life_line valid, arc>=40px, conf>=0.55",
         "line exists (scaffold)", "Structural.", 0, true},
        {"life_short", "life_line", "Course", "arc <= 0.40 * palm_height (LineLengthTable)",
         "short arc (scaffold; classically NOT a lifespan claim)", "Mirrors fate ratios; placeholders.", 0, true},
        {"life_long", "life_line", "Course", "arc >= 0.70 * palm_height (LineLengthTable)", "long arc (scaffold)",
         "Mirrors fate ratios; placeholders.", 0, true},
        {"life_broken", "life_line", "Course", "gap_count >= 1 from constituent segments", "interrupted arc (scaffold)",
         "Structural.", 1, true},
        {"life_age_mapping", "life_line", "Age", "arc-fraction -> 0..90 via LifeLineAnalyzer::ageAtLifeArc",
         "chronology along arc (scaffold anchors)",
         "RESEARCH NEEDED: verify 0..90 range + head anchor vs cited source.", 1, true},
    };
}

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using LineCaseDef = CurvEngine::domain::LineCaseDef;
} // namespace CurvEngine::domains
