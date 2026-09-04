#include "curv/domain/VlmPromptFormatter.hpp"
#include <sstream>
#include <iomanip>

namespace CurvEngine::domains {

std::string VlmPromptFormatter::getSystemInstruction() {
    return "You are an expert Vedic Samudrika Shastra scholar and precision biometric analyst.\n"
           "You are provided with an analytically verified topological line graph extracted via sub-pixel computer vision.\n\n"
           "CRITICAL CONSTRAINTS & GUARDRAILS:\n"
           "1. You must ONLY interpret lines, junctions, and anatomical mounts explicitly verified in the evidence payload.\n"
           "2. If a feature (e.g. Fate Line, Sun Line, secondary branches) has zero verified segments in the evidence, you MUST explicitly declare it as 'unverified/absent' rather than hallucinating.\n"
           "3. Ground your semantic synthesis strictly in classical Samudrika Shastra principles (Ayur Rekha / Life Line, Matru Rekha / Head Line, Hridaya Rekha / Heart Line).\n"
           "4. Maintain a professional, analytical tone emphasizing biometric structures and tendencies rather than fatalistic prophecies.";
}

std::string VlmPromptFormatter::buildUserPrompt(
    const nlohmann::json& evidence_json,
    const std::string& user_query
) {
    std::ostringstream oss;
    oss << "# Verified Biometric Palm Evidence\n\n";

    // 1. Frame Quality & Verification Telemetry
    if (evidence_json.contains("quality")) {
        const auto& q = evidence_json["quality"];
        oss << "## Quality Gate Metrics\n";
        oss << "- Frame Usable: " << (q.value("is_usable", true) ? "YES" : "NO") << "\n";
        oss << "- Focus/Blur Score: " << std::fixed << std::setprecision(1) << q.value("blur_score", 0.0) << "\n";
        oss << "- Mean Luminance: " << q.value("brightness_score", 0.0) << "\n";
        oss << "- Dynamic Range: " << q.value("contrast_score", 0.0) << "\n\n";
    }

    // 2. Domain Analytics (Creases & Mounts)
    if (evidence_json.contains("domain_payload")) {
        const auto& dp = evidence_json["domain_payload"];
        oss << "## Verified Major Creases (Primary Lines)\n";

        // Heart line
        if (dp.contains("heart_line_id") && !dp["heart_line_id"].is_null()) {
            oss << "- **Hridaya Rekha (Heart Line)**: Verified (Segment ID: " << dp["heart_line_id"] << ")\n";
        } else {
            oss << "- **Hridaya Rekha (Heart Line)**: [UNVERIFIED / ABSENT]\n";
        }

        // Head line
        if (dp.contains("head_line_id") && !dp["head_line_id"].is_null()) {
            oss << "- **Matru Rekha (Head Line)**: Verified (Segment ID: " << dp["head_line_id"] << ")\n";
        } else {
            oss << "- **Matru Rekha (Head Line)**: [UNVERIFIED / ABSENT]\n";
        }

        // Life line
        if (dp.contains("life_line_id") && !dp["life_line_id"].is_null()) {
            oss << "- **Ayur Rekha (Life Line)**: Verified (Segment ID: " << dp["life_line_id"] << ")\n";
        } else {
            oss << "- **Ayur Rekha (Life Line)**: [UNVERIFIED / ABSENT]\n";
        }

        oss << "\n## Structural Topology\n";
        oss << "- Topological Intersections/Junctions: " << dp.value("intersections", 0) << "\n";
        oss << "- Continuity Breaks / Gaps: " << dp.value("continuity_breaks", 0) << "\n";
        oss << "- Total Curve Segments Analyzed: " << dp.value("total_segments_analyzed", 0) << "\n\n";

        if (dp.contains("zone_distribution") && dp["zone_distribution"].is_object()) {
            oss << "## Anatomical Zone & Mount Activity\n";
            for (auto it = dp["zone_distribution"].begin(); it != dp["zone_distribution"].end(); ++it) {
                oss << "- Mount `" << it.key() << "`: " << it.value() << " ridge segments\n";
            }
            oss << "\n";
        }
    }

    // 3. Evidence Items
    if (evidence_json.contains("evidence") && evidence_json["evidence"].is_array()) {
        oss << "## Extracted Landmark Evidence Items (" << evidence_json["evidence"].size() << " items)\n";
        for (const auto& item : evidence_json["evidence"]) {
            const std::string type = item.value("type", "unknown");
            const double conf = item.value("confidence", 0.0);
            oss << "- Type: `" << type << "` | Confidence: " << std::fixed << std::setprecision(2) << conf;
            if (item.contains("metadata") && item["metadata"].contains("zone")) {
                oss << " | Zone: " << item["metadata"]["zone"];
            }
            oss << "\n";
        }
        oss << "\n";
    }

    // 4. User Query or Final Directives
    oss << "## Request\n";
    if (!user_query.empty()) {
        oss << "User Context: " << user_query << "\n\n";
    }
    oss << "Provide an authoritative, grounded Samudrika Shastra reading based STRICTLY on the above verified evidence.\n"
        << "For any unverified features, state that they are absent or not confirmed by the sensor.";

    return oss.str();
}

nlohmann::json VlmPromptFormatter::formatGeminiRequest(
    const nlohmann::json& evidence_json,
    const std::string& user_query,
    float temperature
) {
    nlohmann::json request;

    // System instruction
    request["systemInstruction"] = {
        {"parts", nlohmann::json::array({
            {{"text", getSystemInstruction()}}
        })}
    };

    // User prompt
    request["contents"] = nlohmann::json::array({
        {
            {"role", "user"},
            {"parts", nlohmann::json::array({
                {{"text", buildUserPrompt(evidence_json, user_query)}}
            })}
        }
    });

    // Generation configuration
    request["generationConfig"] = {
        {"temperature", temperature},
        {"maxOutputTokens", 2048},
        {"topP", 0.95}
    };

    return request;
}

} // namespace CurvEngine::domains
