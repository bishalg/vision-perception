#include "curv/pattern/MicroPatternDetector.hpp"
#include <algorithm>
#include <cmath>
#include <map>
#include <set>

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
