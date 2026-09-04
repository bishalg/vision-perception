#include <gtest/gtest.h>
#include <fstream>
#include "curv/adapters/OverlayRenderer.hpp"

using namespace CurvEngine;
using namespace CurvEngine::adapters;

TEST(OverlayRendererTest, Test_RenderAndSvgExport) {
    // 1. Create a test RidgeGraph with a curved segment
    CurveSegment seg;
    seg.id = 1;
    seg.total_length = 100.0f;
    for (int i = 0; i < 50; ++i) {
        const float fi = static_cast<float>(i);
        seg.points.emplace_back(20.0f + fi * 2.0f, 50.0f + 10.0f * std::sin(fi * 0.1f));
    }

    RidgeGraph graph;
    graph.segments.push_back(seg);

    cv::Mat canvas = cv::Mat::zeros(200, 200, CV_8UC3);
    QualityReport quality{150.0, 100.0, 45.0, true, "OK"};

    OverlayConfig config;
    config.draw_subpixel_polylines = true;
    config.draw_normals = true;
    config.draw_ribbons = true;
    config.draw_hud = true;

    cv::Mat rendered = OverlayRenderer::render(canvas, graph, quality, config);
    EXPECT_FALSE(rendered.empty());
    EXPECT_EQ(rendered.cols, 200);
    EXPECT_EQ(rendered.rows, 200);

    // Test SVG export
    const std::string svg_path = "test_output.svg";
    const bool svg_ok = OverlayRenderer::exportToSvg(graph, 200, 200, svg_path);
    EXPECT_TRUE(svg_ok);

    std::ifstream svg_file(svg_path);
    ASSERT_TRUE(svg_file.is_open());
    std::string svg_content((std::istreambuf_iterator<char>(svg_file)), std::istreambuf_iterator<char>());
    EXPECT_NE(svg_content.find("<svg"), std::string::npos);
    EXPECT_NE(svg_content.find("<polyline"), std::string::npos);
}
