#include "curv/domain/WallJunctionGraph.hpp"

#include <algorithm>
#include <cmath>

namespace CurvEngine::domain {

int WallJunctionGraph::findOrCreateJunction(const Point2D& pt, int wall_id) {
    for (auto& j : junctions_) {
        const float dx = j.position.x - pt.x;
        const float dy = j.position.y - pt.y;
        if (std::sqrt(dx * dx + dy * dy) <= config_.snap_distance) {
            if (std::find(j.connected_wall_ids.begin(), j.connected_wall_ids.end(), wall_id) ==
                j.connected_wall_ids.end()) {
                j.connected_wall_ids.push_back(wall_id);
            }
            return j.id;
        }
    }

    WallJunction new_j;
    new_j.id = static_cast<int>(junctions_.size()) + 1;
    new_j.position = pt;
    new_j.connected_wall_ids.push_back(wall_id);
    new_j.type = WallJunctionType::TERMINAL_END;
    junctions_.push_back(new_j);
    return new_j.id;
}

void WallJunctionGraph::build(std::vector<WallSegment>& walls) {
    junctions_.clear();
    walls_ = walls;

    // Step 1: Initial endpoint clustering
    for (auto& w : walls_) {
        const int j_start = findOrCreateJunction(w.start, w.id);
        const int j_end = findOrCreateJunction(w.end, w.id);

        // Snap wall endpoints to junction positions
        for (const auto& j : junctions_) {
            if (j.id == j_start) w.start = j.position;
            if (j.id == j_end) w.end = j.position;
        }
    }

    // Step 2: Detect T-junctions where a junction lies along a wall's interior
    std::vector<WallSegment> refined_walls;
    refined_walls.reserve(walls_.size() * 2);
    int next_wall_id = static_cast<int>(walls_.size()) + 100;

    for (const auto& w : walls_) {
        std::vector<Point2D> split_points;

        for (auto& j : junctions_) {
            // If already connected to endpoints, skip
            if (std::find(j.connected_wall_ids.begin(), j.connected_wall_ids.end(), w.id) !=
                j.connected_wall_ids.end()) {
                continue;
            }

            if (w.is_manhattan && w.orientation_deg == 0.0f) {
                // Horizontal wall: check if junction is along the segment
                const float min_x = std::min(w.start.x, w.end.x);
                const float max_x = std::max(w.start.x, w.end.x);
                if (std::abs(j.position.y - w.start.y) <= config_.snap_distance &&
                    j.position.x > min_x + config_.snap_distance && j.position.x < max_x - config_.snap_distance) {
                    j.position.y = w.start.y;
                    j.connected_wall_ids.push_back(w.id);
                    split_points.push_back(j.position);
                }
            } else if (w.is_manhattan && w.orientation_deg == 90.0f) {
                // Vertical wall: check if junction is along the segment
                const float min_y = std::min(w.start.y, w.end.y);
                const float max_y = std::max(w.start.y, w.end.y);
                if (std::abs(j.position.x - w.start.x) <= config_.snap_distance &&
                    j.position.y > min_y + config_.snap_distance && j.position.y < max_y - config_.snap_distance) {
                    j.position.x = w.start.x;
                    j.connected_wall_ids.push_back(w.id);
                    split_points.push_back(j.position);
                }
            }
        }

        if (split_points.empty()) {
            refined_walls.push_back(w);
        } else {
            // Split wall into sub-segments
            if (w.orientation_deg == 0.0f) {
                std::sort(split_points.begin(), split_points.end(),
                          [](const Point2D& a, const Point2D& b) { return a.x < b.x; });
            } else {
                std::sort(split_points.begin(), split_points.end(),
                          [](const Point2D& a, const Point2D& b) { return a.y < b.y; });
            }

            Point2D curr_start = w.start;
            for (const auto& sp : split_points) {
                WallSegment sub;
                sub.id = next_wall_id++;
                sub.start = curr_start;
                sub.end = sp;
                sub.length = std::sqrt((sub.end.x - sub.start.x) * (sub.end.x - sub.start.x) +
                                       (sub.end.y - sub.start.y) * (sub.end.y - sub.start.y));
                sub.orientation_deg = w.orientation_deg;
                sub.thickness_px = w.thickness_px;
                sub.is_manhattan = w.is_manhattan;
                sub.constituent_segment_ids = w.constituent_segment_ids;
                refined_walls.push_back(sub);
                curr_start = sp;
            }
            WallSegment last_sub;
            last_sub.id = next_wall_id++;
            last_sub.start = curr_start;
            last_sub.end = w.end;
            last_sub.length = std::sqrt((last_sub.end.x - last_sub.start.x) * (last_sub.end.x - last_sub.start.x) +
                                        (last_sub.end.y - last_sub.start.y) * (last_sub.end.y - last_sub.start.y));
            last_sub.orientation_deg = w.orientation_deg;
            last_sub.thickness_px = w.thickness_px;
            last_sub.is_manhattan = w.is_manhattan;
            last_sub.constituent_segment_ids = w.constituent_segment_ids;
            refined_walls.push_back(last_sub);
        }
    }

    walls_ = refined_walls;

    // Step 3: Re-associate walls to junctions and classify
    for (auto& j : junctions_) {
        j.connected_wall_ids.clear();
        for (const auto& w : walls_) {
            const float d_start = std::hypot(w.start.x - j.position.x, w.start.y - j.position.y);
            const float d_end = std::hypot(w.end.x - j.position.x, w.end.y - j.position.y);
            if (d_start <= config_.snap_distance || d_end <= config_.snap_distance) {
                j.connected_wall_ids.push_back(w.id);
            }
        }

        const size_t deg = j.connected_wall_ids.size();
        if (deg == 1) {
            j.type = WallJunctionType::TERMINAL_END;
        } else if (deg == 2) {
            j.type = WallJunctionType::CORNER_L;
        } else if (deg == 3) {
            j.type = WallJunctionType::TEE_T;
        } else if (deg >= 4) {
            j.type = WallJunctionType::CROSS_X;
        }
    }
}

} // namespace CurvEngine::domain
