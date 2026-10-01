#include "curv/pattern/PatternValidation.hpp"

#include <cmath>

namespace CurvEngine::pattern {

LoopStats computeLoopStats(const std::vector<Point2D>& pts) {
    LoopStats stats;
    const auto n = pts.size();
    if (n < 3) {
        return stats;
    }

    // Shoelace area + perimeter over the closed polygon (wraparound closes the loop).
    double area2 = 0.0;
    double perim = 0.0;
    double sum_conf = 0.0;
    double cx = 0.0;
    double cy = 0.0;
    for (size_t i = 0; i < n; ++i) {
        const auto j = (i + 1) % n;
        const float xi = pts[i].x;
        const float yi = pts[i].y;
        const float xj = pts[j].x;
        const float yj = pts[j].y;
        const float cross = xi * yj - xj * yi;
        area2 += static_cast<double>(cross);
        const float dx = xj - xi;
        const float dy = yj - yi;
        perim += static_cast<double>(std::sqrt(dx * dx + dy * dy));
        cx += static_cast<double>(xi);
        cy += static_cast<double>(yi);
        sum_conf += static_cast<double>(pts[i].confidence);
    }

    stats.area_px = static_cast<float>(std::fabs(area2) * 0.5);
    stats.perimeter_px = static_cast<float>(perim);
    stats.mean_conf = static_cast<float>(sum_conf / static_cast<double>(n));
    if (stats.perimeter_px > 1e-6F) {
        stats.circularity = 4.0F * 3.14159265F * stats.area_px / (stats.perimeter_px * stats.perimeter_px);
    }
    if (stats.area_px <= 1e-6F) {
        return stats;
    }

    // 2nd-moment ellipse about the centroid: eigenvalues of the covariance matrix give
    // the squared semi-axis ratio, so elongation = sqrt(lambda1 / lambda2).
    cx /= static_cast<double>(n);
    cy /= static_cast<double>(n);
    double cxx = 0.0;
    double cyy = 0.0;
    double cxy = 0.0;
    for (const auto& p : pts) {
        const double dx = static_cast<double>(p.x) - cx;
        const double dy = static_cast<double>(p.y) - cy;
        cxx += dx * dx;
        cyy += dy * dy;
        cxy += dx * dy;
    }
    cxx /= static_cast<double>(n);
    cyy /= static_cast<double>(n);
    cxy /= static_cast<double>(n);
    const double tr = cxx + cyy;
    const double det = cxx * cyy - cxy * cxy;
    const double disc = std::sqrt(std::max(0.0, tr * tr * 0.25 - det));
    const double l2 = std::max(tr * 0.5 - disc, 1e-9);
    constexpr double kMaxElong = 50.0;
    stats.elongation = static_cast<float>(std::min(std::sqrt(std::max(tr * 0.5 + disc, 0.0) / l2), kMaxElong));
    return stats;
}

float maxChordDeviation(const std::vector<Point2D>& pts) {
    if (pts.size() < 3) {
        return 0.0F;
    }
    const Point2D& a = pts.front();
    const Point2D& b = pts.back();
    const float abx = b.x - a.x;
    const float aby = b.y - a.y;
    const float chord = std::sqrt(abx * abx + aby * aby);
    if (chord < 1e-3F) {
        return 0.0F;
    }
    float max_dev = 0.0F;
    for (const auto& p : pts) {
        const float apx = p.x - a.x;
        const float apy = p.y - a.y;
        const float dev = std::fabs(abx * apy - aby * apx) / chord; // |cross(ab, ap)| / |ab|
        max_dev = std::max(max_dev, dev);
    }
    return max_dev / chord;
}

} // namespace CurvEngine::pattern
