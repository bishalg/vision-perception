#include "curv/geometry/JunctionDeriver.hpp"

#include <algorithm>
#include <cmath>
#include <set>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace CurvEngine::geometry {
namespace {

struct Endpoint {
    Point2D pos;
    size_t seg_idx;
    bool front; // true if the segment's first point, false if its last
};

struct EndpointCluster {
    Point2D pos{0.0F, 0.0F, 0.0F, 1.0F};
    std::vector<Endpoint> members;
    std::set<size_t> seg_idxs;
};

struct BBox {
    float min_x{0.0F};
    float min_y{0.0F};
    float max_x{0.0F};
    float max_y{0.0F};
    bool valid{false};
};

BBox segmentBBox(const CurveSegment& seg) {
    BBox box;
    box.valid = seg.points.size() >= 2;
    if (!box.valid) {
        return box;
    }
    box.min_x = box.max_x = seg.points.front().x;
    box.min_y = box.max_y = seg.points.front().y;
    for (const auto& p : seg.points) {
        box.min_x = std::min(box.min_x, p.x);
        box.min_y = std::min(box.min_y, p.y);
        box.max_x = std::max(box.max_x, p.x);
        box.max_y = std::max(box.max_y, p.y);
    }
    return box;
}

struct Crossing {
    size_t seg_i{0};
    size_t seg_j{0};
    Point2D pos{0.0F, 0.0F, 0.0F, 1.0F};
};

// All proper crossings between two polylines at interior points of both. The interior
// parameter margin keeps crossings away from the polylines' own endpoints (those
// connection points belong to endpoint clusters).
void collectInteriorCrossings(const CurveSegment& a, size_t idx_a, const CurveSegment& b, size_t idx_b,
                              std::vector<Crossing>& out) {
    for (size_t i = 0; i + 1 < a.points.size(); ++i) {
        const float ax = a.points[i].x;
        const float ay = a.points[i].y;
        const float abx = a.points[i + 1].x - ax;
        const float aby = a.points[i + 1].y - ay;
        for (size_t j = 0; j + 1 < b.points.size(); ++j) {
            const float cx = b.points[j].x;
            const float cy = b.points[j].y;
            const float cdx = b.points[j + 1].x - cx;
            const float cdy = b.points[j + 1].y - cy;
            const float den = abx * cdy - aby * cdx;
            if (std::fabs(den) < 1e-9F) {
                continue;
            }
            const float t = ((cx - ax) * cdy - (cy - ay) * cdx) / den;
            const float s = ((cx - ax) * aby - (cy - ay) * abx) / den;
            if (t >= 0.02F && t <= 0.98F && s >= 0.02F && s <= 0.98F) {
                out.push_back(Crossing{idx_a, idx_b, Point2D{ax + abx * t, ay + aby * t, 0.0F, 1.0F}});
            }
        }
    }
}

float polylineLength(const std::vector<Point2D>& pts) {
    float length = 0.0F;
    for (size_t k = 0; k + 1 < pts.size(); ++k) {
        const float dx = pts[k + 1].x - pts[k].x;
        const float dy = pts[k + 1].y - pts[k].y;
        length += std::sqrt(dx * dx + dy * dy);
    }
    return length;
}

// Index of the vertex in [from, pts.size()) nearest to p, plus cumulative arc length there.
size_t nearestVertex(const std::vector<Point2D>& pts, size_t from, const Point2D& p, float& cum_at_best) {
    float best_d2 = -1.0F;
    size_t best = from;
    float cum = 0.0F;
    cum_at_best = 0.0F;
    for (size_t k = 0; k < pts.size(); ++k) {
        if (k > 0) {
            const float dx = pts[k].x - pts[k - 1].x;
            const float dy = pts[k].y - pts[k - 1].y;
            cum += std::sqrt(dx * dx + dy * dy);
        }
        if (k < from) {
            continue;
        }
        const float dx = pts[k].x - p.x;
        const float dy = pts[k].y - p.y;
        const float d2 = dx * dx + dy * dy;
        if (best_d2 < 0.0F || d2 < best_d2) {
            best_d2 = d2;
            best = k;
            cum_at_best = cum;
        }
    }
    return best;
}

} // namespace

