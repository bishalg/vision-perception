#pragma once

#include <nlohmann/json.hpp>
#include <string>

namespace CurvEngine {

/**
 * @brief Represents an immutable, serializable perceptual evidence finding.
 */
struct Evidence {
    std::string evidence_id;
    std::string type; // e.g. "quality_report", "line_candidate", "ridge_graph"
    double confidence{1.0};
    nlohmann::json geometry;
    nlohmann::json metadata;

    Evidence() = default;

    Evidence(std::string in_evidence_id, std::string in_type, double in_confidence = 1.0,
             nlohmann::json in_geometry = nlohmann::json::object(),
             nlohmann::json in_metadata = nlohmann::json::object());

    [[nodiscard]] nlohmann::json to_json() const;
};

} // namespace CurvEngine
