#include "curv/temporal/DynamicTimeWarping.hpp"
#include <algorithm>
#include <cmath>
#include <limits>

namespace CurvEngine::temporal {

double DynamicTimeWarping::computeDistance(const std::vector<cv::Point2d>& trajectory_a,
                                           const std::vector<cv::Point2d>& trajectory_b, int sakoe_chiba_window) {
    if (trajectory_a.empty() && trajectory_b.empty()) {
        return 0.0;
    }
    if (trajectory_a.empty() || trajectory_b.empty()) {
        return std::numeric_limits<double>::infinity();
    }

    const size_t n = trajectory_a.size();
    const size_t m = trajectory_b.size();

    const double inf = std::numeric_limits<double>::infinity();
    std::vector<std::vector<double>> d(n + 1, std::vector<double>(m + 1, inf));
    d[0][0] = 0.0;

    for (size_t i = 1; i <= n; ++i) {
        for (size_t j = 1; j <= m; ++j) {
            if (sakoe_chiba_window > 0) {
                const int diff = std::abs(static_cast<int>(i) - static_cast<int>(j));
                if (diff > sakoe_chiba_window) {
                    continue;
                }
            }

            const double dx = trajectory_a[i - 1].x - trajectory_b[j - 1].x;
            const double dy = trajectory_a[i - 1].y - trajectory_b[j - 1].y;
            const double cost = std::hypot(dx, dy);

            const double min_prev = std::min({d[i - 1][j], d[i][j - 1], d[i - 1][j - 1]});
            if (min_prev < inf) {
                d[i][j] = cost + min_prev;
            }
        }
    }

    return d[n][m];
}

std::vector<cv::Point2d> DynamicTimeWarping::normalizeTrajectory(const std::vector<cv::Point2d>& points,
                                                                 size_t target_sample_count) {
    if (points.empty() || target_sample_count == 0) {
        return {};
    }
    if (points.size() == 1 || target_sample_count == 1) {
        return std::vector<cv::Point2d>(target_sample_count, points.front());
    }

    // 1. Resample along cumulative arc length
    std::vector<double> cum_dist(points.size(), 0.0);
    for (size_t i = 1; i < points.size(); ++i) {
        const double d = std::hypot(points[i].x - points[i - 1].x, points[i].y - points[i - 1].y);
        cum_dist[i] = cum_dist[i - 1] + d;
    }

    const double total_len = cum_dist.back();
    std::vector<cv::Point2d> resampled;
    resampled.reserve(target_sample_count);

    if (total_len <= 1e-6) {
        resampled.assign(target_sample_count, cv::Point2d(0.0, 0.0));
        return resampled;
    }

    const double step = total_len / static_cast<double>(target_sample_count - 1);
    resampled.push_back(points.front());

    size_t seg_idx = 0;
    for (size_t i = 1; i < target_sample_count - 1; ++i) {
        const double target_dist = static_cast<double>(i) * step;
        while (seg_idx + 1 < cum_dist.size() && cum_dist[seg_idx + 1] < target_dist) {
            seg_idx++;
        }

        const double seg_len = cum_dist[seg_idx + 1] - cum_dist[seg_idx];
        const double t = (seg_len > 1e-6) ? ((target_dist - cum_dist[seg_idx]) / seg_len) : 0.0;

        const double interp_x = points[seg_idx].x + t * (points[seg_idx + 1].x - points[seg_idx].x);
        const double interp_y = points[seg_idx].y + t * (points[seg_idx + 1].y - points[seg_idx].y);
        resampled.emplace_back(interp_x, interp_y);
    }
    resampled.push_back(points.back());

    // 2. Translate centroid to origin
    double sum_x = 0.0;
    double sum_y = 0.0;
    for (const auto& p : resampled) {
        sum_x += p.x;
        sum_y += p.y;
    }
    const double mean_x = sum_x / static_cast<double>(resampled.size());
    const double mean_y = sum_y / static_cast<double>(resampled.size());

    // 3. Scale to unit bounding radius
    double max_r = 0.0;
    for (auto& p : resampled) {
        p.x -= mean_x;
        p.y -= mean_y;
        max_r = std::max(max_r, std::hypot(p.x, p.y));
    }

    if (max_r > 1e-6) {
        for (auto& p : resampled) {
            p.x /= max_r;
            p.y /= max_r;
        }
    }

    return resampled;
}

GestureMatchResult DynamicTimeWarping::classifyGesture(const std::vector<cv::Point2d>& query_trajectory,
                                                       const std::vector<GestureTemplate>& templates,
                                                       double max_acceptance_distance) {
    GestureMatchResult result;
    if (query_trajectory.empty() || templates.empty()) {
        return result;
    }

    const auto norm_query = normalizeTrajectory(query_trajectory, 32);

    for (const auto& tmpl : templates) {
        const auto norm_tmpl = normalizeTrajectory(tmpl.trajectory, 32);
        const double dist = computeDistance(norm_query, norm_tmpl);

        if (dist < result.dtw_distance) {
            result.dtw_distance = dist;
            result.matched_gesture = tmpl.name;
        }
    }

    if (max_acceptance_distance > 0.0) {
        result.normalized_score = std::max(0.0, 1.0 - (result.dtw_distance / max_acceptance_distance));
    }

    return result;
}

} // namespace CurvEngine::temporal
