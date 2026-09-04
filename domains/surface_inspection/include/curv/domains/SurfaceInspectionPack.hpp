#pragma once

#include "curv/interfaces/IDomainPack.hpp"

namespace CurvEngine::domains {

/**
 * @brief Domain pack for surface defect detection (cracks, micro-scratches, fissures).
 */
class SurfaceInspectionPack final : public IDomainPack {
public:
    explicit SurfaceInspectionPack(float min_length_px = 25.0f);
    ~SurfaceInspectionPack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.surface_inspection"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;

private:
    float min_length_px_;
};

} // namespace CurvEngine::domains
