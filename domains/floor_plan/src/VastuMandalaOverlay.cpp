#include "curv/domain/VastuMandalaOverlay.hpp"

#include <algorithm>
#include <cmath>
#include <numbers>

namespace CurvEngine::domain {

namespace {

constexpr float kPi = std::numbers::pi_v<float>;

Point2D rotatePoint(const Point2D& pt, const Point2D& center, float rad) {
    const float cos_a = std::cos(rad);
    const float sin_a = std::sin(rad);
    const float dx = pt.x - center.x;
    const float dy = pt.y - center.y;
    return Point2D{center.x + dx * cos_a - dy * sin_a, center.y + dx * sin_a + dy * cos_a};
}

bool isPointInPolygon(const Point2D& pt, const std::vector<Point2D>& poly) {
    if (poly.size() < 3) return false;
    bool inside = false;
    const size_t n = poly.size();
    for (size_t i = 0, j = n - 1; i < n; j = i++) {
        if (((poly[i].y > pt.y) != (poly[j].y > pt.y)) &&
            (pt.x < (poly[j].x - poly[i].x) * (pt.y - poly[i].y) / (poly[j].y - poly[i].y) + poly[i].x)) {
            inside = !inside;
        }
    }
    return inside;
}

} // namespace

std::pair<std::string, std::string> VastuMandalaOverlay::getZoneMetadata(VastuZoneId zone_id) {
    switch (zone_id) {
    case VastuZoneId::BRAHMASTHAN:
        return {"Brahmasthan (Center)", "Akasha (Ether/Space)"};
    case VastuZoneId::ISHANYA:
        return {"Ishanya (Northeast)", "Jala (Water)"};
    case VastuZoneId::PURVA:
        return {"Purva (East)", "Surya (Light/Energy)"};
    case VastuZoneId::AGNI:
        return {"Agni (Southeast)", "Tejas (Fire)"};
    case VastuZoneId::DAKSHINA:
        return {"Dakshina (South)", "Yama (Mars/Action)"};
    case VastuZoneId::NAIRUTYA:
        return {"Nairutya (Southwest)", "Prithvi (Earth/Stability)"};
    case VastuZoneId::PASHCHIMA:
        return {"Pashchima (West)", "Varuna (Saturn/Water)"};
    case VastuZoneId::VAYU:
        return {"Vayu (Northwest)", "Vayu (Air/Movement)"};
    case VastuZoneId::UTTARA:
        return {"Uttara (North)", "Kubera (Mercury/Wealth)"};
    }
    return {"Unknown", "None"};
}

std::vector<VastuZone> VastuMandalaOverlay::generateMandala(float min_x, float min_y, float max_x, float max_y) const {

    std::vector<VastuZone> zones;
    zones.reserve(9);

    const float width = max_x - min_x;
    const float height = max_y - min_y;
    if (width <= 0.0f || height <= 0.0f) {
        return zones;
    }

    const float dx = width / 3.0f;
    const float dy = height / 3.0f;
    const Point2D center{min_x + 0.5f * width, min_y + 0.5f * height};
    const float rot_rad = config_.north_orientation_deg * kPi / 180.0f;

    // Standard 3x3 layout (North is Up):
    // Row 0 (Top):    VAYU(NW),    UTTARA(N),   ISHANYA(NE)
    // Row 1 (Mid):    PASHCHIMA(W),BRAHMA(C),   PURVA(E)
    // Row 2 (Bottom): NAIRUTYA(SW),DAKSHINA(S), AGNI(SE)
    static constexpr VastuZoneId kGrid[3][3] = {{VastuZoneId::VAYU, VastuZoneId::UTTARA, VastuZoneId::ISHANYA},
                                                {VastuZoneId::PASHCHIMA, VastuZoneId::BRAHMASTHAN, VastuZoneId::PURVA},
                                                {VastuZoneId::NAIRUTYA, VastuZoneId::DAKSHINA, VastuZoneId::AGNI}};

    static constexpr const char* kDeities[3][3] = {
        {"Vayu", "Kubera", "Shiva/Ishana"}, {"Varuna", "Brahma", "Indra/Surya"}, {"Nirriti", "Yama", "Agni"}};

    for (int r = 0; r < 3; ++r) {
        for (int c = 0; c < 3; ++c) {
            const float x0 = min_x + static_cast<float>(c) * dx;
            const float y0 = min_y + static_cast<float>(r) * dy;
            const float x1 = x0 + dx;
            const float y1 = y0 + dy;

            Point2D p0{x0, y0};
            Point2D p1{x1, y0};
            Point2D p2{x1, y1};
            Point2D p3{x0, y1};

            if (std::abs(rot_rad) > 1e-4f) {
                p0 = rotatePoint(p0, center, rot_rad);
                p1 = rotatePoint(p1, center, rot_rad);
                p2 = rotatePoint(p2, center, rot_rad);
                p3 = rotatePoint(p3, center, rot_rad);
            }

            VastuZone z;
            z.id = kGrid[r][c];
            const auto [name, elem] = getZoneMetadata(z.id);
            z.name = name;
            z.element = elem;
            z.deity = kDeities[r][c];
            z.polygon = {p0, p1, p2, p3};
            z.area_px = dx * dy;
            zones.push_back(z);
        }
    }

    return zones;
}

void VastuMandalaOverlay::assignRoomsToZones(std::vector<RoomPolygon>& rooms,
                                             const std::vector<VastuZone>& zones) const {

    for (auto& room : rooms) {
        bool assigned = false;
        for (const auto& z : zones) {
            if (isPointInPolygon(room.centroid, z.polygon)) {
                room.primary_vastu_zone = z.id;
                assigned = true;
                break;
            }
        }
        if (!assigned && !zones.empty()) {
            // Find closest zone centroid if room centroid is slightly outside
            float min_dist = 1e9f;
            VastuZoneId closest_id = VastuZoneId::BRAHMASTHAN;
            for (const auto& z : zones) {
                if (z.polygon.size() >= 4) {
                    const float z_cx = 0.25f * (z.polygon[0].x + z.polygon[1].x + z.polygon[2].x + z.polygon[3].x);
                    const float z_cy = 0.25f * (z.polygon[0].y + z.polygon[1].y + z.polygon[2].y + z.polygon[3].y);
                    const float dist = std::hypot(z_cx - room.centroid.x, z_cy - room.centroid.y);
                    if (dist < min_dist) {
                        min_dist = dist;
                        closest_id = z.id;
                    }
                }
            }
            room.primary_vastu_zone = closest_id;
        }
    }
}

void VastuMandalaOverlay::evaluateFixtures(std::vector<FixtureItem>& fixtures, const std::vector<VastuZone>& zones,
                                           const std::vector<RoomPolygon>& rooms) const {

    for (auto& fix : fixtures) {
        // Find enclosing room
        for (const auto& rm : rooms) {
            if (isPointInPolygon(fix.position, rm.vertices)) {
                fix.room_id = rm.id;
                break;
            }
        }

        // Find enclosing Vastu zone
        for (const auto& z : zones) {
            if (isPointInPolygon(fix.position, z.polygon)) {
                fix.vastu_zone = z.id;
                break;
            }
        }

        // Evaluate classical Vastu rules
        switch (fix.type) {
        case FixtureType::STOVE:
            if (fix.vastu_zone == VastuZoneId::AGNI) {
                fix.compliance = ComplianceStatus::COMPLIANT;
                fix.compliance_reason =
                    "Ideal: Cooking stove in Agni (Southeast) fire corner enhances health and vitality.";
            } else if (fix.vastu_zone == VastuZoneId::VAYU) {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Acceptable: Northwest secondary fire placement.";
            } else if (fix.vastu_zone == VastuZoneId::ISHANYA) {
                fix.compliance = ComplianceStatus::MAJOR_DEFECT;
                fix.compliance_reason = "Major Defect: Fire element (stove) placed in Ishanya (Northeast) water corner "
                                        "creates spiritual & mental distress.";
            } else if (fix.vastu_zone == VastuZoneId::NAIRUTYA) {
                fix.compliance = ComplianceStatus::DEFECT;
                fix.compliance_reason = "Defect: Kitchen stove in Nairutya (Southwest) causes family friction.";
            } else {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Neutral placement.";
            }
            break;

        case FixtureType::BED:
            if (fix.vastu_zone == VastuZoneId::NAIRUTYA) {
                fix.compliance = ComplianceStatus::COMPLIANT;
                fix.compliance_reason = "Ideal: Master bedroom in Nairutya (Southwest) earth zone provides grounded "
                                        "stability and leadership.";
            } else if (fix.vastu_zone == VastuZoneId::DAKSHINA || fix.vastu_zone == VastuZoneId::PASHCHIMA) {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Acceptable: South or West bedroom placement.";
            } else if (fix.vastu_zone == VastuZoneId::BRAHMASTHAN) {
                fix.compliance = ComplianceStatus::MAJOR_DEFECT;
                fix.compliance_reason =
                    "Major Defect: Bed in Brahmasthan (Center) blocks energy circulation in the home.";
            } else if (fix.vastu_zone == VastuZoneId::ISHANYA) {
                fix.compliance = ComplianceStatus::DEFECT;
                fix.compliance_reason = "Defect: Heavy bedroom furniture in Ishanya suppresses prosperity.";
            } else {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Neutral bedroom placement.";
            }
            break;

        case FixtureType::PUJA_ALTAR:
            if (fix.vastu_zone == VastuZoneId::ISHANYA) {
                fix.compliance = ComplianceStatus::COMPLIANT;
                fix.compliance_reason =
                    "Ideal: Puja altar in Ishanya (Northeast) sacred corner maximizes spiritual serenity.";
            } else if (fix.vastu_zone == VastuZoneId::PURVA || fix.vastu_zone == VastuZoneId::UTTARA) {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Acceptable: East or North divine altar placement.";
            } else if (fix.vastu_zone == VastuZoneId::NAIRUTYA || fix.vastu_zone == VastuZoneId::DAKSHINA) {
                fix.compliance = ComplianceStatus::MAJOR_DEFECT;
                fix.compliance_reason =
                    "Major Defect: Puja mandir in Southwest/South heavy zone diminishes auspiciousness.";
            } else {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Neutral altar placement.";
            }
            break;

        case FixtureType::TOILET:
            if (fix.vastu_zone == VastuZoneId::VAYU) {
                fix.compliance = ComplianceStatus::COMPLIANT;
                fix.compliance_reason = "Ideal: Toilet / waste disposal in Vayu (Northwest) air zone.";
            } else if (fix.vastu_zone == VastuZoneId::ISHANYA) {
                fix.compliance = ComplianceStatus::MAJOR_DEFECT;
                fix.compliance_reason =
                    "Major Defect: Toilet in Ishanya (Northeast) is a critical Vastu dosha blocking wealth and health.";
            } else if (fix.vastu_zone == VastuZoneId::BRAHMASTHAN) {
                fix.compliance = ComplianceStatus::MAJOR_DEFECT;
                fix.compliance_reason =
                    "Major Defect: Toilet in Brahmasthan destroys the core energy matrix of the building.";
            } else {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Acceptable toilet placement.";
            }
            break;

        case FixtureType::MAIN_DOOR:
            if (fix.vastu_zone == VastuZoneId::ISHANYA || fix.vastu_zone == VastuZoneId::PURVA ||
                fix.vastu_zone == VastuZoneId::UTTARA) {
                fix.compliance = ComplianceStatus::COMPLIANT;
                fix.compliance_reason = "Ideal: Main entrance in auspicious Northeast, East, or North portal.";
            } else if (fix.vastu_zone == VastuZoneId::NAIRUTYA) {
                fix.compliance = ComplianceStatus::MAJOR_DEFECT;
                fix.compliance_reason =
                    "Major Defect: Main entrance in Nairutya (Southwest) brings instability and unexpected losses.";
            } else {
                fix.compliance = ComplianceStatus::ACCEPTABLE;
                fix.compliance_reason = "Standard entrance placement.";
            }
            break;

        default:
            fix.compliance = ComplianceStatus::ACCEPTABLE;
            fix.compliance_reason = "Standard fixture placement.";
            break;
        }
    }
}

} // namespace CurvEngine::domain
