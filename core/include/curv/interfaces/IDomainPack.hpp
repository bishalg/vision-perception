#pragma once

#include "curv/Evidence.hpp"
#include "curv/Frame.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include "curv/geometry/RoiWarper.hpp"
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
    /// Optional 21-point landmarks in the same space as `ridges` (usually canonical patch).
    const geometry::HandLandmarks* landmarks{nullptr};
    /// Self-describing provenance for the ridge graph's input image, e.g.
    /// "flat_field_clahe_v1" or "none" — exported verbatim into the domain payload so
    /// every JSON capture states how it was produced.
    const char* preprocessing{"none"};
    /// Optional alternate ridge graph used for micro-sign detection instead of `ridges`.
    /// The major-crease and micro-sign consumers want different images: CLAHE helps the
    /// major pass but collapses micro-sign detection on the canonical capture (1 -> 0), so
    /// a caller can pass the contrast-lifted graph as `ridges` and a flattened-only graph
    /// here. Same crease scale, not a finer one. When null, signs come from `ridges`.
    const RidgeGraph* sign_ridges{nullptr};
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
