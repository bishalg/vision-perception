#pragma once

#include "curv/GeometryTypes.hpp"
#include <string>
#include <vector>

namespace CurvEngine::domain {

/**
 * @brief Categorization of topological wall junctions.
 */
enum class WallJunctionType {
    TERMINAL_END, // Degree 1: doorway opening, wall terminus
    CORNER_L,     // Degree 2: corner meeting at approx 90 degrees
    TEE_T,        // Degree 3: partition wall joining a boundary or exterior wall
    CROSS_X       // Degree 4: crossing partition walls
};

/**
 * @brief Rectified wall segment extracted from curvilinear ridges.
 */
struct WallSegment {
    int id{0};
    Point2D start;
    Point2D end;
    float length{0.0f};
    float orientation_deg{0.0f}; // 0 = horizontal (East/West), 90 = vertical (North/South)
    float thickness_px{0.0f};
    bool is_manhattan{true}; // snapped to 0 or 90 deg (or custom rotation)
    std::vector<int> constituent_segment_ids;
};

/**
 * @brief Topological junction where one or more wall segments meet.
 */
struct WallJunction {
    int id{0};
    Point2D position;
    WallJunctionType type{WallJunctionType::TERMINAL_END};
    std::vector<int> connected_wall_ids;
};

/**
 * @brief 9-Pad classical Vastu Purusha Mandala zones.
 */
enum class VastuZoneId {
    BRAHMASTHAN, // Center - Akasha (Ether / Space)
    ISHANYA,     // Northeast - Jala (Water)
    PURVA,       // East - Surya / Agni (Light / Energy)
    AGNI,        // Southeast - Tejas (Fire)
    DAKSHINA,    // South - Yama (Mars)
    NAIRUTYA,    // Southwest - Prithvi (Earth)
    PASHCHIMA,   // West - Varuna (Saturn)
    VAYU,        // Northwest - Vayu (Air)
    UTTARA       // North - Kubera (Mercury / Wealth)
};

/**
 * @brief Metadata and spatial boundary for a single Vastu zone in the 9-pad grid.
 */
struct VastuZone {
    VastuZoneId id{VastuZoneId::BRAHMASTHAN};
    std::string name;
    std::string deity;
    std::string element;
    std::vector<Point2D> polygon;
    float area_px{0.0f};
};

/**
 * @brief Extracted closed room polygon representing an individual interior space.
 */
struct RoomPolygon {
    int id{0};
    std::string label;
    std::vector<Point2D> vertices;
    Point2D centroid;
    float bounding_width{0.0f};
    float bounding_height{0.0f};
    float perimeter{0.0f};
    float area_px{0.0f};
    float area_sqm{0.0f}; // calibrated when pixels_per_meter > 0
    VastuZoneId primary_vastu_zone{VastuZoneId::BRAHMASTHAN};
    float vastu_zone_overlap_ratio{0.0f};
    std::vector<int> bounding_wall_ids;
};

/**
 * @brief Architectural fixtures / furniture objects detected or placed in the floor plan.
 */
enum class FixtureType { BED, STOVE, TOILET, PUJA_ALTAR, MAIN_DOOR, WATER_TANK, STUDY_DESK, CUSTOM };

enum class ComplianceStatus { COMPLIANT, ACCEPTABLE, DEFECT, MAJOR_DEFECT };

struct FixtureItem {
    int id{0};
    FixtureType type{FixtureType::CUSTOM};
    std::string label;
    Point2D position;
    float width{0.0f};
    float height{0.0f};
    int room_id{-1};
    VastuZoneId vastu_zone{VastuZoneId::BRAHMASTHAN};
    ComplianceStatus compliance{ComplianceStatus::ACCEPTABLE};
    std::string compliance_reason;
};

} // namespace CurvEngine::domain
