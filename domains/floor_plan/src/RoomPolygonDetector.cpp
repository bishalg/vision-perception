#include "curv/domain/RoomPolygonDetector.hpp"

#include <algorithm>
#include <cmath>
#include <map>
#include <numbers>
#include <set>

namespace CurvEngine::domain {

namespace {

struct HalfEdge {
    int from_junction{0};
    int to_junction{0};
    int wall_id{0};
    float angle{0.0f}; // polar angle in [-pi, pi]
};

float computeSignedArea(const std::vector<Point2D>& poly) {
    if (poly.size() < 3) return 0.0f;
    double sum = 0.0;
    const size_t n = poly.size();
    for (size_t i = 0; i < n; ++i) {
        const auto& p1 = poly[i];
        const auto& p2 = poly[(i + 1) % n];
        sum += static_cast<double>(p1.x) * static_cast<double>(p2.y) -
               static_cast<double>(p2.x) * static_cast<double>(p1.y);
    }
    return static_cast<float>(0.5 * sum);
}

Point2D computeCentroid(const std::vector<Point2D>& poly, float signed_area) {
    if (std::abs(signed_area) < 1e-4f || poly.size() < 3) {
        float cx = 0.0f, cy = 0.0f;
        for (const auto& p : poly) {
            cx += p.x;
            cy += p.y;
        }
        const float inv = 1.0f / static_cast<float>(poly.size());
        return Point2D{cx * inv, cy * inv};
    }

    double cx_sum = 0.0;
    double cy_sum = 0.0;
    const size_t n = poly.size();
    for (size_t i = 0; i < n; ++i) {
        const auto& p1 = poly[i];
        const auto& p2 = poly[(i + 1) % n];
        const double cross = static_cast<double>(p1.x) * static_cast<double>(p2.y) -
                             static_cast<double>(p2.x) * static_cast<double>(p1.y);
        cx_sum += (static_cast<double>(p1.x) + static_cast<double>(p2.x)) * cross;
        cy_sum += (static_cast<double>(p1.y) + static_cast<double>(p2.y)) * cross;
    }
    const double factor = 1.0 / (6.0 * static_cast<double>(signed_area));
    return Point2D{static_cast<float>(cx_sum * factor), static_cast<float>(cy_sum * factor)};
}

} // namespace

std::vector<RoomPolygon> RoomPolygonDetector::detectRooms(const std::vector<WallSegment>& walls,
                                                          const std::vector<WallJunction>& junctions) const {

    std::vector<RoomPolygon> rooms;
    if (walls.empty() || junctions.size() < 3) {
        return rooms;
    }

    // Map junction id -> position
    std::map<int, Point2D> j_pos;
    for (const auto& j : junctions) {
        j_pos[j.id] = j.position;
    }

    // Build directed half-edges
    // Map: from_junction -> list of HalfEdge sorted by angle
    std::map<int, std::vector<HalfEdge>> adj;
    std::vector<HalfEdge> all_half_edges;

    for (const auto& w : walls) {
        // Find junction ids closest to w.start and w.end
        int u = -1;
        int v = -1;
        float min_du = 1e9f;
        float min_dv = 1e9f;

        for (const auto& j : junctions) {
            const float du = std::hypot(j.position.x - w.start.x, j.position.y - w.start.y);
            if (du < min_du) {
                min_du = du;
                u = j.id;
            }
            const float dv = std::hypot(j.position.x - w.end.x, j.position.y - w.end.y);
            if (dv < min_dv) {
                min_dv = dv;
                v = j.id;
            }
        }

        if (u == -1 || v == -1 || u == v) continue;

        const auto& pu = j_pos[u];
        const auto& pv = j_pos[v];

        HalfEdge e1{u, v, w.id, std::atan2(pv.y - pu.y, pv.x - pu.x)};
        HalfEdge e2{v, u, w.id, std::atan2(pu.y - pv.y, pu.x - pv.x)};

        adj[u].push_back(e1);
        adj[v].push_back(e2);
        all_half_edges.push_back(e1);
        all_half_edges.push_back(e2);
    }

    // Sort outgoing edges around each junction counter-clockwise
    for (auto& [jid, edge_list] : adj) {
        std::sort(edge_list.begin(), edge_list.end(),
                  [](const HalfEdge& a, const HalfEdge& b) { return a.angle < b.angle; });
    }

    // Planar face extraction
    std::set<std::pair<int, int>> visited_edges;
    int next_room_id = 1;

    for (const auto& initial_edge : all_half_edges) {
        if (visited_edges.contains({initial_edge.from_junction, initial_edge.to_junction})) {
            continue;
        }

        std::vector<Point2D> face_vertices;
        std::vector<int> face_walls;
        HalfEdge curr = initial_edge;
        bool valid_cycle = false;

        const size_t max_steps = all_half_edges.size() + 10;
        for (size_t step = 0; step < max_steps; ++step) {
            visited_edges.insert({curr.from_junction, curr.to_junction});
            face_vertices.push_back(j_pos[curr.from_junction]);
            if (std::find(face_walls.begin(), face_walls.end(), curr.wall_id) == face_walls.end()) {
                face_walls.push_back(curr.wall_id);
            }

            const int v = curr.to_junction;
            const auto& out_edges = adj[v];
            if (out_edges.empty()) break;

            // Find reverse edge in v's outgoing list: (v -> curr.from_junction)
            int rev_idx = -1;
            for (size_t i = 0; i < out_edges.size(); ++i) {
                if (out_edges[i].to_junction == curr.from_junction) {
                    rev_idx = static_cast<int>(i);
                    break;
                }
            }

            if (rev_idx == -1) break;

            // In screen coordinates (y down), turning clockwise / rightmost into interior face
            const size_t next_idx = (static_cast<size_t>(rev_idx) + out_edges.size() - 1) % out_edges.size();
            curr = out_edges[next_idx];

            if (curr.from_junction == initial_edge.from_junction && curr.to_junction == initial_edge.to_junction) {
                valid_cycle = true;
                break;
            }
        }

        if (!valid_cycle || face_vertices.size() < 3) continue;

        const float signed_area = computeSignedArea(face_vertices);

        // Counter-clockwise orientation yields positive area in standard Cartesian (or negative in screen coords).
        // Interior rooms have positive signed area when traversed counter-clockwise.
        // If screen coords (y down), positive vs negative: test magnitude vs min_room_area_px
        const float abs_area = std::abs(signed_area);
        if (abs_area < config_.min_room_area_px) continue;

        // Ensure we only collect interior rooms (not the exterior unbounded face)
        // For interior rooms with counter-clockwise orientation, signed_area is positive (or check vertex order)
        if (signed_area < 0.0f) {
            // Unbounded exterior face
            continue;
        }

        RoomPolygon room;
        room.id = next_room_id++;
        room.label = "Room " + std::to_string(room.id);
        room.vertices = face_vertices;
        room.area_px = abs_area;
        room.centroid = computeCentroid(face_vertices, signed_area);
        room.bounding_wall_ids = face_walls;

        // Compute bounding dimensions & perimeter
        float min_x = 1e9f, max_x = -1e9f;
        float min_y = 1e9f, max_y = -1e9f;
        float perimeter = 0.0f;
        const size_t vn = face_vertices.size();
        for (size_t i = 0; i < vn; ++i) {
            const auto& p1 = face_vertices[i];
            const auto& p2 = face_vertices[(i + 1) % vn];
            perimeter += std::hypot(p2.x - p1.x, p2.y - p1.y);
            min_x = std::min(min_x, p1.x);
            max_x = std::max(max_x, p1.x);
            min_y = std::min(min_y, p1.y);
            max_y = std::max(max_y, p1.y);
        }
        room.bounding_width = max_x - min_x;
        room.bounding_height = max_y - min_y;
        room.perimeter = perimeter;

        if (config_.pixels_per_meter > 0.0f) {
            const float ppm2 = config_.pixels_per_meter * config_.pixels_per_meter;
            room.area_sqm = abs_area / ppm2;
        }

        rooms.push_back(room);
    }

    return rooms;
}

} // namespace CurvEngine::domain
