#pragma once

#include "curv/Evidence.hpp"
#include "curv/Frame.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include <nlohmann/json.hpp>
#include <string>
#include <vector>

namespace CurvEngine {

/**
 * @brief Input bundle supplied to a domain pack from the perception core.
 */
struct DomainInput {
    const Frame& frame;
    const RidgeGraph& ridges;
    const QualityReport& quality;
};

/**
 * @brief Semantic interpretation and evidence produced by a domain pack.
 */
struct DomainOutput {
    std::string domain_name;
    std::vector<Evidence> domain_evidence;
    nlohmann::json domain_payload;
};

/**
 * @brief Pure virtual contract implemented by pluggable domain extension packs.
 */
class IDomainPack {
public:
    virtual ~IDomainPack() = default;

    [[nodiscard]] virtual std::string name() const noexcept = 0;
    [[nodiscard]] virtual std::string version() const noexcept = 0;

    [[nodiscard]] virtual DomainOutput process(const DomainInput& input) = 0;
};

} // namespace CurvEngine
