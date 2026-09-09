#include "curv/domain/WallSegmentExtractor.hpp"

#include <algorithm>
#include <cmath>
#include <numbers>

namespace CurvEngine::domain {

namespace {

constexpr float kPi = std::numbers::pi_v<float>;

float normalizeAngle180(float deg) {
    while (deg < 0.0f)
        deg += 180.0f;
    while (deg >= 180.0f)
        deg -= 180.0f;
    return deg;
}

void ramerDouglasPeucker(const std::vector<Point2D>& points, size_t start, size_t end, float epsilon,
                         std::vector<Point2D>& out) {
    if (end <= start) return;
    float dmax = 0.0f;
    size_t index = start;
    const auto& p1 = points[start];
    const auto& p2 = points[end];
    const float dx = p2.x - p1.x;
    const float dy = p2.y - p1.y;
    const float line_len = std::hypot(dx, dy);

    for (size_t i = start + 1; i < end; ++i) {
        const auto& p = points[i];
        float d = 0.0f;
        if (line_len < 1e-4f) {
            d = std::hypot(p.x - p1.x, p.y - p1.y);
        } else {
            d = std::abs(dy * p.x - dx * p.y + p2.x * p1.y - p2.y * p1.x) / line_len;
        }
        if (d > dmax) {
            index = i;
            dmax = d;
        }
    }

    if (dmax > epsilon) {
        ramerDouglasPeucker(points, start, index, epsilon, out);
        ramerDouglasPeucker(points, index, end, epsilon, out);
    } else {
        if (out.empty() || out.back().x != p1.x || out.back().y != p1.y) {
            out.push_back(p1);
        }
        out.push_back(p2);
    }
}

} // namespace

std::vector<WallSegment> WallSegmentExtractor::extractWalls(const RidgeGraph& ridge_graph) const {
    std::vector<WallSegment> raw_walls;
    raw_walls.reserve(ridge_graph.segments.size() * 2);

    int next_id = 1;
    for (const auto& seg : ridge_graph.segments) {
        if (seg.points.size() < 2) continue;

        std::vector<Point2D> simplified;
        simplified.reserve(seg.points.size());
        ramerDouglasPeucker(seg.points, 0, seg.points.size() - 1, 4.0f, simplified);

        if (simplified.size() < 2) continue;

        // Extract straight wall pieces from consecutive simplified vertices
        const size_t num_edges = simplified.size() - 1;
        for (size_t k = 0; k < num_edges; ++k) {
            const auto& p_start = simplified[k];
            const auto& p_end = simplified[k + 1];

            const float dx = p_end.x - p_start.x;
            const float dy = p_end.y - p_start.y;
            const float edge_len = std::hypot(dx, dy);

            if (edge_len < config_.min_wall_length) continue;

            const float angle_rad = std::atan2(dy, dx);
            const float angle_deg = normalizeAngle180(angle_rad * 180.0f / kPi);

            WallSegment wall;
            wall.id = next_id++;
            wall.start = p_start;
            wall.end = p_end;
            wall.length = edge_len;
            wall.orientation_deg = angle_deg;
            wall.thickness_px = config_.default_wall_thickness;
            wall.constituent_segment_ids.push_back(seg.id);

            // Check Manhattan alignment (0 or 90 degrees)
            if (std::abs(angle_deg - 0.0f) <= config_.manhattan_tolerance_deg ||
                std::abs(angle_deg - 180.0f) <= config_.manhattan_tolerance_deg) {
                wall.orientation_deg = 0.0f;
                wall.is_manhattan = true;
                const float mid_y = 0.5f * (wall.start.y + wall.end.y);
                wall.start.y = mid_y;
                wall.end.y = mid_y;
                if (wall.start.x > wall.end.x) std::swap(wall.start, wall.end);
            } else if (std::abs(angle_deg - 90.0f) <= config_.manhattan_tolerance_deg) {
                wall.orientation_deg = 90.0f;
                wall.is_manhattan = true;
                const float mid_x = 0.5f * (wall.start.x + wall.end.x);
                wall.start.x = mid_x;
                wall.end.x = mid_x;
                if (wall.start.y > wall.end.y) std::swap(wall.start, wall.end);
            } else {
                wall.is_manhattan = false;
            }

            raw_walls.push_back(wall);
        }
    }

    if (raw_walls.empty()) {
        return raw_walls;
    }

    // Collinear merging for Manhattan walls
    std::vector<WallSegment> merged_walls;
    std::vector<bool> merged(raw_walls.size(), false);

    for (size_t i = 0; i < raw_walls.size(); ++i) {
        if (merged[i]) continue;
        auto current = raw_walls[i];

        if (current.is_manhattan) {
            for (size_t j = i + 1; j < raw_walls.size(); ++j) {
                if (merged[j]) continue;
                const auto& candidate = raw_walls[j];

                if (!candidate.is_manhattan || candidate.orientation_deg != current.orientation_deg) {
                    continue;
                }

                if (current.orientation_deg == 0.0f) {
                    // Both horizontal: check y distance and x overlap / gap
                    if (std::abs(current.start.y - candidate.start.y) <= config_.collinear_dist_tolerance) {
                        const float gap = std::max(0.0f, std::max(current.start.x, candidate.start.x) -
                                                             std::min(current.end.x, candidate.end.x));
                        if (gap <= config_.merge_gap_threshold) {
                            current.start.x = std::min(current.start.x, candidate.start.x);
                            current.end.x = std::max(current.end.x, candidate.end.x);
                            current.length = current.end.x - current.start.x;
                            current.constituent_segment_ids.insert(current.constituent_segment_ids.end(),
                                                                   candidate.constituent_segment_ids.begin(),
                                                                   candidate.constituent_segment_ids.end());
                            merged[j] = true;
                        }
                    }
                } else if (current.orientation_deg == 90.0f) {
                    // Both vertical: check x distance and y overlap / gap
                    if (std::abs(current.start.x - candidate.start.x) <= config_.collinear_dist_tolerance) {
                        const float gap = std::max(0.0f, std::max(current.start.y, candidate.start.y) -
                                                             std::min(current.end.y, candidate.end.y));
                        if (gap <= config_.merge_gap_threshold) {
                            current.start.y = std::min(current.start.y, candidate.start.y);
                            current.end.y = std::max(current.end.y, candidate.end.y);
                            current.length = current.end.y - current.start.y;
                            current.constituent_segment_ids.insert(current.constituent_segment_ids.end(),
                                                                   candidate.constituent_segment_ids.begin(),
                                                                   candidate.constituent_segment_ids.end());
                            merged[j] = true;
                        }
                    }
                }
            }
        }

        merged_walls.push_back(current);
    }

    return merged_walls;
}

} // namespace CurvEngine::domain
