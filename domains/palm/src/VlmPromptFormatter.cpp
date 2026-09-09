#include "curv/domain/VlmPromptFormatter.hpp"
#include <iomanip>
#include <sstream>

namespace CurvEngine::domains {

std::string VlmPromptFormatter::getSystemInstruction() {
    return "You are an expert Vedic Samudrika Shastra scholar and precision biometric analyst.\n"
           "You are provided with an analytically verified topological line graph extracted via sub-pixel computer "
           "vision.\n\n"
           "CRITICAL CONSTRAINTS & GUARDRAILS:\n"
           "1. You must ONLY interpret lines, junctions, and anatomical mounts explicitly verified in the evidence "
           "payload.\n"
           "2. If a feature (e.g. Fate Line, Sun Line, secondary branches) has zero verified segments in the evidence, "
           "you MUST explicitly declare it as 'unverified/absent' rather than hallucinating.\n"
           "3. Ground your semantic synthesis strictly in classical Samudrika Shastra principles (Ayur Rekha / Life "
           "Line, Matru Rekha / Head Line, Hridaya Rekha / Heart Line).\n"
           "4. Maintain a professional, analytical tone emphasizing biometric structures and tendencies rather than "
           "fatalistic prophecies.";
}

std::string VlmPromptFormatter::buildUserPrompt(const nlohmann::json& evidence_json, const std::string& user_query) {
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

        const std::vector<std::tuple<std::string, std::string, std::string>> line_info = {
            {"heart_line", "Hridaya Rekha (Heart Line)", "heart_line_id"},
            {"head_line", "Matru Rekha (Head Line)", "head_line_id"},
            {"life_line", "Ayur Rekha (Life Line)", "life_line_id"},
            {"fate_line", "Karma / Shani Rekha (Fate Line)", "fate_line_id"}};

        const bool has_major_lines = dp.contains("major_lines") && dp["major_lines"].is_object();
        for (const auto& [key, title, legacy_id_key] : line_info) {
            if (has_major_lines && dp["major_lines"].contains(key)) {
                const auto& l_obj = dp["major_lines"][key];
                if (l_obj.value("present", false)) {
                    oss << "- **" << title << "**: Verified\n";
                    oss << "  - Arc Length: " << std::fixed << std::setprecision(1)
                        << l_obj.value("arc_length_px", 0.0f) << " px";
                    oss << " | Tortuosity: " << std::setprecision(2) << l_obj.value("tortuosity", 1.0f);
                    oss << " | Angle: " << std::setprecision(1) << l_obj.value("mean_angle_deg", 0.0f) << "°\n";
                    oss << "  - Mounts: `" << l_obj.value("start_zone", "unknown") << "` -> `"
                        << l_obj.value("end_zone", "unknown") << "`";
                    oss << " | Bridged Gaps: " << l_obj.value("gap_count", 0);
                    oss << " | Confidence: " << std::setprecision(2) << l_obj.value("confidence", 0.0f) << "\n";
                    continue;
                }
            } else if (dp.contains(legacy_id_key) && !dp[legacy_id_key].is_null()) {
                oss << "- **" << title << "**: Verified (Segment ID: " << dp[legacy_id_key] << ")\n";
                continue;
            }
            oss << "- **" << title << "**: [UNVERIFIED / ABSENT]\n";
        }

        // Cross-line structure
        if (dp.contains("palm_analysis") && dp["palm_analysis"].is_object()) {
            const auto& pa = dp["palm_analysis"];
            oss << "\n## Cross-Line Biometric Structure\n";
            if (pa.contains("heart_head_min_distance_px") && !pa["heart_head_min_distance_px"].is_null()) {
                oss << "- Heart-Head Separation: " << std::fixed << std::setprecision(1)
                    << pa["heart_head_min_distance_px"].get<float>() << " px\n";
            }
            oss << "- Named Line Intersections: " << pa.value("named_line_intersections", 0) << "\n";
            oss << "- Dominant Crease: " << pa.value("dominant_line", "none") << "\n";
            oss << "- Palm Line Curvature Type: " << pa.value("palm_type_proxy", "balanced") << "\n";
        }

        oss << "\n## Structural Topology\n";
        oss << "- Topological Intersections/Junctions: " << dp.value("intersections", 0) << "\n";
        oss << "- Major Crease Gaps: " << dp.value("continuity_breaks", 0) << "\n";
        oss << "- Total Curve Segments Analyzed: " << dp.value("total_segments_analyzed", 0) << "\n\n";

        if (dp.contains("zone_distribution") && dp["zone_distribution"].is_object()) {
            oss << "## Anatomical Zone & Mount Activity\n";
            for (auto it = dp["zone_distribution"].begin(); it != dp["zone_distribution"].end(); ++it) {
                oss << "- Mount `" << it.key() << "`: " << it.value() << " ridge segments\n";
            }
            oss << "\n";
        }
    }

    // 3. Evidence Items (domain landmarks only - suppress raw line_candidate clutter)
    if (evidence_json.contains("evidence") && evidence_json["evidence"].is_array()) {
        std::vector<nlohmann::json> verified_items;
        for (const auto& item : evidence_json["evidence"]) {
            const std::string type = item.value("type", "unknown");
            if (type != "line_candidate") {
                verified_items.push_back(item);
            }
        }
        if (!verified_items.empty()) {
            oss << "## Extracted Landmark Evidence Items (" << verified_items.size() << " items)\n";
            for (const auto& item : verified_items) {
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
    }

    // 4. User Query or Final Directives
    oss << "## Request\n";
    if (!user_query.empty()) {
        oss << "User Context: " << user_query << "\n\n";
    }
    oss << "Provide an authoritative, grounded Samudrika Shastra reading based STRICTLY on the above verified "
           "evidence.\n"
        << "For any unverified features, state that they are absent or not confirmed by the sensor.";

    return oss.str();
}

nlohmann::json VlmPromptFormatter::formatGeminiRequest(const nlohmann::json& evidence_json,
                                                       const std::string& user_query, float temperature) {
    nlohmann::json request;

    // System instruction
    request["systemInstruction"] = {{"parts", nlohmann::json::array({{{"text", getSystemInstruction()}}})}};

    // User prompt
    request["contents"] = nlohmann::json::array(
        {{{"role", "user"},
          {"parts", nlohmann::json::array({{{"text", buildUserPrompt(evidence_json, user_query)}}})}}});

    // Generation configuration
    request["generationConfig"] = {{"temperature", temperature}, {"maxOutputTokens", 2048}, {"topP", 0.95}};

    return request;
}

} // namespace CurvEngine::domains
