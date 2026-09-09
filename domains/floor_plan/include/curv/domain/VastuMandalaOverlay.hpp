#pragma once

#include "curv/domain/WallTypes.hpp"
#include <vector>

namespace CurvEngine::domain {

struct VastuConfig {
    float north_orientation_deg{0.0f}; // 0.0 = North is UP, 90.0 = North is RIGHT
};

class VastuMandalaOverlay {
public:
    explicit VastuMandalaOverlay(VastuConfig config = VastuConfig{}) : config_(config) {}

    /**
     * @brief Computes the 9-pad Vastu Purusha Mandala zones over the building footprint.
     */
    [[nodiscard]] std::vector<VastuZone> generateMandala(float min_x, float min_y, float max_x, float max_y) const;

    /**
     * @brief Maps rooms to primary Vastu zones based on centroid location.
     */
    void assignRoomsToZones(std::vector<RoomPolygon>& rooms, const std::vector<VastuZone>& zones) const;

    /**
     * @brief Evaluates architectural fixtures against classical Vastu principles.
     */
    void evaluateFixtures(std::vector<FixtureItem>& fixtures, const std::vector<VastuZone>& zones,
                          const std::vector<RoomPolygon>& rooms) const;

    /**
     * @brief Helper to get localized name and element of a Vastu zone.
     */
    static std::pair<std::string, std::string> getZoneMetadata(VastuZoneId zone_id);

private:
    VastuConfig config_;
};

} // namespace CurvEngine::domain
