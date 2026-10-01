#pragma once

#include "curv/GeometryTypes.hpp"

#include <string>
#include <vector>

namespace CurvEngine::pattern {

/**
 * @brief A named major-line polyline offered as a candidate host for a detected sign.
 *        curve_id is caller-assigned (e.g. 0=heart, 1=head, 2=life, 3=fate).
 */
struct HostPolyline {
    int curve_id{0};
    std::string name;
    std::vector<Point2D> points;
};

/**
 * @brief Result of host-line attribution for one detected sign.
 *        attributed() is false when the sign centroid lies farther than the band from
 *        every host polyline (a mount-region mark, not a line mark).
 */
struct HostAttributionResult {
    int host_curve_id{-1};
    std::string host_name;
    float host_arc_s{-1.0F}; // relative position [0, 1] along the host polyline
    float dist_px{-1.0F};    // distance to the best host regardless of band acceptance

    [[nodiscard]] bool attributed() const { return host_curve_id >= 0; }
};

/**
 * @brief Attributes a point to the nearest host polyline within `band_px`.
 *        Arc position uses cumulative polyline length at the closest projection.
 */
[[nodiscard]] HostAttributionResult attributeHost(const Point2D& c, const std::vector<HostPolyline>& hosts,
                                                  float band_px);

} // namespace CurvEngine::pattern
