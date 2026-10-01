#include "curv/pattern/MicroPatternDetector.hpp"
#include <algorithm>
#include <cmath>
#include <map>
#include <set>
#include <unordered_map>

namespace CurvEngine::pattern {

namespace {

Point2D computeCentroid(const std::vector<Point2D>& pts) {
    if (pts.empty()) return {0.0f, 0.0f};
    float sum_x = 0.0f;
    float sum_y = 0.0f;
    for (const auto& p : pts) {
        sum_x += p.x;
        sum_y += p.y;
    }
    float n = static_cast<float>(pts.size());
    return {sum_x / n, sum_y / n};
}

float distance2D(const Point2D& a, const Point2D& b) {
    float dx = b.x - a.x;
    float dy = b.y - a.y;
    return std::sqrt(dx * dx + dy * dy);
}

// Build adjacency matrix between segments based on shared junctions
std::map<int, std::set<int>> buildSegmentAdjacency(const RidgeGraph& graph) {
    std::map<int, std::set<int>> adj;
    for (const auto& j : graph.junctions) {
        for (size_t a = 0; a < j.connected_segment_ids.size(); ++a) {
            for (size_t b = a + 1; b < j.connected_segment_ids.size(); ++b) {
                int id_a = j.connected_segment_ids[a];
                int id_b = j.connected_segment_ids[b];
                adj[id_a].insert(id_b);
                adj[id_b].insert(id_a);
            }
        }
    }
    return adj;
}

} // namespace

std::vector<DetectedPattern> MicroPatternDetector::findIslands(const RidgeGraph& graph) {
    std::vector<DetectedPattern> islands;

    // An island occurs when two distinct junctions connect the exact same two segments,
    // or two segments start at junction J1 and terminate at junction J2.
    for (size_t i = 0; i < graph.junctions.size(); ++i) {
        for (size_t j = i + 1; j < graph.junctions.size(); ++j) {
            const auto& j1 = graph.junctions[i];
            const auto& j2 = graph.junctions[j];

            std::vector<int> shared;
            for (int seg1 : j1.connected_segment_ids) {
                for (int seg2 : j2.connected_segment_ids) {
                    if (seg1 == seg2) {
                        shared.push_back(seg1);
                    }
                }
            }

            if (shared.size() >= 2) {
                DetectedPattern pat;
                pat.type = PatternType::Island;
                pat.name = "Island";
                pat.confidence = 0.95f;
                pat.source_segment_ids = {shared[0], shared[1]};
                pat.points = {j1.position, j2.position};
                pat.centroid = computeCentroid(pat.points);
                islands.push_back(pat);
            }
        }
    }

    return islands;
}

std::vector<DetectedPattern> MicroPatternDetector::findTriangles(const RidgeGraph& graph) {
    std::vector<DetectedPattern> triangles;
    auto adj = buildSegmentAdjacency(graph);

    // Find 3-cycles in segment adjacency graph
    std::vector<int> seg_ids;
    for (const auto& [seg_id, _] : adj) {
        seg_ids.push_back(seg_id);
    }

    for (size_t i = 0; i < seg_ids.size(); ++i) {
        for (size_t j = i + 1; j < seg_ids.size(); ++j) {
            for (size_t k = j + 1; k < seg_ids.size(); ++k) {
                int u = seg_ids[i];
                int v = seg_ids[j];
                int w = seg_ids[k];

                if (adj[u].count(v) && adj[v].count(w) && adj[w].count(u)) {
                    DetectedPattern pat;
                    pat.type = PatternType::Triangle;
                    pat.name = "Triangle";
                    pat.confidence = 0.90f;
                    pat.source_segment_ids = {u, v, w};

                    // Collect junction points connecting them
                    for (const auto& junc : graph.junctions) {
                        int matches = 0;
                        for (int cid : junc.connected_segment_ids) {
                            if (cid == u || cid == v || cid == w) matches++;
                        }
                        if (matches >= 2) {
                            pat.points.push_back(junc.position);
                        }
                    }
                    pat.centroid = computeCentroid(pat.points);
                    triangles.push_back(pat);
                }
            }
        }
    }

    return triangles;
}

std::vector<DetectedPattern> MicroPatternDetector::findSquares(const RidgeGraph& graph) {
    std::vector<DetectedPattern> squares;
    auto adj = buildSegmentAdjacency(graph);

    std::vector<int> seg_ids;
    for (const auto& [seg_id, _] : adj) {
        seg_ids.push_back(seg_id);
    }

    // Find 4-cycles: u - v - w - x - u
    for (size_t i = 0; i < seg_ids.size(); ++i) {
        for (size_t j = i + 1; j < seg_ids.size(); ++j) {
            for (size_t k = j + 1; k < seg_ids.size(); ++k) {
                for (size_t m = k + 1; m < seg_ids.size(); ++m) {
                    int u = seg_ids[i];
                    int v = seg_ids[j];
                    int w = seg_ids[k];
                    int x = seg_ids[m];

                    bool is_4cycle = (adj[u].count(v) && adj[v].count(w) && adj[w].count(x) && adj[x].count(u)) ||
                                     (adj[u].count(w) && adj[w].count(v) && adj[v].count(x) && adj[x].count(u)) ||
                                     (adj[u].count(v) && adj[v].count(x) && adj[x].count(w) && adj[w].count(u));

                    if (is_4cycle) {
                        DetectedPattern pat;
                        pat.type = PatternType::Square;
                        pat.name = "Square";
                        pat.confidence = 0.85f;
                        pat.source_segment_ids = {u, v, w, x};

                        for (const auto& junc : graph.junctions) {
                            int matches = 0;
                            for (int cid : junc.connected_segment_ids) {
                                if (cid == u || cid == v || cid == w || cid == x) matches++;
                            }
                            if (matches >= 2) {
                                pat.points.push_back(junc.position);
                            }
                        }
                        pat.centroid = computeCentroid(pat.points);
                        squares.push_back(pat);
                    }
                }
            }
        }
    }

    return squares;
}

std::vector<DetectedPattern> MicroPatternDetector::findCrosses(const RidgeGraph& graph) {
    std::vector<DetectedPattern> crosses;

    for (const auto& junc : graph.junctions) {
        // Degree 4 intersection forms a cross
        if (junc.connected_segment_ids.size() == 4) {
            DetectedPattern pat;
            pat.type = PatternType::Cross;
            pat.name = "Cross";
            pat.confidence = 0.90f;
            pat.centroid = junc.position;
            pat.points = {junc.position};
            pat.source_segment_ids = junc.connected_segment_ids;
            crosses.push_back(pat);
        }
    }

    return crosses;
}

std::vector<DetectedPattern> MicroPatternDetector::findStars(const RidgeGraph& graph) {
    std::vector<DetectedPattern> stars;

    for (const auto& junc : graph.junctions) {
        // Degree 5 or higher forms a star
        if (junc.connected_segment_ids.size() >= 5) {
            DetectedPattern pat;
            pat.type = PatternType::Star;
            pat.name = "Star";
            pat.confidence = 0.95f;
            pat.centroid = junc.position;
            pat.points = {junc.position};
            pat.source_segment_ids = junc.connected_segment_ids;
            stars.push_back(pat);
        }
    }

    return stars;
}

std::vector<DetectedPattern> MicroPatternDetector::findTridents(const RidgeGraph& graph) {
    std::vector<DetectedPattern> tridents;

    for (const auto& junc : graph.junctions) {
        // Degree 3 forward branching termination
        if (junc.connected_segment_ids.size() == 3) {
            DetectedPattern pat;
            pat.type = PatternType::Trident;
            pat.name = "Trident";
            pat.confidence = 0.85f;
            pat.centroid = junc.position;
            pat.points = {junc.position};
            pat.source_segment_ids = junc.connected_segment_ids;
            tridents.push_back(pat);
        }
    }

    return tridents;
}

std::vector<DetectedPattern> MicroPatternDetector::findNets(const RidgeGraph& graph) {
    std::vector<DetectedPattern> nets;

    // A net (grille) has multiple clustered degree-4 cross junctions in close proximity
    std::vector<Point2D> cross_pts;
    for (const auto& junc : graph.junctions) {
        if (junc.connected_segment_ids.size() >= 4) {
            cross_pts.push_back(junc.position);
        }
    }

    if (cross_pts.size() >= 4) {
        // Simple bounding check for grid cluster
        DetectedPattern pat;
        pat.type = PatternType::Net;
        pat.name = "Net";
        pat.confidence = 0.88f;
        pat.points = cross_pts;
        pat.centroid = computeCentroid(cross_pts);
        nets.push_back(pat);
    }

    return nets;
}

std::vector<DetectedPattern> MicroPatternDetector::findFish(const RidgeGraph& graph) {
    std::vector<DetectedPattern> fish_signs;

    // A fish sign consists of an Island or Triangle body with an attached cross/fin vertex
    auto triangles = findTriangles(graph);
    auto islands = findIslands(graph);

    // Look for any triangle or island where at least one vertex is connected to tail rays
    for (const auto& tri : triangles) {
        for (const auto& junc : graph.junctions) {
            if (distance2D(junc.position, tri.centroid) < 40.0f && junc.connected_segment_ids.size() >= 4) {
                DetectedPattern pat;
                pat.type = PatternType::Fish;
                pat.name = "Fish (Matsya)";
                pat.confidence = 0.92f;
                pat.centroid = tri.centroid;
                pat.points = tri.points;
                pat.points.push_back(junc.position);
                pat.source_segment_ids = tri.source_segment_ids;
                fish_signs.push_back(pat);
                break;
            }
        }
    }

    return fish_signs;
}

const char* patternTypeName(PatternType type) {
    switch (type) {
    case PatternType::Square:
        return "Square";
    case PatternType::Triangle:
        return "Triangle";
    case PatternType::Star:
        return "Star";
    case PatternType::Circle:
        return "Circle";
    case PatternType::Island:
        return "Island";
    case PatternType::Cross:
        return "Cross";
    case PatternType::Trident:
        return "Trident";
    case PatternType::SupportingLine:
        return "SupportingLine";
    case PatternType::ChainedLine:
        return "ChainedLine";
    case PatternType::Tassels:
        return "Tassels";
    case PatternType::Net:
        return "Net";
    case PatternType::Fish:
        return "Fish";
    }
    return "Unknown";
}

// ===================== Geometry-validated detection (P2 soft filters) =====================

namespace {

// Per-call O(1) lookups: the validated finders emit many candidates on dense graphs,
// and per-candidate linear scans (segment-by-id, junction-by-position) dominated the
// profile. Built once per finder call, passed down through the validators.
class GraphLookup {
public:
    explicit GraphLookup(const RidgeGraph& graph) {
        for (const auto& seg : graph.segments) {
            segs_[seg.id] = &seg;
        }
        for (const auto& j : graph.junctions) {
            junctions_[junctionKey(j.position)].push_back(&j);
        }
    }

