#include "curv/domain/MountMarkDetector.hpp"

#include <cmath>

#include "curv/pattern/MicroPatternDetector.hpp"

namespace CurvEngine::domain {

namespace {

const geometry::Zone* findZone(const std::vector<geometry::Zone>& zones, const std::string& id) {
    for (const auto& z : zones) {
        if (z.id == id) {
            return &z;
        }
    }
    return nullptr;
}

bool zoneHasBadMark(const RidgeGraph& graph, const geometry::Zone& zone, bool include_island) {
    // (a) Detected cross/star figure with centroid inside the mount
    // (plus island figures for mounts whose spec row lists them).
    const auto patterns = pattern::MicroPatternDetector::detectAll(graph);
    for (const auto& pat : patterns) {
        if (pat.type != pattern::PatternType::Cross && pat.type != pattern::PatternType::Star &&
            !(include_island && pat.type == pattern::PatternType::Island)) {
            continue;
        }
        if (geometry::ZoneIntersector::isPointInZone(pat.centroid, zone)) {
            return true;
        }
    }
    // (b) Grill proxy: dense junction cluster inside the mount.
    int junctions_in_zone = 0;
    for (const auto& junction : graph.junctions) {
        if (geometry::ZoneIntersector::isPointInZone(junction.position, zone)) {
            ++junctions_in_zone;
        }
    }
    if (junctions_in_zone >= MountMarkDetector::kGrillJunctionMin) {
        return true;
    }
    // (c) Horiz proxy: short near-horizontal ridge fragments fully inside.
    int horiz_in_zone = 0;
    for (const auto& seg : graph.segments) {
        if (seg.points.size() < 2 || seg.total_length >= MountMarkDetector::kHorizMaxLengthPx) {
            continue;
        }
        bool all_inside = true;
        for (const auto& pt : seg.points) {
            if (!geometry::ZoneIntersector::isPointInZone(pt, zone)) {
                all_inside = false;
                break;
            }
        }
        if (!all_inside) {
            continue;
        }
        const float dx = std::abs(seg.points.back().x - seg.points.front().x);
        const float dy = std::abs(seg.points.back().y - seg.points.front().y);
        if (dx >= dy) {
            ++horiz_in_zone;
        }
    }
    return horiz_in_zone >= MountMarkDetector::kHorizSegmentMin;
}

} // namespace

MountMarks MountMarkDetector::detect(const RidgeGraph& graph, const std::vector<geometry::Zone>& zones) {
    MountMarks marks;
    if (const auto* saturn = findZone(zones, "saturn_zone")) {
        marks.saturn_bad = zoneHasBadMark(graph, *saturn, false);
    }
    if (const auto* mercury = findZone(zones, "mercury_zone")) {
        marks.mercury_bad = zoneHasBadMark(graph, *mercury, true);
    }
    return marks;
}

} // namespace CurvEngine::domain
