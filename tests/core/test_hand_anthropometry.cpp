#include "curv/geometry/HandAnthropometry.hpp"
#include "curv/surface/HandMeshGenerator.hpp"
#include <gtest/gtest.h>

namespace {

std::vector<std::array<float, 2>> scaffoldLandmarks() {
    return {
        {0.50f, 0.85f}, // 0: Wrist
        {0.36f, 0.74f}, // 1: Thumb CMC
        {0.28f, 0.63f}, // 2: Thumb MCP
        {0.24f, 0.54f}, // 3: Thumb IP
        {0.21f, 0.46f}, // 4: Thumb Tip
        {0.37f, 0.46f}, // 5: Index MCP
        {0.35f, 0.35f}, // 6: Index PIP
        {0.34f, 0.27f}, // 7: Index DIP
        {0.33f, 0.20f}, // 8: Index Tip
        {0.49f, 0.44f}, // 9: Middle MCP
        {0.49f, 0.32f}, // 10: Middle PIP
        {0.49f, 0.23f}, // 11: Middle DIP
        {0.49f, 0.15f}, // 12: Middle Tip
        {0.60f, 0.46f}, // 13: Ring MCP
        {0.61f, 0.35f}, // 14: Ring PIP
        {0.62f, 0.27f}, // 15: Ring DIP
        {0.63f, 0.21f}, // 16: Ring Tip
        {0.70f, 0.50f}, // 17: Pinky MCP
        {0.72f, 0.42f}, // 18: Pinky PIP
        {0.73f, 0.36f}, // 19: Pinky DIP
        {0.74f, 0.30f}  // 20: Pinky Tip
    };
}

} // anonymous namespace

TEST(HandAnthropometry, ComputesValidDimensionsAndRatios) {
    const auto lm = scaffoldLandmarks();
    const auto report = CurvEngine::geometry::HandAnthropometer::compute(
        lm, 1000, 1000, 80.0f, {{"thenar", 3.2f}, {"radial_interdigital", 2.1f}}, "right"
    );

    EXPECT_EQ(report.hand_side, "right");
    EXPECT_NEAR(report.palm.metacarpal_breadth_mm, 80.0f, 0.01f);
    EXPECT_GT(report.palm.palm_length_mm, 50.0f);
    EXPECT_GT(report.palm.total_hand_length_mm, 120.0f);

    ASSERT_EQ(report.digits.size(), 5);
    EXPECT_EQ(report.digits[0].name, "thumb");
    EXPECT_EQ(report.digits[1].name, "index");
    EXPECT_EQ(report.digits[2].name, "middle");
    EXPECT_EQ(report.digits[3].name, "ring");
    EXPECT_EQ(report.digits[4].name, "pinky");

    // Middle finger is anatomically longest digit
    EXPECT_GT(report.digits[2].total_length_mm, report.digits[1].total_length_mm);
    EXPECT_GT(report.digits[2].total_length_mm, report.digits[3].total_length_mm);
    EXPECT_GT(report.digits[3].total_length_mm, report.digits[4].total_length_mm);

    // 2D:4D ratio should fall within natural human biological bounds (typically 0.88 - 1.05)
    EXPECT_GT(report.ratios.ratio_2d_4d, 0.85f);
    EXPECT_LT(report.ratios.ratio_2d_4d, 1.15f);

    // Anatomical pads
    ASSERT_EQ(report.pads.size(), 7);
    const auto it_thenar = std::find_if(report.pads.begin(), report.pads.end(),
        [](const auto& p) { return p.pad_name == "thenar"; });
    ASSERT_NE(it_thenar, report.pads.end());
    EXPECT_FLOAT_EQ(it_thenar->height_mm, 3.2f);

    // Anatomical flexion creases
    ASSERT_EQ(report.creases.size(), 4);
    EXPECT_EQ(report.creases[0].anatomical_name, "distal_transverse_crease");
    EXPECT_EQ(report.creases[0].traditional_alias, "heart");
    EXPECT_GT(report.creases[0].arc_length_mm, 20.0f);
    EXPECT_EQ(report.creases[1].anatomical_name, "proximal_transverse_crease");
    EXPECT_EQ(report.creases[1].traditional_alias, "head");
    EXPECT_EQ(report.creases[2].anatomical_name, "thenar_longitudinal_crease");
    EXPECT_EQ(report.creases[2].traditional_alias, "life");
    EXPECT_EQ(report.creases[3].anatomical_name, "median_longitudinal_crease");
    EXPECT_EQ(report.creases[3].traditional_alias, "fate");

    // JSON serialization
    const auto json = report.toJson();
    EXPECT_EQ(json["schema_version"], "curv_hand_anthropometry_v1");
    EXPECT_TRUE(json.contains("palm"));
    EXPECT_TRUE(json.contains("digits"));
    EXPECT_TRUE(json.contains("ratios"));
    EXPECT_TRUE(json.contains("creases"));
}

TEST(HandMeshGenerator, Generates3DSurfaceAndObj) {
    const auto lm = scaffoldLandmarks();
    const auto mesh = CurvEngine::surface::HandMeshGenerator::generate(
        lm, {{"thenar", 4.0f}, {"palmar_fossa", -2.0f}}, 80.0f, 1000, 1000
    );

    EXPECT_EQ(mesh.vertices.size(), 22);
    EXPECT_GT(mesh.triangles.size(), 10);
    EXPECT_GT(mesh.max_elevation_mm, mesh.min_elevation_mm);

    // Check vertex colors are within normalized RGB [0..1]
    for (const auto& v : mesh.vertices) {
        EXPECT_GE(v.color[0], 0.0f);
        EXPECT_LE(v.color[0], 1.0f);
        EXPECT_GE(v.color[1], 0.0f);
        EXPECT_LE(v.color[1], 1.0f);
        EXPECT_GE(v.color[2], 0.0f);
        EXPECT_LE(v.color[2], 1.0f);
    }

    const std::string obj = mesh.toWavefrontObj();
    EXPECT_FALSE(obj.empty());
    EXPECT_NE(obj.find("v "), std::string::npos);
    EXPECT_NE(obj.find("vn "), std::string::npos);
    EXPECT_NE(obj.find("f "), std::string::npos);

    const auto json = mesh.toJson();
    EXPECT_TRUE(json.contains("vertices"));
    EXPECT_TRUE(json.contains("triangles"));
    EXPECT_TRUE(json.contains("colors"));
}