    [[nodiscard]] const CurveSegment* segment(int id) const {
        const auto it = segs_.find(id);
        return (it != segs_.end()) ? it->second : nullptr;
    }

    // Nearest junction within 0.5 px (the junctionNear tolerance), via 1px buckets.
    [[nodiscard]] const Junction* junctionNear(const Point2D& pos) const {
        const int bx = static_cast<int>(std::floor(pos.x));
        const int by = static_cast<int>(std::floor(pos.y));
        const Junction* best = nullptr;
        float best_d2 = 0.25F;
        for (int dy = -1; dy <= 1; ++dy) {
            for (int dx = -1; dx <= 1; ++dx) {
                const auto it = junctions_.find(junctionKey(bx + dx, by + dy));
                if (it == junctions_.end()) {
                    continue;
                }
                for (const Junction* j : it->second) {
                    const float ddx = j->position.x - pos.x;
                    const float ddy = j->position.y - pos.y;
                    const float d2 = ddx * ddx + ddy * ddy;
                    if (d2 <= best_d2) {
                        best_d2 = d2;
                        best = j;
                    }
                }
            }
        }
        return best;
    }

private:
    static long long junctionKey(int bx, int by) {
        return static_cast<long long>(bx) * 100000L + static_cast<long long>(by);
    }
    static long long junctionKey(float x, float y) {
        return static_cast<long long>(std::floor(x)) * 100000L + static_cast<long long>(std::floor(y));
    }
    [[nodiscard]] static long long junctionKey(const Point2D& p) { return junctionKey(p.x, p.y); }

