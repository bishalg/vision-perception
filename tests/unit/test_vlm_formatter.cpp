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
