#include <gtest/gtest.h>
#include <cmath>
#include <vector>
#include <opencv2/imgproc.hpp>
#include "curv/geometry/RoiWarper.hpp"
#include "curv/roi/CanonicalWarper.hpp"

using namespace CurvEngine;
using namespace CurvEngine::geometry;

namespace {

// Helper to rotate a point around center
cv::Point2f rotatePoint(const cv::Point2f& pt, const cv::Point2f& center, float rad) {
    const float c = std::cos(rad);
    const float s = std::sin(rad);
    const float dx = pt.x - center.x;
    const float dy = pt.y - center.y;
    return cv::Point2f(center.x + c * dx - s * dy, center.y + s * dx + c * dy);
}

HandLandmarks createRotatedLandmarks(float angle_deg, int img_w, int img_h) {
    const cv::Point2f center(static_cast<float>(img_w) * 0.5f, static_cast<float>(img_h) * 0.5f);
    const float rad = angle_deg * 0.017453292519943295f; // deg to rad

    // Base landmarks in standard upright orientation
    // Wrist at bottom, fingers at top
    cv::Point2f base_p0(center.x, center.y + 80.0f);        // Wrist
    cv::Point2f base_p5(center.x - 45.0f, center.y - 40.0f); // Index MCP
    cv::Point2f base_p9(center.x, center.y - 60.0f);        // Middle MCP
    cv::Point2f base_p13(center.x + 40.0f, center.y - 40.0f); // Ring MCP
    cv::Point2f base_p17(center.x + 65.0f, center.y - 20.0f); // Pinky MCP

    HandLandmarks lms;
    lms.is_normalized = true;

    auto set_pt = [&](int idx, const cv::Point2f& base_pt) {
        cv::Point2f rotated = rotatePoint(base_pt, center, rad);
        lms.points[static_cast<size_t>(idx)] = LandmarkPoint(rotated.x / static_cast<float>(img_w),
                                                            rotated.y / static_cast<float>(img_h));
    };

    set_pt(0, base_p0);
    set_pt(5, base_p5);
    set_pt(9, base_p9);
    set_pt(13, base_p13);
    set_pt(17, base_p17);

    return lms;
}

} // anonymous namespace

TEST(RoiWarperTest, Test_CanonicalTransform_AtMultipleRotations) {
    const int img_w = 640;
    const int img_h = 640;
    const int target_dim = 512;
    const float expected_c = static_cast<float>(target_dim) * 0.5f;

    const std::vector<float> test_angles = {0.0f, 30.0f, 45.0f, 90.0f, -60.0f, 180.0f};

    for (float angle : test_angles) {
        HandLandmarks lms = createRotatedLandmarks(angle, img_w, img_h);
        RoiTransform transform = computeCanonicalPalmTransform(lms, img_w, img_h, target_dim);

        EXPECT_FALSE(transform.M.empty());
        EXPECT_FALSE(transform.M_inv.empty());
        EXPECT_EQ(transform.target_dim, target_dim);

        // 1. Verify centroid maps precisely to canonical center (256, 256)
        const double* m = transform.M.ptr<double>();
        const double cx = static_cast<double>(transform.center.x);
        const double cy = static_cast<double>(transform.center.y);
        const double mapped_cx = m[0] * cx + m[1] * cy + m[2];
        const double mapped_cy = m[3] * cx + m[4] * cy + m[5];

        EXPECT_NEAR(mapped_cx, static_cast<double>(expected_c), 0.05) << "Centroid X mismatch at angle " << angle;
        EXPECT_NEAR(mapped_cy, static_cast<double>(expected_c), 0.05) << "Centroid Y mismatch at angle " << angle;

        // 2. Verify Wrist -> Middle MCP vector is strictly vertical (X coordinates match)
        const auto& p0_norm = lms.points[0];
        const auto& p9_norm = lms.points[9];
        const double p0_x = static_cast<double>(p0_norm.x * static_cast<float>(img_w));
        const double p0_y = static_cast<double>(p0_norm.y * static_cast<float>(img_h));
        const double p9_x = static_cast<double>(p9_norm.x * static_cast<float>(img_w));
        const double p9_y = static_cast<double>(p9_norm.y * static_cast<float>(img_h));

        const double can_p0_x = m[0] * p0_x + m[1] * p0_y + m[2];
        const double can_p0_y = m[3] * p0_x + m[4] * p0_y + m[5];
        const double can_p9_x = m[0] * p9_x + m[1] * p9_y + m[2];
        const double can_p9_y = m[3] * p9_x + m[4] * p9_y + m[5];

        EXPECT_NEAR(can_p0_x, can_p9_x, 0.05)
            << "Wrist and Middle MCP not vertically aligned in canonical space for angle " << angle;
        EXPECT_LT(can_p9_y, can_p0_y)
            << "Middle MCP must be higher (lower Y in screen space) than Wrist for angle " << angle;

        // 3. Verify sub-pixel coordinate inversion (M * M_inv == Identity)
        // Test an arbitrary point in original frame
        const double test_orig_x = 320.45;
        const double test_orig_y = 280.75;

        // Forward transform
        const double can_x = m[0] * test_orig_x + m[1] * test_orig_y + m[2];
        const double can_y = m[3] * test_orig_x + m[4] * test_orig_y + m[5];

        // Inverse transform
        const double* mi = transform.M_inv.ptr<double>();
        const double restored_x = mi[0] * can_x + mi[1] * can_y + mi[2];
        const double restored_y = mi[3] * can_x + mi[4] * can_y + mi[5];

        EXPECT_NEAR(restored_x, test_orig_x, 0.01) << "Inverse X reconstruction error exceeded at angle " << angle;
        EXPECT_NEAR(restored_y, test_orig_y, 0.01) << "Inverse Y reconstruction error exceeded at angle " << angle;
    }
}