    std::unordered_map<int, const CurveSegment*> segs_;
    std::unordered_map<long long, std::vector<const Junction*>> junctions_;
};

// Legacy fixed confidences double as topology priors for the validated path.
float topologyPrior(PatternType type) {
    switch (type) {
    case PatternType::Island:
        return 0.95F;
    case PatternType::Triangle:
        return 0.90F;
    case PatternType::Square:
        return 0.85F;
    case PatternType::Cross:
        return 0.90F;
    case PatternType::Star:
        return 0.95F;
    case PatternType::Trident:
        return 0.85F;
    case PatternType::Net:
        return 0.88F;
    case PatternType::Fish:
        return 0.92F;
    default:
        return 0.80F;
    }
}

struct ArmDir {
    float ux{0.0F};
    float uy{0.0F}; // unit direction pointing away from the junction
    float length{0.0F};
};

// Arm directions at a junction: direction from the junction to the farthest point of each
// attached segment (junctions do not record their attachment point on the segment).
std::vector<ArmDir> armsAt(const GraphLookup& lookup, const Junction& junc) {
    std::vector<ArmDir> arms;
    arms.reserve(junc.connected_segment_ids.size());
    for (int id : junc.connected_segment_ids) {
        const CurveSegment* seg = lookup.segment(id);
        if (seg == nullptr || seg->points.empty()) {
            continue;
        }
        const Point2D* far_pt = nullptr;
        float far_d = -1.0F;
        for (const auto& p : seg->points) {
            const float dx = p.x - junc.position.x;
            const float dy = p.y - junc.position.y;
            const float d = std::sqrt(dx * dx + dy * dy);
            if (d > far_d) {
                far_d = d;
                far_pt = &p;
            }
        }
        if (far_pt == nullptr || far_d <= 1e-3F) {
            continue;
        }
        ArmDir arm;
        arm.ux = (far_pt->x - junc.position.x) / far_d;
        arm.uy = (far_pt->y - junc.position.y) / far_d;
        arm.length = far_d;
        arms.push_back(arm);
    }
    return arms;
}

// Counts vertices that coincide with a degree-4+ junction (grid cells share 4 of those).
int degree4VertexCount(const GraphLookup& lookup, const std::vector<Point2D>& vertices) {
    int count = 0;
    for (const auto& v : vertices) {
        const Junction* j = lookup.junctionNear(v);
        if (j != nullptr && j->connected_segment_ids.size() >= 4) {
            ++count;
        }
    }
    return count;
}

// Reconstructs the island's closed loop from its two shared arc segments.
std::vector<Point2D> islandLoopPoints(const GraphLookup& lookup, const DetectedPattern& isl) {
    if (isl.source_segment_ids.size() < 2) {
        return {};
    }
    const CurveSegment* a = lookup.segment(isl.source_segment_ids[0]);
    const CurveSegment* b = lookup.segment(isl.source_segment_ids[1]);
    if (a == nullptr || b == nullptr || a->points.size() < 2 || b->points.size() < 2) {
        return {};
    }
    std::vector<Point2D> loop = a->points;
    for (auto it = b->points.rbegin(); it != b->points.rend(); ++it) {
        loop.push_back(*it);
    }
    return loop;
}

float angleBetween(float ux1, float uy1, float ux2, float uy2) {
    const float n1 = std::sqrt(ux1 * ux1 + uy1 * uy1);
    const float n2 = std::sqrt(ux2 * ux2 + uy2 * uy2);
    if (n1 < 1e-6F || n2 < 1e-6F) {
        return 0.0F;
    }
    const float dot = std::min(std::max((ux1 * ux2 + uy1 * uy2) / (n1 * n2), -1.0F), 1.0F);
    constexpr float kRadToDeg = 57.2957795F;
    return std::acos(dot) * kRadToDeg;
}

float acuteBetween(float ux1, float uy1, float ux2, float uy2) {
    const float a = angleBetween(ux1, uy1, ux2, uy2);
    return (a > 90.0F) ? 180.0F - a : a;
}

// Absolute direction of an arm in [0, 360) degrees.
float dirAngleDeg(const ArmDir& arm) {
    const float deg = std::atan2(arm.uy, arm.ux) * 57.2957795F;
    return (deg < 0.0F) ? deg + 360.0F : deg;
}

// Minimum angular gap between consecutive arm directions (cyclically sorted). For stars
// this is the "adjacent arm angle" — opposite arms fold to ~0 under the acute measure
// and must not count against the fan.
float minAdjacentGapDeg(const std::vector<ArmDir>& arms) {
    if (arms.size() < 2) {
        return 360.0F;
    }
    std::vector<float> angles;
    angles.reserve(arms.size());
    for (const auto& arm : arms) {
        angles.push_back(dirAngleDeg(arm));
    }
    std::sort(angles.begin(), angles.end());
    float min_gap = 360.0F;
    for (size_t i = 0; i < angles.size(); ++i) {
        const float next = (i + 1 < angles.size()) ? angles[i + 1] : angles[0] + 360.0F;
        min_gap = std::min(min_gap, next - angles[i]);
    }
    return min_gap;
}

// Angle at vertex b for polygon a-b-c.
float angleAtVertex(const Point2D& a, const Point2D& b, const Point2D& c) {
    const float abx = a.x - b.x;
    const float aby = a.y - b.y;
    const float cbx = c.x - b.x;
    const float cby = c.y - b.y;
    return angleBetween(abx, aby, cbx, cby);
}

// Orders vertices around their centroid so corner angles / side ratios read the polygon
// in traversal order regardless of the junction order in the graph.
std::vector<Point2D> polygonOrdered(const std::vector<Point2D>& pts) {
    if (pts.size() < 3) {
        return pts;
    }
    float cx = 0.0F;
    float cy = 0.0F;
    for (const auto& p : pts) {
        cx += p.x;
        cy += p.y;
    }
    const float n = static_cast<float>(pts.size());
    cx /= n;
    cy /= n;
    std::vector<Point2D> sorted = pts;
    std::sort(sorted.begin(), sorted.end(), [cx, cy](const Point2D& a, const Point2D& b) {
        return std::atan2(a.y - cy, a.x - cx) < std::atan2(b.y - cy, b.x - cx);
    });
    return sorted;
}

void dropBelowMinConfidence(std::vector<DetectedPattern>& pats, float min_confidence) {
    pats.erase(std::remove_if(pats.begin(), pats.end(),
                              [min_confidence](const DetectedPattern& p) { return p.confidence < min_confidence; }),
               pats.end());
}

// Accumulates named soft-filter scores and their product for a candidate.
class ScoreTrace {
public:
    void add(const char* name, float score) {
        trace_.push_back(FilterScore{name, score});
        product_ *= score;
    }
    [[nodiscard]] float product() const { return product_; }
    [[nodiscard]] std::vector<FilterScore> take() { return std::move(trace_); }

private:
    std::vector<FilterScore> trace_;
    float product_{1.0F};
};

// Junction positions shared by at least two of the given segments (vertex set for
// cycle candidates).
// Junction positions keyed by segment pair: consecutive sides of a cycle meet at known
// junctions, so vertices come from this map instead of a per-candidate scan over all
// junctions. A pair sharing two junctions (an island) stores both positions.
using PairPositions = std::unordered_map<long long, std::vector<Point2D>>;

PairPositions buildPairPositions(const RidgeGraph& graph) {
    PairPositions map;
    for (const auto& j : graph.junctions) {
        const auto& ids = j.connected_segment_ids;
        for (size_t a = 0; a + 1 < ids.size(); ++a) {
            for (size_t b = a + 1; b < ids.size(); ++b) {
                const long long lo = std::min(ids[a], ids[b]);
                const long long hi = std::max(ids[a], ids[b]);
                map[lo * 100000L + hi].push_back(j.position);
            }
        }
    }
    return map;
}

const std::vector<Point2D>* pairPositions(const PairPositions& map, int a, int b) {
    const long long lo = std::min(a, b);
    const long long hi = std::max(a, b);
    const auto it = map.find(lo * 100000L + hi);
    return (it != map.end()) ? &it->second : nullptr;
}

// True when the given vertex sets (one per cycle corner) span a region small enough that
// the size filters could ever accept it. Pure prune: anything rejected here would fail
// the validator's size bound anyway (>2x margin).
bool cycleSpreadWithin(const std::vector<const std::vector<Point2D>*>& corners, float max_extent_px) {
    std::vector<Point2D> pts;
    for (const auto* c : corners) {
        if (c == nullptr) {
            return true; // unknown geometry: do not prune
        }
        for (const auto& p : *c) {
            pts.push_back(p);
        }
    }
    if (pts.empty()) {
        return true;
    }
    float max_d2 = 0.0F;
    for (size_t i = 0; i < pts.size(); ++i) {
        for (size_t j = i + 1; j < pts.size(); ++j) {
            const float dx = pts[i].x - pts[j].x;
            const float dy = pts[i].y - pts[j].y;
            max_d2 = std::max(max_d2, dx * dx + dy * dy);
        }
    }
    return max_d2 <= max_extent_px * max_extent_px;
}

std::vector<DetectedPattern> findTrianglesFast(const RidgeGraph& graph) {
    const auto adj = buildSegmentAdjacency(graph);
    const PairPositions pair_pos = buildPairPositions(graph);
    const float graph_extent = 0.55F * static_cast<float>(graph.debug_mask.cols ? graph.debug_mask.cols : 512);
    std::vector<DetectedPattern> out;
    for (const auto& [a, neighbors_a] : adj) {
        for (int b : neighbors_a) {
            if (b <= a) {
                continue;
            }
            const auto it_b = adj.find(b);
            if (it_b == adj.end()) {
                continue;
            }
            for (int c : it_b->second) {
                if (c <= b) {
                    continue;
                }
                if (neighbors_a.count(c) == 0) {
                    continue;
                }
                const std::vector<Point2D>* ab = pairPositions(pair_pos, a, b);
                const std::vector<Point2D>* bc = pairPositions(pair_pos, b, c);
                const std::vector<Point2D>* ca = pairPositions(pair_pos, c, a);
                if (!cycleSpreadWithin({ab, bc, ca}, 0.35F * graph_extent)) {
                    continue;
                }
                DetectedPattern pat;
                pat.type = PatternType::Triangle;
                pat.name = "Triangle";
                pat.confidence = 0.90f;
                pat.source_segment_ids = {a, b, c};
                for (const auto* corner : {ab, bc, ca}) {
                    if (corner != nullptr) {
                        pat.points.insert(pat.points.end(), corner->begin(), corner->end());
                    }
                }
                pat.centroid = computeCentroid(pat.points);
                out.push_back(std::move(pat));
            }
        }
    }
    return out;
}

// Adjacency-driven 4-cycle enumeration: u is the minimum id of the cycle, v < x the two
// neighbors of u, w closes the ring via adj(v) ∩ adj(x). Cost is bounded by the local
// junction degree instead of C(k,4).
std::vector<DetectedPattern> findSquaresFast(const RidgeGraph& graph) {
    const auto adj = buildSegmentAdjacency(graph);
    const PairPositions pair_pos = buildPairPositions(graph);
    const float graph_extent = 0.55F * static_cast<float>(graph.debug_mask.cols ? graph.debug_mask.cols : 512);
    std::vector<DetectedPattern> out;
    for (const auto& [u, neighbors_u] : adj) {
        if (neighbors_u.size() < 2) {
            continue;
        }
        for (auto v_it = neighbors_u.begin(); v_it != neighbors_u.end(); ++v_it) {
            const int v = *v_it;
            const auto it_v = adj.find(v);
            if (it_v == adj.end()) {
                continue;
            }
            for (auto x_it = std::next(v_it); x_it != neighbors_u.end(); ++x_it) {
                const int x = *x_it;
                const auto it_x = adj.find(x);
                if (it_x == adj.end()) {
                    continue;
                }
                for (int w : it_v->second) {
                    if (w <= u || w == v || w == x) {
                        continue;
                    }
                    if (it_x->second.count(w) == 0) {
                        continue;
                    }
                    const std::vector<Point2D>* uv = pairPositions(pair_pos, u, v);
                    const std::vector<Point2D>* vw = pairPositions(pair_pos, v, w);
                    const std::vector<Point2D>* wx = pairPositions(pair_pos, w, x);
                    const std::vector<Point2D>* xu = pairPositions(pair_pos, x, u);
                    if (!cycleSpreadWithin({uv, vw, wx, xu}, 0.35F * graph_extent)) {
                        continue;
                    }
                    DetectedPattern pat;
                    pat.type = PatternType::Square;
                    pat.name = "Square";
                    pat.confidence = 0.85f;
                    pat.source_segment_ids = {u, v, w, x};
                    for (const auto* corner : {uv, vw, wx, xu}) {
                        if (corner != nullptr) {
                            pat.points.insert(pat.points.end(), corner->begin(), corner->end());
                        }
                    }
                    pat.centroid = computeCentroid(pat.points);
                    out.push_back(std::move(pat));
                }
            }
        }
    }
    return out;
}

void validateIsland(DetectedPattern& pat, const RidgeGraph& graph, const GraphLookup& lookup,
                    const MicroSignConfig& cfg) {
    if (graph.segments.empty()) {
        return; // legacy synthetic graph: no geometry to validate, keep topology prior
    }
    const std::vector<Point2D> loop = islandLoopPoints(lookup, pat);
    if (loop.size() < 3) {
        pat.confidence = 0.0F; // collapsed loop in a real graph: reject
        pat.filter_trace = {FilterScore{"degenerate", 0.0F}};
        return;
    }
    const LoopStats stats = computeLoopStats(loop);
    const MicroSignRanges& r = cfg.ranges;
    ScoreTrace trace;
    trace.add("size", softRange(cfg.scale.norm(std::sqrt(std::max(stats.area_px, 0.0F))), r.island_size_lo,
                                r.island_size_hi, r.island_size_margin));
    trace.add("elong", softRange(stats.elongation, r.island_elong_lo, r.island_elong_hi, r.island_elong_margin));
    pat.filter_trace = trace.take();
    pat.confidence = topologyPrior(pat.type) * trace.product();
}

void validateTriangle(DetectedPattern& pat, const RidgeGraph& graph, const GraphLookup& lookup,
                      const MicroSignConfig& cfg) {
    if (graph.segments.empty()) {
        return; // legacy synthetic graph: no geometry to validate, keep topology prior
    }
    const MicroSignRanges& r = cfg.ranges;
    const std::vector<Point2D> ordered = polygonOrdered(pat.points);
    if (ordered.size() < 3) {
        pat.confidence = 0.0F; // degenerate cycle (e.g. segments through one junction)
        return;
    }
    const LoopStats poly = computeLoopStats(ordered);
    ScoreTrace trace;
    trace.add("size", softRange(cfg.scale.norm(std::sqrt(std::max(poly.area_px, 0.0F))), r.tri_size_lo, r.tri_size_hi,
                                r.tri_size_margin));

    if (ordered.size() == 3) {
        float min_angle = 180.0F;
        float max_angle = 0.0F;
        for (size_t i = 0; i < 3; ++i) {
            const float a = angleAtVertex(ordered[(i + 2) % 3], ordered[i], ordered[(i + 1) % 3]);
            min_angle = std::min(min_angle, a);
            max_angle = std::max(max_angle, a);
        }
        trace.add("angle_lo", softRange(min_angle, r.tri_angle_lo, r.tri_angle_hi, r.tri_angle_margin));
        trace.add("angle_hi", softRange(max_angle, r.tri_angle_lo, r.tri_angle_hi, r.tri_angle_margin));
    }

    bool have_straightness = false;
    float straight_score = 1.0F;
    for (int id : pat.source_segment_ids) {
        const CurveSegment* seg = lookup.segment(id);
        if (seg == nullptr || seg->points.size() < 3) {
            continue;
        }
        have_straightness = true;
        straight_score = std::min(
            straight_score, softRange(maxChordDeviation(seg->points), 0.0F, r.tri_straight_hi, r.tri_straight_margin));
    }
    if (have_straightness) {
        trace.add("straight", straight_score);
    }

    if (degree4VertexCount(lookup, ordered) > r.tri_max_degree4_vertices) {
        trace.add("not_net", 0.0F); // grid cell, not a sign
    }
    pat.filter_trace = trace.take();
    pat.confidence = topologyPrior(pat.type) * trace.product();
}

void validateSquare(DetectedPattern& pat, const RidgeGraph& graph, const GraphLookup& lookup,
                    const MicroSignConfig& cfg) {
    if (graph.segments.empty()) {
        return; // legacy synthetic graph: no geometry to validate, keep topology prior
    }
    const MicroSignRanges& r = cfg.ranges;
    const std::vector<Point2D> ordered = polygonOrdered(pat.points);
    if (ordered.size() < 4) {
        pat.confidence = 0.0F; // degenerate cycle (e.g. segments through one junction)
        return;
    }
    const LoopStats poly = computeLoopStats(ordered);
    ScoreTrace trace;
    trace.add("size", softRange(cfg.scale.norm(std::sqrt(std::max(poly.area_px, 0.0F))), r.sq_size_lo, r.sq_size_hi,
                                r.sq_size_margin));

    if (ordered.size() == 4) {
        float min_angle = 180.0F;
        float max_angle = 0.0F;
        float min_side = -1.0F;
        float max_side = 0.0F;
        for (size_t i = 0; i < 4; ++i) {
            const float a = angleAtVertex(ordered[(i + 3) % 4], ordered[i], ordered[(i + 1) % 4]);
            min_angle = std::min(min_angle, a);
            max_angle = std::max(max_angle, a);
            const float dx = ordered[(i + 1) % 4].x - ordered[i].x;
            const float dy = ordered[(i + 1) % 4].y - ordered[i].y;
            const float side = std::sqrt(dx * dx + dy * dy);
            min_side = (min_side < 0.0F) ? side : std::min(min_side, side);
            max_side = std::max(max_side, side);
        }
        trace.add("angle_lo", softRange(min_angle, r.sq_angle_lo, r.sq_angle_hi, r.sq_angle_margin));
        trace.add("angle_hi", softRange(max_angle, r.sq_angle_lo, r.sq_angle_hi, r.sq_angle_margin));
        if (min_side > 1e-3F) {
            trace.add("side_ratio", softRange(max_side / min_side, 0.0F, r.sq_side_ratio_hi, r.sq_side_ratio_margin));
        }
    }

    if (degree4VertexCount(lookup, ordered) > r.sq_max_degree4_vertices) {
        trace.add("not_net", 0.0F); // grid cell, not a sign
    }
    pat.filter_trace = trace.take();
    pat.confidence = topologyPrior(pat.type) * trace.product();
}

void validateJunctionFan(DetectedPattern& pat, const GraphLookup& lookup, const MicroSignConfig& cfg) {
    const Junction* junc = lookup.junctionNear(pat.centroid);
    if (junc == nullptr) {
        return; // no arm geometry available -> keep topology prior
    }
    const std::vector<ArmDir> arms = armsAt(lookup, *junc);
    if (arms.size() < 3) {
        return;
    }
    const MicroSignRanges& r = cfg.ranges;

    float min_pairwise = 180.0F;
    float max_pairwise = 0.0F;
    float sum_x = 0.0F;
    float sum_y = 0.0F;
    float min_len = -1.0F;
    float max_len = 0.0F;
    for (size_t i = 0; i < arms.size(); ++i) {
        sum_x += arms[i].ux;
        sum_y += arms[i].uy;
        min_len = (min_len < 0.0F) ? arms[i].length : std::min(min_len, arms[i].length);
        max_len = std::max(max_len, arms[i].length);
        for (size_t j = i + 1; j < arms.size(); ++j) {
            const float a = acuteBetween(arms[i].ux, arms[i].uy, arms[j].ux, arms[j].uy);
            min_pairwise = std::min(min_pairwise, a);
            max_pairwise = std::max(max_pairwise, a);
        }
    }

    ScoreTrace trace;
    if (pat.type == PatternType::Cross && arms.size() == 4) {
        // Pair the 4 arms into the 2 most collinear strokes; score the stroke axes.
        static constexpr size_t kPairings[3][4] = {{0, 1, 2, 3}, {0, 2, 1, 3}, {0, 3, 1, 2}};
        float best_collinearity = -2.0F;
        size_t best = 0;
        for (size_t p = 0; p < 3; ++p) {
            const auto& idx = kPairings[p];
            const float c1 = std::fabs(arms[idx[0]].ux * arms[idx[1]].ux + arms[idx[0]].uy * arms[idx[1]].uy);
            const float c2 = std::fabs(arms[idx[2]].ux * arms[idx[3]].ux + arms[idx[2]].uy * arms[idx[3]].uy);
            const float total = c1 + c2;
            if (total > best_collinearity) {
                best_collinearity = total;
                best = p;
            }
        }
        const auto& idx = kPairings[best];
        // Stroke-axis angle: acute angle between the leading arm of each paired stroke
        // (acuteBetween is sign-agnostic, so either arm works as the axis representative).
        const float stroke_acute = acuteBetween(arms[idx[0]].ux, arms[idx[0]].uy, arms[idx[2]].ux, arms[idx[2]].uy);
        trace.add("angle", softRange(stroke_acute, r.cross_acute_lo, r.cross_acute_hi, r.cross_acute_margin));
        trace.add("arm_lo", softRange(cfg.scale.norm(min_len), r.cross_arm_lo, r.cross_arm_hi, r.cross_arm_margin));
        trace.add("arm_hi", softRange(cfg.scale.norm(max_len), r.cross_arm_lo, r.cross_arm_hi, r.cross_arm_margin));

        float straight_score = 1.0F;
        bool have_straightness = false;
        for (int id : pat.source_segment_ids) {
            const CurveSegment* seg = lookup.segment(id);
            if (seg == nullptr || seg->points.size() < 3) {
                continue;
            }
            have_straightness = true;
            straight_score = std::min(straight_score, softRange(maxChordDeviation(seg->points), 0.0F,
                                                                r.cross_straight_hi, r.cross_straight_margin));
        }
        if (have_straightness) {
            trace.add("straight", straight_score);
        }
    } else if (pat.type == PatternType::Star) {
        trace.add("arms",
                  softRange(static_cast<float>(arms.size()), r.star_arms_lo, r.star_arms_hi, r.star_arms_margin));
        trace.add("gap", softRange(minAdjacentGapDeg(arms), r.star_pairwise_lo, 360.0F, r.star_pairwise_margin));
        trace.add("arm_lo", softRange(cfg.scale.norm(min_len), r.star_arm_lo, r.star_arm_hi, r.star_arm_margin));
        trace.add("arm_hi", softRange(cfg.scale.norm(max_len), r.star_arm_lo, r.star_arm_hi, r.star_arm_margin));
    } else if (pat.type == PatternType::Trident && arms.size() == 3) {
        trace.add("pairwise_lo", softRange(min_pairwise, r.trident_pairwise_lo, 180.0F, r.trident_pairwise_margin));
        trace.add("pairwise_hi", softRange(max_pairwise, 0.0F, r.trident_pairwise_hi, r.trident_pairwise_margin));
        const float n = static_cast<float>(arms.size());
        const float coherence = std::sqrt(sum_x * sum_x + sum_y * sum_y) / n;
        trace.add("coherence", softRange(coherence, r.trident_coherence_lo, 1.0F, r.trident_coherence_margin));
        if (min_len > 1e-3F) {
            trace.add("len_ratio",
                      softRange(max_len / min_len, 0.0F, r.trident_len_ratio_hi, r.trident_len_ratio_margin));
        }
        trace.add("prong_lo",
                  softRange(cfg.scale.norm(min_len), r.trident_prong_lo, r.trident_prong_hi, r.trident_prong_margin));
        trace.add("prong_hi",
                  softRange(cfg.scale.norm(max_len), r.trident_prong_lo, r.trident_prong_hi, r.trident_prong_margin));
    } else {
        return; // degree/shape mismatch -> keep topology prior
    }
    pat.filter_trace = trace.take();
    pat.confidence = topologyPrior(pat.type) * trace.product();
}

// Validated nets: clusters of cross junctions connected through shared segments, with
// palm-width-normalized nearest-neighbor spacing (legacy emitted one global net).
std::vector<DetectedPattern> findNetsValidated(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    const MicroSignRanges& r = cfg.ranges;
    std::vector<const Junction*> crosses;
    for (const auto& j : graph.junctions) {
        if (j.connected_segment_ids.size() >= 4) {
            crosses.push_back(&j);
        }
    }
    if (static_cast<int>(crosses.size()) < r.net_min_junctions) {
        return {};
    }

    const float radius = cfg.scale.px(r.net_cluster_radius_norm);
    std::vector<bool> used(crosses.size(), false);
    std::vector<DetectedPattern> nets;
    for (size_t seed = 0; seed < crosses.size(); ++seed) {
        if (used[seed]) {
            continue;
        }
        std::vector<const Junction*> cluster{crosses[seed]};
        used[seed] = true;
        bool grew = true;
        while (grew) {
            grew = false;
            float cx = 0.0F;
            float cy = 0.0F;
            for (const auto* j : cluster) {
                cx += j->position.x;
                cy += j->position.y;
            }
            const float n = static_cast<float>(cluster.size());
            cx /= n;
            cy /= n;
            for (size_t i = 0; i < crosses.size(); ++i) {
                if (used[i]) {
                    continue;
                }
                const float dx = crosses[i]->position.x - cx;
                const float dy = crosses[i]->position.y - cy;
                if (dx * dx + dy * dy <= radius * radius) {
                    cluster.push_back(crosses[i]);
                    used[i] = true;
                    grew = true;
                }
            }
        }
        if (static_cast<int>(cluster.size()) < r.net_min_junctions) {
            continue;
        }

        float nn_sum = 0.0F;
        for (const auto* j : cluster) {
            float nn = -1.0F;
            for (const auto* other : cluster) {
                if (other == j) {
                    continue;
                }
                const float dx = other->position.x - j->position.x;
                const float dy = other->position.y - j->position.y;
                const float d = std::sqrt(dx * dx + dy * dy);
                nn = (nn < 0.0F) ? d : std::min(nn, d);
            }
            if (nn >= 0.0F) {
                nn_sum += nn;
            }
        }
        const float count = static_cast<float>(cluster.size());
        const float mean_nn = nn_sum / count;

        int shared_links = 0;
        for (size_t i = 0; i < cluster.size(); ++i) {
            for (size_t j = i + 1; j < cluster.size(); ++j) {
                for (int id : cluster[i]->connected_segment_ids) {
                    if (std::find(cluster[j]->connected_segment_ids.begin(), cluster[j]->connected_segment_ids.end(),
                                  id) != cluster[j]->connected_segment_ids.end()) {
                        ++shared_links;
                        break;
                    }
                }
            }
        }

        ScoreTrace trace;
        trace.add("spacing",
                  softRange(cfg.scale.norm(mean_nn), r.net_spacing_lo, r.net_spacing_hi, r.net_spacing_margin));
        if (shared_links < r.net_min_shared_links) {
            trace.add("links", 0.0F); // unconnected crosses are signs, not a grille
        }
        const float confidence = topologyPrior(PatternType::Net) * trace.product();
        if (confidence < r.min_confidence) {
            continue;
        }

        DetectedPattern pat;
        pat.type = PatternType::Net;
        pat.name = "Net";
        pat.confidence = confidence;
        pat.filter_trace = trace.take();
        std::vector<Point2D> pts;
        pts.reserve(cluster.size());
        std::set<int> seg_ids;
        for (const auto* j : cluster) {
            pts.push_back(j->position);
            seg_ids.insert(j->connected_segment_ids.begin(), j->connected_segment_ids.end());
        }
        pat.points = std::move(pts);
        pat.centroid = computeCentroid(pat.points);
        pat.source_segment_ids.assign(seg_ids.begin(), seg_ids.end());
        nets.push_back(std::move(pat));
    }
    return nets;
}

// Validated fish: elongated island/triangle body with a nearby degree-4 tail junction.
// (Legacy considered triangle bodies only; islands carry the "island_fish" flag too.)
std::vector<DetectedPattern> findFishValidated(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    const MicroSignRanges& r = cfg.ranges;
    const GraphLookup lookup(graph);
    std::vector<DetectedPattern> out;
    auto try_tail = [&](DetectedPattern body, bool island_body) {
        const Junction* tail = nullptr;
        float tail_d = -1.0F;
        for (const auto& j : graph.junctions) {
            if (j.connected_segment_ids.size() < 4) {
                continue;
            }
            const float dx = j.position.x - body.centroid.x;
            const float dy = j.position.y - body.centroid.y;
            const float d = std::sqrt(dx * dx + dy * dy);
            if (tail_d < 0.0F || d < tail_d) {
                tail_d = d;
                tail = &j;
            }
        }
        if (tail == nullptr) {
            return;
        }
        ScoreTrace trace;
        trace.add("tail_dist", softRange(cfg.scale.norm(tail_d), 0.0F, r.fish_tail_dist_hi, r.fish_tail_dist_margin));

        const std::vector<Point2D> loop = island_body ? islandLoopPoints(lookup, body) : body.points;
        if (loop.size() >= 3) {
            const LoopStats stats = computeLoopStats(loop);
            trace.add("body_size", softRange(cfg.scale.norm(std::sqrt(std::max(stats.area_px, 0.0F))), r.fish_size_lo,
                                             r.fish_size_hi, r.fish_size_margin));
            trace.add("aspect", softRange(stats.elongation, r.fish_aspect_lo, 8.0F, r.fish_aspect_margin));
        }

        const float confidence = topologyPrior(PatternType::Fish) * trace.product();
        if (confidence < r.min_confidence) {
            return;
        }
        body.type = PatternType::Fish;
        body.name = "Fish (Matsya)";
        body.confidence = confidence;
        body.filter_trace = trace.take();
        body.points.push_back(tail->position);
        out.push_back(std::move(body));
    };

    for (auto& tri : MicroPatternDetector::findTriangles(graph, cfg)) {
        try_tail(std::move(tri), false);
    }
    for (auto& isl : MicroPatternDetector::findIslands(graph, cfg)) {
        try_tail(std::move(isl), true);
    }
    return out;
}

// Fraction of the smaller candidate's source segments shared with the other candidate.
float supportOverlap(const DetectedPattern& a, const DetectedPattern& b) {
    if (a.source_segment_ids.empty() || b.source_segment_ids.empty()) {
        return 0.0F;
    }
    size_t shared = 0;
    for (int id : a.source_segment_ids) {
        if (std::find(b.source_segment_ids.begin(), b.source_segment_ids.end(), id) != b.source_segment_ids.end()) {
            ++shared;
        }
    }
    const size_t smaller = std::min(a.source_segment_ids.size(), b.source_segment_ids.size());
    return (smaller > 0) ? static_cast<float>(shared) / static_cast<float>(smaller) : 0.0F;
}

// Same-locus dedup: candidates sharing more than `max_overlap` of the smaller support
// are one physical mark; the lower-confidence one is dropped. Nets are excluded on both
// sides — a grille's support is the union of its junctions' pieces, so it would swamp
// (or be swamped by) every cross inside it.
void dedupeOverlappingSupport(std::vector<DetectedPattern>& pats, float max_overlap) {
    std::stable_sort(pats.begin(), pats.end(),
                     [](const DetectedPattern& a, const DetectedPattern& b) { return a.confidence > b.confidence; });
    std::vector<DetectedPattern> kept;
    kept.reserve(pats.size());
    for (auto& p : pats) {
        if (p.type == PatternType::Net) {
            kept.push_back(std::move(p));
            continue;
        }
        bool dominated = false;
        for (const auto& k : kept) {
            if (k.type != PatternType::Net && supportOverlap(p, k) >= max_overlap) {
                dominated = true;
                break;
            }
        }
        if (!dominated) {
            kept.push_back(std::move(p));
        }
    }
    pats = std::move(kept);
}

} // namespace

std::vector<DetectedPattern> MicroPatternDetector::findIslands(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    auto pats = findIslands(graph);
    const GraphLookup lookup(graph);
    for (auto& pat : pats) {
        validateIsland(pat, graph, lookup, cfg);
    }
    dropBelowMinConfidence(pats, cfg.ranges.min_confidence);
    return pats;
}

std::vector<DetectedPattern> MicroPatternDetector::findTriangles(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    auto pats = findTrianglesFast(graph);
    const GraphLookup lookup(graph);
    for (auto& pat : pats) {
        validateTriangle(pat, graph, lookup, cfg);
    }
    dropBelowMinConfidence(pats, cfg.ranges.min_confidence);
    return pats;
}

std::vector<DetectedPattern> MicroPatternDetector::findSquares(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    auto pats = findSquaresFast(graph);
    const GraphLookup lookup(graph);
    for (auto& pat : pats) {
        validateSquare(pat, graph, lookup, cfg);
    }
    dropBelowMinConfidence(pats, cfg.ranges.min_confidence);
    return pats;
}

std::vector<DetectedPattern> MicroPatternDetector::findCrosses(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    auto pats = findCrosses(graph);
    const GraphLookup lookup(graph);
    for (auto& pat : pats) {
        validateJunctionFan(pat, lookup, cfg);
    }
    dropBelowMinConfidence(pats, cfg.ranges.min_confidence);
    return pats;
}

std::vector<DetectedPattern> MicroPatternDetector::findStars(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    auto pats = findStars(graph);
    const GraphLookup lookup(graph);
    for (auto& pat : pats) {
        validateJunctionFan(pat, lookup, cfg);
    }
    dropBelowMinConfidence(pats, cfg.ranges.min_confidence);
    return pats;
}

std::vector<DetectedPattern> MicroPatternDetector::findTridents(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    auto pats = findTridents(graph);
    const GraphLookup lookup(graph);
    for (auto& pat : pats) {
        validateJunctionFan(pat, lookup, cfg);
    }
    dropBelowMinConfidence(pats, cfg.ranges.min_confidence);
    return pats;
}

std::vector<DetectedPattern> MicroPatternDetector::findNets(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    return findNetsValidated(graph, cfg);
}

std::vector<DetectedPattern> MicroPatternDetector::findFish(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    return findFishValidated(graph, cfg);
}

std::vector<DetectedPattern> MicroPatternDetector::detectAll(const RidgeGraph& graph, const MicroSignConfig& cfg) {
    std::vector<DetectedPattern> results;

    auto push_all = [&](std::vector<DetectedPattern>&& pats) {
        results.insert(results.end(), std::make_move_iterator(pats.begin()), std::make_move_iterator(pats.end()));
    };

    push_all(findFish(graph, cfg));
    push_all(findIslands(graph, cfg));
    push_all(findTriangles(graph, cfg));
    push_all(findSquares(graph, cfg));
    push_all(findStars(graph, cfg));
    push_all(findCrosses(graph, cfg));
    push_all(findTridents(graph, cfg));
    push_all(findNets(graph, cfg));

    dedupeOverlappingSupport(results, cfg.ranges.support_overlap_max);
    return results;
}

std::vector<DetectedPattern> MicroPatternDetector::detectAll(const RidgeGraph& graph) {
    std::vector<DetectedPattern> results;

    auto push_all = [&](std::vector<DetectedPattern>&& pats) {
        results.insert(results.end(), std::make_move_iterator(pats.begin()), std::make_move_iterator(pats.end()));
    };

    push_all(findFish(graph));
    push_all(findIslands(graph));
    push_all(findTriangles(graph));
    push_all(findSquares(graph));
    push_all(findStars(graph));
    push_all(findCrosses(graph));
    push_all(findTridents(graph));
    push_all(findNets(graph));

    return results;
}

} // namespace CurvEngine::pattern
