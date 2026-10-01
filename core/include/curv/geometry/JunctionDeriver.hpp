#pragma once

#include "curv/GeometryTypes.hpp"

namespace CurvEngine::geometry {

/**
 * @brief Derives junctions for a ridge graph whose segments were traced independently.
 *
 * Steger tracing emits disconnected sub-pixel polylines, but topological consumers
 * (micro-pattern detection, fork counting, mount marks) need the connection points.
 * Two implicit sources are recovered:
 *  1. endpoint clusters — segment endpoints within tol_px of each other;
 *  2. through-crossings — polyline pairs intersecting strictly inside both (a crease
 *     traced straight through another). The involved segments are SPLIT at the crossing
 *     so the junction looks like the 4-piece meeting the topology expects; the first
 *     piece keeps the original segment id.
 *
 * graph.junctions is rebuilt from the segment geometry (pre-existing entries are
 * replaced — call this on a segments-only graph before any junction-dependent use).
 *
 * @return Number of junctions in the graph after derivation.
 */
size_t deriveJunctions(RidgeGraph& graph, float tol_px = 2.5F);

} // namespace CurvEngine::geometry
