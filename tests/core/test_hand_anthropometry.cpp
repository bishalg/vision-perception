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

    // Check normal vectors are unit length and finite
    for (const auto& v : mesh.vertices) {
        EXPECT_TRUE(std::isfinite(v.normal[0]));
        EXPECT_TRUE(std::isfinite(v.normal[1]));
        EXPECT_TRUE(std::isfinite(v.normal[2]));
        const float len = std::sqrt(v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2]);
        EXPECT_NEAR(len, 1.0f, 0.05f);
    }
}

TEST(HandAnthropometry, HandlesCorruptAndBoundaryInputs) {
    // 1. Empty landmarks
    const auto empty_report = CurvEngine::geometry::HandAnthropometer::compute({});
    EXPECT_TRUE(empty_report.digits.empty());
    EXPECT_FLOAT_EQ(empty_report.mm_per_pixel, 0.0f);

    // 2. Insufficient landmarks (< 21)
    std::vector<std::array<float, 2>> partial(15, {0.5f, 0.5f});
    const auto partial_report = CurvEngine::geometry::HandAnthropometer::compute(partial);
    EXPECT_TRUE(partial_report.digits.empty());

    // 3. Landmarks containing NaN / Inf
    auto nan_lms = scaffoldLandmarks();
    nan_lms[5][0] = std::numeric_limits<float>::quiet_NaN();
    const auto nan_report = CurvEngine::geometry::HandAnthropometer::compute(nan_lms);
    EXPECT_TRUE(nan_report.digits.empty());
    EXPECT_NO_THROW({
        auto j = nan_report.toJson();
        (void)j.dump();
    });

    auto inf_lms = scaffoldLandmarks();
    inf_lms[17][1] = std::numeric_limits<float>::infinity();
    const auto inf_report = CurvEngine::geometry::HandAnthropometer::compute(inf_lms);
    EXPECT_TRUE(inf_report.digits.empty());

    // 4. Degenerate landmarks (all at same coordinate -> zero breadth)
    std::vector<std::array<float, 2>> collapsed(21, {0.5f, 0.5f});
    const auto collapsed_report = CurvEngine::geometry::HandAnthropometer::compute(collapsed);
    EXPECT_TRUE(collapsed_report.digits.empty());
    EXPECT_FLOAT_EQ(collapsed_report.mm_per_pixel, 0.0f);

    // 5. Negative reference width (should clamp to 80mm fallback)
    const auto valid_lms = scaffoldLandmarks();
    const auto fallback_report = CurvEngine::geometry::HandAnthropometer::compute(valid_lms, 1000, 1000, -50.0f);
    EXPECT_FLOAT_EQ(fallback_report.palm.metacarpal_breadth_mm, 80.0f);
    EXPECT_GT(fallback_report.palm.palm_length_mm, 0.0f);
}

TEST(HandMeshGenerator, HandlesCorruptAndBoundaryInputs) {
    // 1. Empty landmarks
    const auto empty_mesh = CurvEngine::surface::HandMeshGenerator::generate({});
    EXPECT_TRUE(empty_mesh.vertices.empty());
    EXPECT_TRUE(empty_mesh.triangles.empty());

    // 2. NaN landmarks
    auto nan_lms = scaffoldLandmarks();
    nan_lms[0][0] = std::numeric_limits<float>::quiet_NaN();
    const auto nan_mesh = CurvEngine::surface::HandMeshGenerator::generate(nan_lms);
    EXPECT_TRUE(nan_mesh.vertices.empty());
    EXPECT_TRUE(nan_mesh.triangles.empty());

    // 3. Degenerate collapsed landmarks
    std::vector<std::array<float, 2>> collapsed(21, {0.5f, 0.5f});
    const auto collapsed_mesh = CurvEngine::surface::HandMeshGenerator::generate(collapsed);
    EXPECT_TRUE(collapsed_mesh.vertices.empty());
    EXPECT_TRUE(collapsed_mesh.triangles.empty());
}