TEST(RoiWarperTest, Test_InvertGraphCoordinates) {
    const int target_dim = 512;
    HandLandmarks lms = createRotatedLandmarks(45.0f, 600, 600);
    RoiTransform transform = computeCanonicalPalmTransform(lms, 600, 600, target_dim);

    RidgeGraph graph;
    CurveSegment seg;
    seg.id = 1;
    seg.total_length = 50.0f;
    seg.points.emplace_back(256.0f, 256.0f); // At canonical center
    seg.points.emplace_back(276.0f, 256.0f);
    graph.segments.push_back(seg);

    invertGraphCoordinates(graph, transform.M_inv);

    EXPECT_EQ(graph.segments.size(), 1);
    const auto& restored_pt = graph.segments[0].points[0];

    // Canonical center must map back to transform.center in original frame
    EXPECT_NEAR(restored_pt.x, transform.center.x, 0.05);
    EXPECT_NEAR(restored_pt.y, transform.center.y, 0.05);

    // Segment length scaled correctly by inverse scale factor
    EXPECT_GT(graph.segments[0].total_length, 0.0f);
}

TEST(RoiWarperTest, Test_CanonicalWarperClass_ImageWarp) {
    cv::Mat blank(400, 400, CV_8UC3, cv::Scalar(40, 40, 40));
    // Draw a bright circle
    cv::circle(blank, cv::Point(200, 200), 50, cv::Scalar(220, 220, 220), -1);
    Frame frame(blank, "source_test", 1);

    roi::CanonicalWarper warper(512);

    // Quad warp test
    std::array<Point2D, 4> corners = {
        Point2D{100.0f, 100.0f},
        Point2D{300.0f, 100.0f},
        Point2D{300.0f, 300.0f},
        Point2D{100.0f, 300.0f}
    };
    Frame quad_frame = warper.warpQuad(frame, corners, 512);
    EXPECT_EQ(quad_frame.width(), 512);
    EXPECT_EQ(quad_frame.height(), 512);
    EXPECT_FALSE(quad_frame.empty());

    // Palm warp test
    HandLandmarks lms = createRotatedLandmarks(30.0f, 400, 400);
    Frame palm_frame = warper.warpPalm(frame, lms, 512);
    EXPECT_EQ(palm_frame.width(), 512);
    EXPECT_EQ(palm_frame.height(), 512);
    EXPECT_FALSE(palm_frame.empty());
}
