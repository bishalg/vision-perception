#pragma once

#include "curv/domain/RoomPolygonDetector.hpp"
#include "curv/domain/VastuMandalaOverlay.hpp"
#include "curv/domain/WallJunctionGraph.hpp"
#include "curv/domain/WallSegmentExtractor.hpp"
#include "curv/interfaces/IDomainPack.hpp"

namespace CurvEngine::domains {

/**
 * @brief Domain pack for architectural floor plan wall extraction, room polygonization,
 * and 9-pad Vastu Purusha Mandala compliance analysis.
 */
class FloorPlanDomainPack final : public IDomainPack {
public:
    FloorPlanDomainPack() = default;
    ~FloorPlanDomainPack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.floor_plan"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;
};

} // namespace CurvEngine::domains
