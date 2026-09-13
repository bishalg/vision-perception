#pragma once

#include <string>
#include <vector>

namespace CurvEngine::domain {

// Per-line short/long classification thresholds.
//
// Classical Samudrika Shastra defines "short" and "long" differently per line
// because each line's natural anatomical span differs (fate runs wrist->fingers
// vertically; heart/head run horizontally under the fingers; life arcs tightly
// around the thenar mount). A single ratio pair is fate-specific, not universal.
//
// Normalization basis ("palm_height" | "palm_width") is explicit per line:
// vertical lines normalize against canonical patch height (the fate-line
// precedent, FateLineAnalyzer kShortRatio/kLongRatio), horizontal lines
// against measured palm width (pinky-mount to Jupiter-mount span).
//
// All rows start with calibrated=false: these are hand-picked placeholders,
// in the same unvalidated state fate's constants are in today. Do NOT present
// derived short/long claims as calibrated. See tools/calibrate_line_thresholds.py
// (planned) for the dataset-percentile calibration plan.
struct LineLengthThresholds {
    std::string line_id;        // "heart_line" | "head_line" | "life_line" | "fate_line"
    float short_ratio{0.0F};    // arc/basis <= this -> "short"
    float long_ratio{1.0F};     // arc/basis >= this -> "long" (between = "medium")
    std::string basis;          // "palm_height" | "palm_width"
    std::string classical_note; // citation / rationale, mirroring blog_anchor in FateLineCases
    bool calibrated{false};     // tentative until data-calibrated
};

struct LineLengthClass {
    bool is_short{false};
    bool is_long{false};
};

class LineLengthTable {
public:
    // Placeholder table (see struct note). fate values mirror the existing
    // FateLineAnalyzer kShortRatio/kLongRatio so both paths agree.
    [[nodiscard]] static std::vector<LineLengthThresholds> defaults() {
        return {
            {"fate_line", 0.40F, 0.70F, "palm_height",
             "Existing convention: vertical wrist->saturn span vs patch height.", false},
            {"life_line", 0.40F, 0.70F, "palm_height",
             "Mirrors fate's span; a short life arc is classically NOT a lifespan claim.", false},
            {"heart_line", 0.55F, 0.80F, "palm_width",
             "Ends under index (short) vs crosses to Jupiter mount edge (long).", false},
            {"head_line", 0.45F, 0.75F, "palm_width",
             "Ends below middle finger (short) vs reaches Mercury-mount edge (long).", false},
        };
    }

    [[nodiscard]] static const LineLengthThresholds* find(const std::vector<LineLengthThresholds>& table,
                                                          const std::string& line_id) {
        for (const auto& row : table) {
            if (row.line_id == line_id) {
                return &row;
            }
        }
        return nullptr;
    }

    // Pure classification: arc_px measured, basis_px caller-provided
    // (palm height for vertical lines, measured palm width for horizontal).
    [[nodiscard]] static LineLengthClass classify(const LineLengthThresholds& row, float arc_px, float basis_px) {
        LineLengthClass out;
        if (basis_px <= 0.0F || arc_px < 0.0F) {
            return out;
        }
        const float ratio = arc_px / basis_px;
        out.is_short = ratio <= row.short_ratio;
        out.is_long = ratio >= row.long_ratio;
        return out;
    }
};

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using LineLengthThresholds = CurvEngine::domain::LineLengthThresholds;
using LineLengthClass = CurvEngine::domain::LineLengthClass;
using LineLengthTable = CurvEngine::domain::LineLengthTable;
} // namespace CurvEngine::domains
