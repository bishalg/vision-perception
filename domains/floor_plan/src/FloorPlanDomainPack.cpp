#include "curv/domains/FloorPlanDomainPack.hpp"

#include <algorithm>
#include <iomanip>
#include <sstream>

namespace CurvEngine::domains {

namespace {

std::string vastuZoneToString(domain::VastuZoneId id) {
    switch (id) {
    case domain::VastuZoneId::BRAHMASTHAN:
        return "BRAHMASTHAN";
    case domain::VastuZoneId::ISHANYA:
        return "ISHANYA";
    case domain::VastuZoneId::PURVA:
        return "PURVA";
    case domain::VastuZoneId::AGNI:
        return "AGNI";
    case domain::VastuZoneId::DAKSHINA:
        return "DAKSHINA";
    case domain::VastuZoneId::NAIRUTYA:
        return "NAIRUTYA";
    case domain::VastuZoneId::PASHCHIMA:
        return "PASHCHIMA";
    case domain::VastuZoneId::VAYU:
        return "VAYU";
    case domain::VastuZoneId::UTTARA:
        return "UTTARA";
    }
    return "UNKNOWN";
}

domain::FixtureType stringToFixtureType(const std::string& str) {
    if (str == "BED" || str == "bed") return domain::FixtureType::BED;
    if (str == "STOVE" || str == "stove" || str == "kitchen") return domain::FixtureType::STOVE;
    if (str == "TOILET" || str == "toilet" || str == "wc") return domain::FixtureType::TOILET;
    if (str == "PUJA_ALTAR" || str == "puja" || str == "mandir") return domain::FixtureType::PUJA_ALTAR;
    if (str == "MAIN_DOOR" || str == "entrance" || str == "door") return domain::FixtureType::MAIN_DOOR;
    if (str == "WATER_TANK" || str == "tank") return domain::FixtureType::WATER_TANK;
    if (str == "STUDY_DESK" || str == "desk") return domain::FixtureType::STUDY_DESK;
    return domain::FixtureType::CUSTOM;
}

std::string fixtureTypeToString(domain::FixtureType type) {
    switch (type) {
    case domain::FixtureType::BED:
        return "BED";
    case domain::FixtureType::STOVE:
        return "STOVE";
    case domain::FixtureType::TOILET:
        return "TOILET";
    case domain::FixtureType::PUJA_ALTAR:
        return "PUJA_ALTAR";
    case domain::FixtureType::MAIN_DOOR:
        return "MAIN_DOOR";
    case domain::FixtureType::WATER_TANK:
        return "WATER_TANK";
    case domain::FixtureType::STUDY_DESK:
        return "STUDY_DESK";
    case domain::FixtureType::CUSTOM:
        return "CUSTOM";
    }
    return "CUSTOM";
}

std::string complianceToString(domain::ComplianceStatus status) {
    switch (status) {
    case domain::ComplianceStatus::COMPLIANT:
        return "COMPLIANT";
    case domain::ComplianceStatus::ACCEPTABLE:
        return "ACCEPTABLE";
    case domain::ComplianceStatus::DEFECT:
        return "DEFECT";
    case domain::ComplianceStatus::MAJOR_DEFECT:
        return "MAJOR_DEFECT";
    }
    return "ACCEPTABLE";
}

} // anonymous namespace

DomainOutput FloorPlanDomainPack::process(const DomainInput& input) {
    DomainOutput output;
    output.domain_name = name();

    // 1. Read metadata
    float north_deg = 0.0f;
    if (input.frame.metadata.contains("north_deg")) {
        try {
            north_deg = std::stof(input.frame.metadata.at("north_deg"));
        } catch (...) {
        }
    }

    float scale_px_per_m = 0.0f;
    if (input.frame.metadata.contains("scale_px_per_m")) {
        try {
            scale_px_per_m = std::stof(input.frame.metadata.at("scale_px_per_m"));
        } catch (...) {
        }
    }

    // 2. Extract and rectify wall segments
    domain::WallExtractionConfig wall_cfg;
    domain::WallSegmentExtractor extractor(wall_cfg);
    auto walls = extractor.extractWalls(input.ridges);

    // 3. Build topological junction graph
    domain::JunctionGraphConfig junc_cfg;
    domain::WallJunctionGraph junc_graph(junc_cfg);
    junc_graph.build(walls);
    const auto& snapped_walls = junc_graph.snappedWalls();
    const auto& junctions = junc_graph.junctions();

    // 4. Detect closed room polygons
    domain::RoomDetectionConfig room_cfg;
    room_cfg.pixels_per_meter = scale_px_per_m;
    domain::RoomPolygonDetector room_detector(room_cfg);
    auto rooms = room_detector.detectRooms(snapped_walls, junctions);

    // 5. Compute total building envelope
    float min_x = 1e9f, max_x = -1e9f;
    float min_y = 1e9f, max_y = -1e9f;

    for (const auto& w : snapped_walls) {
        min_x = std::min({min_x, w.start.x, w.end.x});
        max_x = std::max({max_x, w.start.x, w.end.x});
        min_y = std::min({min_y, w.start.y, w.end.y});
        max_y = std::max({max_y, w.start.y, w.end.y});
    }

    if (snapped_walls.empty() || min_x >= max_x || min_y >= max_y) {
        output.domain_payload = {{"status", "insufficient_walls"}, {"wall_count", 0}, {"room_count", 0}};
        return output;
    }

    // 6. Generate 9-pad Vastu Purusha Mandala
    domain::VastuConfig vastu_cfg;
    vastu_cfg.north_orientation_deg = north_deg;
    domain::VastuMandalaOverlay mandala_overlay(vastu_cfg);
    const auto zones = mandala_overlay.generateMandala(min_x, min_y, max_x, max_y);

    // 7. Assign rooms to primary Vastu zones
    mandala_overlay.assignRoomsToZones(rooms, zones);

    // 8. Ingest and evaluate architectural fixtures
    std::vector<domain::FixtureItem> fixtures;
    if (input.frame.metadata.contains("fixtures_json")) {
        try {
            auto f_json = nlohmann::json::parse(input.frame.metadata.at("fixtures_json"));
            if (f_json.is_array()) {
                int f_id = 1;
                for (const auto& item : f_json) {
                    domain::FixtureItem fix;
                    fix.id = f_id++;
                    fix.type = stringToFixtureType(item.value("type", "CUSTOM"));
                    fix.label = item.value("label", fixtureTypeToString(fix.type));
                    fix.position.x = item.value("x", 0.0f);
                    fix.position.y = item.value("y", 0.0f);
                    fixtures.push_back(fix);
                }
            }
        } catch (...) {
        }
    }

    mandala_overlay.evaluateFixtures(fixtures, zones, rooms);

    // 9. Calculate Vastu compliance scoring
    int compliant_count = 0;
    int defect_count = 0;
    int major_defect_count = 0;

    for (const auto& fix : fixtures) {
        if (fix.compliance == domain::ComplianceStatus::COMPLIANT) {
            compliant_count++;
        } else if (fix.compliance == domain::ComplianceStatus::DEFECT) {
            defect_count++;
        } else if (fix.compliance == domain::ComplianceStatus::MAJOR_DEFECT) {
            major_defect_count++;
        }
    }

    float compliance_score = 100.0f;
    if (!fixtures.empty()) {
        const float penalty = static_cast<float>(defect_count * 15 + major_defect_count * 30);
        compliance_score = std::max(0.0f, 100.0f - penalty);
    }

    // 10. Assemble JSON domain payload
    nlohmann::json payload;
    payload["status"] = "success";
    payload["north_orientation_deg"] = north_deg;
    payload["pixels_per_meter"] = scale_px_per_m;
    payload["building_envelope"] = {{"min_x", min_x}, {"min_y", min_y},         {"max_x", max_x},
                                    {"max_y", max_y}, {"width", max_x - min_x}, {"height", max_y - min_y}};

    // Walls
    nlohmann::json walls_arr = nlohmann::json::array();
    float total_wall_len = 0.0f;
    for (const auto& w : snapped_walls) {
        total_wall_len += w.length;
        walls_arr.push_back({{"id", w.id},
                             {"start", {{"x", w.start.x}, {"y", w.start.y}}},
                             {"end", {{"x", w.end.x}, {"y", w.end.y}}},
                             {"length", w.length},
                             {"orientation_deg", w.orientation_deg},
                             {"thickness_px", w.thickness_px},
                             {"is_manhattan", w.is_manhattan}});
    }
    payload["wall_count"] = snapped_walls.size();
    payload["walls"] = walls_arr;

    // Junctions
    nlohmann::json junc_arr = nlohmann::json::array();
    for (const auto& j : junctions) {
        std::string j_type_str = "TERMINAL_END";
        if (j.type == domain::WallJunctionType::CORNER_L)
            j_type_str = "CORNER_L";
        else if (j.type == domain::WallJunctionType::TEE_T)
            j_type_str = "TEE_T";
        else if (j.type == domain::WallJunctionType::CROSS_X)
            j_type_str = "CROSS_X";

        junc_arr.push_back({{"id", j.id},
                            {"position", {{"x", j.position.x}, {"y", j.position.y}}},
                            {"type", j_type_str},
                            {"connected_wall_ids", j.connected_wall_ids}});
    }
    payload["junction_count"] = junctions.size();
    payload["junctions"] = junc_arr;

    // Rooms
    nlohmann::json rooms_arr = nlohmann::json::array();
    float total_floor_area_px = 0.0f;
    float total_floor_area_sqm = 0.0f;
    for (const auto& rm : rooms) {
        total_floor_area_px += rm.area_px;
        total_floor_area_sqm += rm.area_sqm;

        nlohmann::json verts = nlohmann::json::array();
        for (const auto& v : rm.vertices) {
            verts.push_back({{"x", v.x}, {"y", v.y}});
        }

        rooms_arr.push_back({{"id", rm.id},
                             {"label", rm.label},
                             {"area_px", rm.area_px},
                             {"area_sqm", rm.area_sqm},
                             {"perimeter", rm.perimeter},
                             {"bounding_width", rm.bounding_width},
                             {"bounding_height", rm.bounding_height},
                             {"centroid", {{"x", rm.centroid.x}, {"y", rm.centroid.y}}},
                             {"primary_vastu_zone", vastuZoneToString(rm.primary_vastu_zone)},
                             {"vertices", verts},
                             {"bounding_wall_ids", rm.bounding_wall_ids}});
    }
    payload["room_count"] = rooms.size();
    payload["rooms"] = rooms_arr;

    // Vastu Mandala Zones
    nlohmann::json zones_arr = nlohmann::json::array();
    for (const auto& z : zones) {
        nlohmann::json poly = nlohmann::json::array();
        for (const auto& p : z.polygon) {
            poly.push_back({{"x", p.x}, {"y", p.y}});
        }
        zones_arr.push_back({{"id", vastuZoneToString(z.id)},
                             {"name", z.name},
                             {"deity", z.deity},
                             {"element", z.element},
                             {"polygon", poly}});
    }
    payload["vastu_zones"] = zones_arr;

    // Fixtures
    nlohmann::json fix_arr = nlohmann::json::array();
    for (const auto& fix : fixtures) {
        fix_arr.push_back({{"id", fix.id},
                           {"type", fixtureTypeToString(fix.type)},
                           {"label", fix.label},
                           {"position", {{"x", fix.position.x}, {"y", fix.position.y}}},
                           {"room_id", fix.room_id},
                           {"vastu_zone", vastuZoneToString(fix.vastu_zone)},
                           {"compliance", complianceToString(fix.compliance)},
                           {"compliance_reason", fix.compliance_reason}});
    }
    payload["fixtures"] = fix_arr;

    // Vastu summary
    payload["vastu_summary"] = {{"compliance_score_percent", compliance_score},
                                {"compliant_fixtures", compliant_count},
                                {"defect_fixtures", defect_count},
                                {"major_defect_fixtures", major_defect_count}};

    // Grounded VLM prompt
    std::ostringstream prompt_ss;
    prompt_ss << "You are an expert Vedic Vastu Shastra architect analyzing a floor plan.\n\n"
              << "BUILDING OVERVIEW:\n"
              << "- North Orientation: " << north_deg << " degrees\n"
              << "- Extracted Wall Count: " << snapped_walls.size() << "\n"
              << "- Delineated Room Count: " << rooms.size() << "\n"
              << "- Total Measured Floor Area: " << std::fixed << std::setprecision(1) << total_floor_area_px
              << " px^2";
    if (scale_px_per_m > 0.0f) {
        prompt_ss << " (" << total_floor_area_sqm << " m^2)";
    }
    prompt_ss << "\n\nROOMS & VASTU ZONES:\n";
    for (const auto& rm : rooms) {
        prompt_ss << "- " << rm.label << ": Area=" << static_cast<int>(rm.area_px)
                  << " px^2, Primary Zone=" << vastuZoneToString(rm.primary_vastu_zone) << "\n";
    }
    if (!fixtures.empty()) {
        prompt_ss << "\nFIXTURE COMPLIANCE FINDINGS:\n";
        for (const auto& fix : fixtures) {
            prompt_ss << "- " << fix.label << " in " << vastuZoneToString(fix.vastu_zone) << " -> "
                      << complianceToString(fix.compliance) << ": " << fix.compliance_reason << "\n";
        }
    }
    prompt_ss << "\nProvide architectural recommendations and remedial measures based on classical Vastu Shastra.";
    payload["grounded_prompt"] = prompt_ss.str();

    output.domain_payload = payload;

    // Evidence records
    output.domain_evidence.emplace_back(
        "ev_floorplan_walls", "wall_topology", 1.0,
        nlohmann::json{{"wall_count", snapped_walls.size()}, {"total_length", total_wall_len}});

    output.domain_evidence.emplace_back(
        "ev_floorplan_rooms", "room_polygons", 1.0,
        nlohmann::json{{"room_count", rooms.size()}, {"total_area_px", total_floor_area_px}});

    output.domain_evidence.emplace_back(
        "ev_vastu_compliance", "vastu_evaluation", 1.0,
        nlohmann::json{{"compliance_score", compliance_score}, {"defects", defect_count + major_defect_count}});

    return output;
}

} // namespace CurvEngine::domains
