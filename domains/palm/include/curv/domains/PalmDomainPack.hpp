#pragma once

#include "curv/interfaces/IDomainPack.hpp"

namespace CurvEngine::domains {

/**
 * @brief Domain pack for biometric palm crease mapping and anatomical landmark classification.
 */
class PalmDomainPack final : public IDomainPack {
public:
    PalmDomainPack() = default;
    ~PalmDomainPack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.palm"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;
};

} // namespace CurvEngine::domains
