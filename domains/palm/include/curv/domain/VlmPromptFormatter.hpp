#pragma once

#include <string>
#include <nlohmann/json.hpp>

namespace CurvEngine::domains {

/**
 * @brief Formats analytically verified perception evidence into grounded multimodal
 * prompts for Large Language Models (Google Gemini 2.5 / Gemma 2).
 * 
 * Enforces strict anti-hallucination guardrails and grounds interpretations
 * strictly in classical Vedic Samudrika Shastra rules.
 */
class VlmPromptFormatter {
public:
    /**
     * @brief Generates standard system instruction text grounding the model.
     */
    static std::string getSystemInstruction();

    /**
     * @brief Generates structured user prompt content detailing verified evidence.
     * @param evidence_json The full perception evidence JSON produced by the CurvEngine pipeline.
     * @param user_query Optional custom query or context from user (default empty).
     */
    static std::string buildUserPrompt(
        const nlohmann::json& evidence_json,
        const std::string& user_query = ""
    );

    /**
     * @brief Builds complete Gemini API request JSON payload.
     * @param evidence_json Full perception evidence JSON.
     * @param user_query Optional user query.
     * @param temperature Sampling temperature (default 0.2 for deterministic precision).
     * @return Formatted nlohmann::json payload ready for POST /v1beta/models/...:generateContent.
     */
    static nlohmann::json formatGeminiRequest(
        const nlohmann::json& evidence_json,
        const std::string& user_query = "",
        float temperature = 0.2f
    );
};

} // namespace CurvEngine::domains

namespace CurvEngine::domain {
    using VlmPromptFormatter = CurvEngine::domains::VlmPromptFormatter;
}
