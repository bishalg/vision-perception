#include "curv/domain/CreaseGeometry.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <unordered_set>

namespace CurvEngine::domain {

namespace {

inline float dist2d(const Point2D& a, const Point2D& b) {
    return std::hypot(a.x - b.x, a.y - b.y);
}

// Orientation test for 2D line intersection: ccw > 0, cw < 0, collinear == 0
inline float cross2d(const Point2D& a, const Point2D& b, const Point2D& c) {
    return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
}

bool segmentsIntersect(const Point2D& p1, const Point2D& p2, const Point2D& p3, const Point2D& p4) {
    const float d1 = cross2d(p3, p4, p1);
    const float d2 = cross2d(p3, p4, p2);
    const float d3 = cross2d(p1, p2, p3);
    const float d4 = cross2d(p1, p2, p4);

    if (((d1 > 0.0f && d2 < 0.0f) || (d1 < 0.0f && d2 > 0.0f)) &&
        ((d3 > 0.0f && d4 < 0.0f) || (d3 < 0.0f && d4 > 0.0f))) {
        return true;
    }
    return false;
}

std::string findPointZone(const Point2D& pt, const std::vector<geometry::Zone>& zones) {
    for (const auto& zone : zones) {
        if (geometry::ZoneIntersector::isPointInZone(pt, zone)) {
            return zone.id;
        }
    }
    return "unknown";
}

} // anonymous namespace

float CreaseGeometry::computeArcLength(const std::vector<Point2D>& points) {
    if (points.size() < 2) {
        return 0.0f;
    }
    float length = 0.0f;
    for (size_t i = 1; i < points.size(); ++i) {
        length += dist2d(points[i - 1], points[i]);
    }
    return length;
}

float CreaseGeometry::computeChordLength(const std::vector<Point2D>& points) {
    if (points.size() < 2) {
        return 0.0f;
    }
    return dist2d(points.front(), points.back());
}

float CreaseGeometry::computeTortuosity(float arc_length, float chord_length) {
    if (chord_length < 1e-3f) {
        return 1.0f;
    }
    return std::max(1.0f, arc_length / chord_length);
}

float CreaseGeometry::computeAngleDeg(const Point2D& start, const Point2D& end) {
    const float dx = std::abs(end.x - start.x);
    const float dy = std::abs(end.y - start.y);
    constexpr float kRadToDeg = 180.0f / 3.14159265358979323846f;
    return std::atan2(dy, dx) * kRadToDeg;
}

float CreaseGeometry::computeMinDistance(const std::vector<Point2D>& a, const std::vector<Point2D>& b) {
    if (a.empty() || b.empty()) {
        return -1.0f;
    }
    float min_dist = std::numeric_limits<float>::max();
    for (const auto& pa : a) {
        for (const auto& pb : b) {
            const float d = dist2d(pa, pb);
            if (d < min_dist) {
                min_dist = d;
            }
        }
    }
    return min_dist;
}

int CreaseGeometry::countForks(const std::vector<int>& segment_ids, const std::vector<Junction>& junctions) {
    if (segment_ids.empty() || junctions.empty()) {
        return 0;
    }
    const std::unordered_set<int> id_set(segment_ids.begin(), segment_ids.end());
    int forks = 0;
    for (const auto& junc : junctions) {
        for (int conn_id : junc.connected_segment_ids) {
            if (id_set.contains(conn_id)) {
                forks++;
                break;
            }
        }
    }
    return forks;
}

int CreaseGeometry::countIntersections(const std::vector<Point2D>& a, const std::vector<Point2D>& b) {
    if (a.size() < 2 || b.size() < 2) {
        return 0;
    }
    int count = 0;
    for (size_t i = 1; i < a.size(); ++i) {
        for (size_t j = 1; j < b.size(); ++j) {
            if (segmentsIntersect(a[i - 1], a[i], b[j - 1], b[j])) {
                count++;
            }
        }
    }
    return count;
}

CreaseMetrics CreaseGeometry::computeMetrics(const ChainedCreaseLine& chain, const RidgeGraph& graph,
                                             const std::vector<geometry::Zone>& zones) {
    CreaseMetrics metrics;
    if (chain.points.empty()) {
        return metrics;
    }

    metrics.arc_length_px = (chain.total_length > 0.0f) ? chain.total_length : computeArcLength(chain.points);
    metrics.chord_length_px = computeChordLength(chain.points);
    metrics.tortuosity = computeTortuosity(metrics.arc_length_px, metrics.chord_length_px);
    metrics.mean_angle_deg = computeAngleDeg(chain.points.front(), chain.points.back());
    metrics.gap_count =
        (chain.constituent_segment_ids.size() > 1) ? static_cast<int>(chain.constituent_segment_ids.size() - 1) : 0;
    metrics.fork_count = countForks(chain.constituent_segment_ids, graph.junctions);

    // Compute mean intensity and mean width across constituent segments if present in graph
    const std::unordered_set<int> seg_set(chain.constituent_segment_ids.begin(), chain.constituent_segment_ids.end());
    float sum_intensity = 0.0f;
    size_t sample_count = 0;

    for (const auto& seg : graph.segments) {
        if (seg_set.contains(seg.id)) {
            for (const auto& pt : seg.points) {
                sum_intensity += pt.intensity;
                sample_count++;
            }
        }
    }

    if (sample_count > 0) {
        metrics.mean_intensity = sum_intensity / static_cast<float>(sample_count);
        metrics.mean_width_px = (chain.total_length > 0.0f && sample_count > 1)
                                    ? std::max(1.0f, chain.total_length / static_cast<float>(sample_count))
                                    : 2.0f;
    } else {
        metrics.mean_intensity = chain.mean_intensity;
        metrics.mean_width_px = 2.0f;
    }

    metrics.start_zone = findPointZone(chain.points.front(), zones);
    metrics.end_zone = findPointZone(chain.points.back(), zones);

    return metrics;
}

} // namespace CurvEngine::domain