size_t deriveJunctions(RidgeGraph& graph, float tol_px) {
    const size_t seg_count = graph.segments.size();
    const float tol2 = tol_px * tol_px;

    // ---- Phase 1: endpoint clusters ---------------------------------------
    std::vector<Endpoint> endpoints;
    for (size_t i = 0; i < seg_count; ++i) {
        const auto& seg = graph.segments[i];
        if (seg.points.size() < 2) {
            continue;
        }
        endpoints.push_back({seg.points.front(), i, true});
        endpoints.push_back({seg.points.back(), i, false});
    }
    // Sparse endpoint grid: cluster members are gathered from the 3x3 cell block around
    // each seed (cell = tol_px), so clustering is linear-in-endpoints instead of the
    // O(endpoints^2) scan that stalled on fine-scale graphs.
    std::unordered_map<long long, std::vector<size_t>> egrid;
    auto cell_key = [&](const Point2D& p) {
        return static_cast<long long>(std::floor(p.x / tol_px)) * 100000L +
               static_cast<long long>(std::floor(p.y / tol_px));
    };
    for (size_t i = 0; i < endpoints.size(); ++i) {
        egrid[cell_key(endpoints[i].pos)].push_back(i);
    }
    const int base_cx = 0;
    const int base_cy = 0;
    (void)base_cx;
    (void)base_cy;
    std::vector<EndpointCluster> clusters;
    std::vector<bool> assigned(endpoints.size(), false);
    for (size_t i = 0; i < endpoints.size(); ++i) {
        if (assigned[i]) {
            continue;
        }
        EndpointCluster cluster;
        cluster.members.push_back(endpoints[i]);
        cluster.seg_idxs.insert(endpoints[i].seg_idx);
        assigned[i] = true;
        const int qc_x = static_cast<int>(std::floor(endpoints[i].pos.x / tol_px));
        const int qc_y = static_cast<int>(std::floor(endpoints[i].pos.y / tol_px));
        for (int cy = qc_y - 1; cy <= qc_y + 1; ++cy) {
            for (int cx = qc_x - 1; cx <= qc_x + 1; ++cx) {
                const auto it = egrid.find(static_cast<long long>(cx) * 100000L + cy);
                if (it == egrid.end()) {
                    continue;
                }
                for (size_t j : it->second) {
                    if (assigned[j]) {
                        continue;
                    }
                    const float dx = endpoints[j].pos.x - endpoints[i].pos.x;
                    const float dy = endpoints[j].pos.y - endpoints[i].pos.y;
                    if (dx * dx + dy * dy <= tol2) {
                        assigned[j] = true;
                        cluster.members.push_back(endpoints[j]);
                        cluster.seg_idxs.insert(endpoints[j].seg_idx);
                    }
                }
            }
        }
        if (cluster.seg_idxs.size() >= 2) {
            float sum_x = 0.0F;
            float sum_y = 0.0F;
            for (const auto& m : cluster.members) {
                sum_x += m.pos.x;
                sum_y += m.pos.y;
            }
            const float n = static_cast<float>(cluster.members.size());
            cluster.pos = Point2D{sum_x / n, sum_y / n, 0.0F, 1.0F};
            clusters.push_back(std::move(cluster));
        }
    }

    // ---- Phase 2: through-crossings between segment pairs ------------------
    std::vector<BBox> boxes(seg_count);
    for (size_t i = 0; i < seg_count; ++i) {
        boxes[i] = segmentBBox(graph.segments[i]);
    }
    std::vector<Crossing> crossings;
    {
        // Sparse bbox grid: overlapping bboxes always share at least one cell (each bbox
        // is registered in every cell it spans), so the O(n^2) pair scan becomes
        // linear-in-k times local density.
        constexpr float kCell = 96.0F;
        std::unordered_map<long long, std::vector<size_t>> buckets;
        for (size_t i = 0; i < seg_count; ++i) {
            if (!boxes[i].valid) {
                continue;
            }
            const int cx0 = static_cast<int>(std::floor(boxes[i].min_x / kCell));
            const int cx1 = static_cast<int>(std::floor(boxes[i].max_x / kCell));
            const int cy0 = static_cast<int>(std::floor(boxes[i].min_y / kCell));
            const int cy1 = static_cast<int>(std::floor(boxes[i].max_y / kCell));
            for (int cy = cy0; cy <= cy1; ++cy) {
                for (int cx = cx0; cx <= cx1; ++cx) {
                    buckets[static_cast<long long>(cx) * 100000L + cy].push_back(i);
                }
            }
        }
        std::unordered_set<long long> tested;
        for (const auto& [key, members] : buckets) {
            for (size_t a = 0; a + 1 < members.size(); ++a) {
                for (size_t b = a + 1; b < members.size(); ++b) {
                    const size_t i = members[a];
                    const size_t j = members[b];
                    const long long pair_key =
                        static_cast<long long>(i) * static_cast<long long>(seg_count) + static_cast<long long>(j);
                    if (!tested.insert(pair_key).second) {
                        continue;
                    }
                    const auto& bi = boxes[i];
                    const auto& bj = boxes[j];
                    if (bi.max_x < bj.min_x || bj.max_x < bi.min_x || bi.max_y < bj.min_y || bj.max_y < bi.min_y) {
                        continue;
                    }
                    collectInteriorCrossings(graph.segments[i], i, graph.segments[j], j, crossings);
                }
            }
        }
    }
    // Deterministic order, then drop crossings coinciding with an endpoint cluster or
    // with an already accepted crossing (concurrent multi-segment meetings).
    std::sort(crossings.begin(), crossings.end(), [](const Crossing& a, const Crossing& b) {
        if (a.pos.x != b.pos.x) {
            return a.pos.x < b.pos.x;
        }
        if (a.pos.y != b.pos.y) {
            return a.pos.y < b.pos.y;
        }
        if (a.seg_i != b.seg_i) {
            return a.seg_i < b.seg_i;
        }
        return a.seg_j < b.seg_j;
    });
    std::vector<Crossing> accepted;
    for (const auto& c : crossings) {
        bool near_existing = false;
        for (const auto& cluster : clusters) {
            const float dx = cluster.pos.x - c.pos.x;
            const float dy = cluster.pos.y - c.pos.y;
            if (dx * dx + dy * dy <= tol2) {
                near_existing = true;
                break;
            }
        }
        if (!near_existing) {
            for (const auto& a : accepted) {
                const float dx = a.pos.x - c.pos.x;
                const float dy = a.pos.y - c.pos.y;
                if (dx * dx + dy * dy <= tol2) {
                    near_existing = true;
                    break;
                }
            }
        }
        if (!near_existing) {
            accepted.push_back(c);
        }
    }

    // ---- Phase 3: split segments at their accepted crossings ---------------
    // splits[i]: (cumulative arc length, index into accepted) sorted by arc length.
    std::vector<std::vector<std::pair<float, size_t>>> splits(seg_count);
    for (size_t k = 0; k < accepted.size(); ++k) {
        splits[accepted[k].seg_i].emplace_back(0.0F, k);
        splits[accepted[k].seg_j].emplace_back(0.0F, k);
    }

    std::vector<CurveSegment> new_segments;
    std::vector<std::vector<int>> piece_ids(seg_count); // per piece, arc order; -1 = degenerate
    int next_id = 0;
    for (const auto& seg : graph.segments) {
        next_id = std::max(next_id, seg.id + 1);
    }

    for (size_t i = 0; i < seg_count; ++i) {
        const auto& seg = graph.segments[i];
        if (seg.points.size() < 2) {
            continue;
        }
        if (splits[i].empty()) {
            piece_ids[i].push_back(seg.id);
            new_segments.push_back(seg);
            continue;
        }
        for (auto& split : splits[i]) {
            split.first = [&] {
                float cum = 0.0F;
                nearestVertex(seg.points, 0, accepted[split.second].pos, cum);
                return cum;
            }();
        }
        std::sort(splits[i].begin(), splits[i].end(), [](const auto& a, const auto& b) { return a.first < b.first; });

        const size_t piece_count = splits[i].size() + 1;
        piece_ids[i].assign(piece_count, -1);
        std::vector<std::vector<Point2D>> pieces(piece_count);
        size_t vertex = 0;
        for (size_t s = 0; s < splits[i].size(); ++s) {
            const Point2D& cut = accepted[splits[i][s].second].pos;
            float cum_at_best = 0.0F;
            const size_t best = nearestVertex(seg.points, vertex, cut, cum_at_best);
            for (size_t k = vertex; k <= best; ++k) {
                pieces[s].push_back(seg.points[k]);
            }
            pieces[s].push_back(cut); // exact crossing point as the piece boundary
            vertex = best;
        }
        for (size_t k = vertex; k < seg.points.size(); ++k) {
            pieces.back().push_back(seg.points[k]);
        }

        for (size_t p = 0; p < pieces.size(); ++p) {
            if (pieces[p].size() < 2) {
                continue; // split collapsed a piece (degenerate); junction assembly tolerates it
            }
            CurveSegment out;
            out.id = (p == 0) ? seg.id : next_id++;
            out.points = std::move(pieces[p]);
            out.total_length = polylineLength(out.points);
            out.is_closed = false;
            piece_ids[i][p] = out.id;
            new_segments.push_back(std::move(out));
        }
    }

    // ---- Phase 4: assemble junctions --------------------------------------
    std::vector<Junction> new_junctions;
    for (const auto& cluster : clusters) {
        std::set<int> ids;
        for (const auto& m : cluster.members) {
            const auto& pid = piece_ids[m.seg_idx];
            if (pid.empty()) {
                continue;
            }
            const size_t pick = m.front ? 0 : pid.size() - 1;
            if (pid[pick] >= 0) {
                ids.insert(pid[pick]);
            }
        }
        if (ids.size() >= 2) {
            Junction junc;
            junc.position = cluster.pos;
            junc.connected_segment_ids.assign(ids.begin(), ids.end());
            new_junctions.push_back(std::move(junc));
        }
    }
    for (size_t k = 0; k < accepted.size(); ++k) {
        const size_t si = accepted[k].seg_i;
        const size_t sj = accepted[k].seg_j;
        int m = -1;
        int n = -1;
        for (size_t x = 0; x < splits[si].size(); ++x) {
            if (splits[si][x].second == k) {
                m = static_cast<int>(x);
                break;
            }
        }
        for (size_t x = 0; x < splits[sj].size(); ++x) {
            if (splits[sj][x].second == k) {
                n = static_cast<int>(x);
                break;
            }
        }
        if (m < 0 || n < 0) {
            continue;
        }
        std::set<int> ids;
        const int cand[4] = {piece_ids[si][static_cast<size_t>(m)], piece_ids[si][static_cast<size_t>(m) + 1],
                             piece_ids[sj][static_cast<size_t>(n)], piece_ids[sj][static_cast<size_t>(n) + 1]};
        for (int id : cand) {
            if (id >= 0) {
                ids.insert(id);
            }
        }
        if (ids.size() >= 2) {
            Junction junc;
            junc.position = accepted[k].pos;
            junc.connected_segment_ids.assign(ids.begin(), ids.end());
            new_junctions.push_back(std::move(junc));
        }
    }

    graph.segments = std::move(new_segments);
    graph.junctions = std::move(new_junctions);
    return graph.junctions.size();
}

} // namespace CurvEngine::geometry
