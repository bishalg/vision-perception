#include "curv/pattern/HostAttribution.hpp"

#include <cmath>

namespace CurvEngine::pattern {

HostAttributionResult attributeHost(const Point2D& c, const std::vector<HostPolyline>& hosts, float band_px) {
    HostAttributionResult best;
    float best_dist = -1.0F;

    for (const auto& host : hosts) {
        const auto n = host.points.size();
        if (n < 2) {
            continue;
        }

        // Cumulative arc length so the closest projection maps to a [0,1] position.
        float cumulative = 0.0F;
        float best_host_dist = -1.0F;
        float best_host_arc = -1.0F;

        for (size_t i = 0; i + 1 < n; ++i) {
            const Point2D& a = host.points[i];
            const Point2D& b = host.points[i + 1];
            const float abx = b.x - a.x;
            const float aby = b.y - a.y;
            const float ab2 = abx * abx + aby * aby;

            float t = 0.0F;
            if (ab2 > 1e-12F) {
                t = ((c.x - a.x) * abx + (c.y - a.y) * aby) / ab2;
                t = std::min(std::max(t, 0.0F), 1.0F);
            }
            const float px = a.x + abx * t;
            const float py = a.y + aby * t;
            const float dx = c.x - px;
            const float dy = c.y - py;
            const float d = std::sqrt(dx * dx + dy * dy);
            if (best_host_dist < 0.0F || d < best_host_dist) {
                best_host_dist = d;
                best_host_arc = cumulative + t * std::sqrt(ab2);
            }
            cumulative += std::sqrt(ab2);
        }

        if (best_host_dist < 0.0F) {
            continue;
        }
        if (best_dist < 0.0F || best_host_dist < best_dist) {
            best_dist = best_host_dist;
            best.host_curve_id = host.curve_id;
            best.host_name = host.name;
            best.dist_px = best_host_dist;
            const float total = cumulative;
            best.host_arc_s = (total > 1e-6F) ? best_host_arc / total : 0.0F;
        }
    }

    if (best_dist < 0.0F || best_dist > band_px) {
        best.host_curve_id = -1;
        best.host_name.clear();
        best.host_arc_s = -1.0F;
        if (best_dist >= 0.0F) {
            best.dist_px = best_dist;
        }
    }
    return best;
}

} // namespace CurvEngine::pattern
