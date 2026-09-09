#include <gtest/gtest.h>
#include <nlohmann/json.hpp>
#include "curv/domain/VlmPromptFormatter.hpp"

using namespace CurvEngine::domains;

TEST(VlmPromptFormatterTest, Test_SystemInstructionGuardrails) {
    const std::string sys_instruction = VlmPromptFormatter::getSystemInstruction();

    EXPECT_FALSE(sys_instruction.empty());
    EXPECT_NE(sys_instruction.find("Samudrika Shastra"), std::string::npos);
    EXPECT_NE(sys_instruction.find("GUARDRAILS"), std::string::npos);
    EXPECT_NE(sys_instruction.find("hallucinating"), std::string::npos);
}

TEST(VlmPromptFormatterTest, Test_BuildUserPrompt_And_GeminiRequest) {
    nlohmann::json mock_evidence = {
        {"quality", {
            {"blur_score", 245.5},
            {"brightness_score", 128.0},
            {"contrast_score", 45.0},
            {"is_usable", true}
        }},
        {"domain_payload", {
            {"status", "analyzed"},
            {"heart_line_id", 10},
            {"head_line_id", 20},
            {"life_line_id", 30},
            {"intersections", 2},
            {"continuity_breaks", 0},
            {"total_segments_analyzed", 14},
            {"zone_distribution", {
                {"jupiter_zone", 3},
                {"apollo_zone", 2},
                {"plain_of_mars", 4}
            }}
        }},
        {"evidence", nlohmann::json::array({
            {
                {"type", "crease_heart"},
                {"confidence", 0.95},
                {"metadata", {{"zone", "jupiter_zone"}}}
            }
        })}
    };

    const std::string prompt_text = VlmPromptFormatter::buildUserPrompt(mock_evidence, "Evaluate leadership potential.");

    EXPECT_NE(prompt_text.find("Hridaya Rekha"), std::string::npos);
    EXPECT_NE(prompt_text.find("Matru Rekha"), std::string::npos);
    EXPECT_NE(prompt_text.find("Ayur Rekha"), std::string::npos);
    EXPECT_NE(prompt_text.find("jupiter_zone"), std::string::npos);
    EXPECT_NE(prompt_text.find("Evaluate leadership potential"), std::string::npos);

    // Format Gemini Request Payload
    nlohmann::json gemini_req = VlmPromptFormatter::formatGeminiRequest(mock_evidence, "Evaluate leadership potential.", 0.2f);

    EXPECT_TRUE(gemini_req.contains("contents"));
    EXPECT_TRUE(gemini_req.contains("systemInstruction"));
    EXPECT_TRUE(gemini_req.contains("generationConfig"));

    EXPECT_EQ(gemini_req["generationConfig"]["temperature"], 0.2f);
    EXPECT_FALSE(gemini_req["contents"][0]["parts"][0]["text"].get<std::string>().empty());
}

TEST(VlmPromptFormatterTest, Test_MajorLinesPrompt_And_NoLineCandidateFlood) {
    nlohmann::json evidence_with_major_lines = {
        {"quality", {
            {"blur_score", 350.0},
            {"brightness_score", 140.0},
            {"contrast_score", 48.0},
            {"is_usable", true}
        }},
        {"domain_payload", {
            {"status", "analyzed"},
            {"major_lines", {
                {"heart_line", {
                    {"present", true},
                    {"confidence", 0.92},
                    {"arc_length_px", 185.0},
                    {"chord_length_px", 165.0},
                    {"tortuosity", 1.12},
                    {"mean_angle_deg", 14.5},
                    {"start_zone", "jupiter_zone"},
                    {"end_zone", "mercury_zone"},
                    {"gap_count", 1}
                }},
                {"head_line", {
                    {"present", true},
                    {"confidence", 0.89},
                    {"arc_length_px", 172.0},
                    {"chord_length_px", 160.0},
                    {"tortuosity", 1.075},
                    {"mean_angle_deg", 22.0},
                    {"start_zone", "mars_lower"},
                    {"end_zone", "plain_of_mars"},
                    {"gap_count", 0}
                }},
                {"life_line", {
                    {"present", true},
                    {"confidence", 0.94},
                    {"arc_length_px", 210.0},
                    {"chord_length_px", 170.0},
                    {"tortuosity", 1.235},
                    {"mean_angle_deg", 65.0},
                    {"start_zone", "mars_lower"},
                    {"end_zone", "venus_zone"},
                    {"gap_count", 0}
                }},
                {"fate_line", {
                    {"present", false}
                }}
            }},
            {"palm_analysis", {
                {"heart_head_min_distance_px", 34.5},
                {"named_line_intersections", 1},
                {"dominant_line", "life_line"},
                {"palm_type_proxy", "balanced"}
            }},
            {"intersections", 4},
            {"continuity_breaks", 1},
            {"total_segments_analyzed", 200}
        }},
        {"evidence", nlohmann::json::array()}
    };

    // Flood the evidence array with 150 raw line_candidate items
    for (int i = 0; i < 150; ++i) {
        evidence_with_major_lines["evidence"].push_back({
            {"type", "line_candidate"},
            {"confidence", 0.95}
        });
    }

    // Add 1 domain-level landmark item
    evidence_with_major_lines["evidence"].push_back({
        {"type", "crease_heart"},
        {"confidence", 0.92},
        {"metadata", {{"zone", "jupiter_zone"}}}
    });

    const std::string prompt = VlmPromptFormatter::buildUserPrompt(evidence_with_major_lines, "General reading");

    // Must contain grounded metrics
    EXPECT_NE(prompt.find("Arc Length: 185.0 px"), std::string::npos);
    EXPECT_NE(prompt.find("Tortuosity: 1.12"), std::string::npos);
    EXPECT_NE(prompt.find("Heart-Head Separation: 34.5 px"), std::string::npos);
    EXPECT_NE(prompt.find("Karma / Shani Rekha (Fate Line)"), std::string::npos);
    EXPECT_NE(prompt.find("[UNVERIFIED / ABSENT]"), std::string::npos);

    // CRITICAL: Must NOT contain raw line_candidate clutter!
    EXPECT_EQ(prompt.find("line_candidate"), std::string::npos);

    // Count lines in prompt: must be compact (under 60 lines)
    const auto line_count = std::count(prompt.begin(), prompt.end(), '\n');
    EXPECT_LT(line_count, 60);
}

