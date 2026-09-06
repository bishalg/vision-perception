#pragma once

#include "curv/interfaces/IDomainPack.hpp"

namespace CurvEngine::domains {

struct GeospatialFloodConfig {
    float river_min_length_px{150.0f};    ///< Segments longer than this are primary river trunks.
    float tributary_min_length_px{50.0f}; ///< Segments between this and river_min are tributaries.
};

/**
 * @brief Domain pack for geospatial river corridor and flood surge analysis.
 *
 * Interprets domain-agnostic RidgeGraph curvilinear segments as hydrological
 * features (river channel trunks, braided tributaries, minor drainage gullies),
 * scores connectivity, and produces domain evidence and summary telemetry.
 */
class GeospatialFloodPack final : public IDomainPack {
public:
    using Config = GeospatialFloodConfig;

    explicit GeospatialFloodPack(Config cfg = {});
    ~GeospatialFloodPack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.geospatial"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;

private:
    Config cfg_;
};

} // namespace CurvEngine::domains
