#include "curv/Evidence.hpp"

namespace CurvEngine {

Evidence::Evidence(std::string in_evidence_id, std::string in_type, double in_confidence, nlohmann::json in_geometry,
                   nlohmann::json in_metadata)
    : evidence_id(std::move(in_evidence_id)), type(std::move(in_type)), confidence(in_confidence),
      geometry(std::move(in_geometry)), metadata(std::move(in_metadata)) {}

nlohmann::json Evidence::to_json() const {
    return nlohmann::json{{"evidence_id", evidence_id},
                          {"type", type},
                          {"confidence", confidence},
                          {"geometry", geometry.is_null() ? nlohmann::json::object() : geometry},
                          {"metadata", metadata.is_null() ? nlohmann::json::object() : metadata}};
}

} // namespace CurvEngine
