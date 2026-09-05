#include "curv/geometry/ZoneIntersector.hpp"
#include <algorithm>
#include <opencv2/imgproc.hpp>
#if __has_include(<opencv2/geometry/2d.hpp>)
#include <opencv2/geometry/2d.hpp>
#endif

namespace CurvEngine::geometry {

namespace {

std::vector<cv::Point2f> toCvPoints(const std::vector<Point2D>& poly) {
    std::vector<cv::Point2f> pts;
    pts.reserve(poly.size());
    for (const auto& p : poly) {
        pts.emplace_back(p.x, p.y);
    }
    return pts;
}

} // anonymous namespace

bool ZoneIntersector::isPointInZone(const Point2D& point, const Zone& zone) {
    if (zone.polygon.size() < 3) {
        return false;
    }
    const auto pts = toCvPoints(zone.polygon);
    const double result = cv::pointPolygonTest(pts, cv::Point2f(point.x, point.y), false);
    return result >= 0.0;
}

std::string ZoneIntersector::findPrimaryZone(const CurveSegment& segment, const std::vector<Zone>& zones,
                                             float min_overlap_ratio) {
    if (segment.points.empty() || zones.empty()) {
        return "unknown";
    }

    const auto overlaps = computeZoneOverlap(segment, zones);
    if (overlaps.empty()) {
        return "unknown";
    }

    if (overlaps.front().second >= min_overlap_ratio) {
        return overlaps.front().first;
    }

    return "unknown";
}

std::vector<std::pair<std::string, float>> ZoneIntersector::computeZoneOverlap(const CurveSegment& segment,
                                                                               const std::vector<Zone>& zones) {
    std::vector<std::pair<std::string, float>> result;
    if (segment.points.empty() || zones.empty()) {
        return result;
    }

    const float total_pts = static_cast<float>(segment.points.size());

    for (const auto& zone : zones) {
        if (zone.polygon.size() < 3) continue;

        const auto pts = toCvPoints(zone.polygon);
        int inside_count = 0;

        for (const auto& pt : segment.points) {
            if (cv::pointPolygonTest(pts, cv::Point2f(pt.x, pt.y), false) >= 0.0) {
                inside_count++;
            }
        }

        if (inside_count > 0) {
            const float ratio = static_cast<float>(inside_count) / total_pts;
            result.emplace_back(zone.id, ratio);
        }
    }

    std::ranges::sort(result, [](const auto& a, const auto& b) { return a.second > b.second; });

    return result;
}

} // namespace CurvEngine::geometry
