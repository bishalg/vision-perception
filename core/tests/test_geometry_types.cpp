#include <cassert>
#include <iostream>
#include "curv/GeometryTypes.hpp"

int main() {
    CurvEngine::Point2D pt{10.5f, 20.25f, 0.85f, 0.99f};
    assert(pt.x > 10.0f);
    assert(pt.y > 20.0f);
    assert(pt.confidence > 0.9f);

    CurvEngine::CurveSegment seg;
    seg.id = 1;
    seg.points.push_back(pt);
    seg.total_length = 15.0f;
    seg.average_curvature = 0.05f;
    seg.is_closed = false;
    assert(seg.points.size() == 1);

    CurvEngine::Junction junc;
    junc.position = pt;
    junc.connected_segment_ids = {1, 2};
    junc.branch_angle = 1.57f;
    assert(junc.connected_segment_ids.size() == 2);

    CurvEngine::RidgeGraph graph;
    graph.segments.push_back(seg);
    graph.junctions.push_back(junc);
    assert(graph.segments.size() == 1);
    assert(graph.junctions.size() == 1);

    std::cout << "[PASS] CurvEngine::GeometryTypes basic contract verified." << std::endl;
    return 0;
}
