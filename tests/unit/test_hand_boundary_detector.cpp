#include "curv/adapters/HandBoundaryDetector.hpp"
#include <gtest/gtest.h>
#include <opencv2/imgproc.hpp>

using namespace CurvEngine;
using namespace CurvEngine::adapters;

TEST(HandBoundaryDetectorTest, EmptyImageReturnsInvalid) {
    cv::Mat const empty;
    auto const res = HandBoundaryDetector::detectBoundary(empty);
    EXPECT_FALSE(res.is_valid);
    EXPECT_TRUE(res.mask.empty());
    EXPECT_TRUE(res.contour.empty());
}

TEST(HandBoundaryDetectorTest, SyntheticSkinPatchDetection) {
    // 200x200 canvas with neutral gray background (desk)
    cv::Mat canvas(200, 200, CV_8UC3, cv::Scalar(180, 180, 180));

    // Draw skin-colored circle in the center (radius 50)
    // Typical skin tone: B=110, G=150, R=220
    cv::circle(canvas, cv::Point(100, 100), 50, cv::Scalar(110, 150, 220), -1);

    auto const res = HandBoundaryDetector::detectBoundary(canvas);
    EXPECT_TRUE(res.is_valid);
    EXPECT_FALSE(res.mask.empty());
    EXPECT_FALSE(res.contour.empty());
    EXPECT_GT(res.area, 6000.0); // pi * 50^2 ~ 7853

    // Check that center pixel is marked inside (255) and corner is outside (0)
    EXPECT_EQ(res.mask.at<uint8_t>(100, 100), 255);
    EXPECT_EQ(res.mask.at<uint8_t>(10, 10), 0);
}

TEST(HandBoundaryDetectorTest, FilterGraphByMaskPrunesNoiseOutside) {
    // 100x100 mask with circular active region at center
    cv::Mat mask = cv::Mat::zeros(100, 100, CV_8UC1);
    cv::circle(mask, cv::Point(50, 50), 30, cv::Scalar(255), -1);

    RidgeGraph graph;

    // Segment 1: completely inside (center)
    CurveSegment seg_inside;
    seg_inside.id = 1;
    seg_inside.points = {Point2D(48.0f, 50.0f), Point2D(50.0f, 50.0f), Point2D(52.0f, 50.0f)};
    graph.segments.push_back(seg_inside);

    // Segment 2: completely outside (top-left corner)
    CurveSegment seg_outside;
    seg_outside.id = 2;
    seg_outside.points = {Point2D(5.0f, 5.0f), Point2D(6.0f, 6.0f), Point2D(7.0f, 7.0f)};
    graph.segments.push_back(seg_outside);

    // Segment 3: mostly outside
    CurveSegment seg_mixed;
    seg_mixed.id = 3;
    seg_mixed.points = {Point2D(50.0f, 50.0f), Point2D(10.0f, 10.0f), Point2D(12.0f, 12.0f), Point2D(14.0f, 14.0f)};
    graph.segments.push_back(seg_mixed);

    EXPECT_EQ(graph.segments.size(), 3);

    HandBoundaryDetector::filterGraphByMask(graph, mask, 0.70f);

    ASSERT_EQ(graph.segments.size(), 1);
    EXPECT_EQ(graph.segments[0].id, 1);
}
