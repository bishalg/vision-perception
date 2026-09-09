#include "curv/domain/HandAnatomyDecomposer.hpp"
#include "curv/adapters/HandLandmarkDetector.hpp"
#include <gtest/gtest.h>

using namespace CurvEngine;
using namespace CurvEngine::domain;
using namespace CurvEngine::geometry;
using namespace CurvEngine::adapters;

TEST(HandAnatomyDecomposerTest, EmptyLandmarksReturnInvalid) {
    HandLandmarks empty;
    auto const res = HandAnatomyDecomposer::decompose(empty);
    EXPECT_FALSE(res.is_valid);
    EXPECT_TRUE(res.digits.empty());
    EXPECT_TRUE(res.basal_mcp_crease.empty());
    EXPECT_TRUE(res.central_palm_disc.empty());
}

TEST(HandAnatomyDecomposerTest, DecomposesRightHandLandmarks) {
    const int w = 512;
    const int h = 512;
    auto const lms = HandLandmarkDetector::generateHeuristicLandmarks(w, h, false);

    auto const res = HandAnatomyDecomposer::decompose(lms, static_cast<float>(w), static_cast<float>(h));
    EXPECT_TRUE(res.is_valid);

    // Verify 5 digits are present
    ASSERT_EQ(res.digits.size(), 5);
    EXPECT_EQ(res.digits[0].name, "Thumb");
    EXPECT_EQ(res.digits[1].name, "Index");
    EXPECT_EQ(res.digits[2].name, "Middle");
    EXPECT_EQ(res.digits[3].name, "Ring");
    EXPECT_EQ(res.digits[4].name, "Pinky");

    // Each digit should have valid joints
    for (const auto& d : res.digits) {
        EXPECT_GT(d.joint_chain.size(), 1);
        EXPECT_GT(d.tip.y, 0.0f);
        EXPECT_LT(d.tip.y, static_cast<float>(h));
    }

    // Basal MCP crease arc should contain 4 points (Index to Pinky MCPs)
    ASSERT_EQ(res.basal_mcp_crease.size(), 4);

    // Central palm disc should be a valid polygon
    EXPECT_GE(res.central_palm_disc.size(), 4);

    // Palm center (approx 256, 320) should be inside palm disc
    EXPECT_TRUE(HandAnatomyDecomposer::isInsidePalmDisc(Point2D(256.0f, 320.0f), res.central_palm_disc));

    // Distant point (outside hand, top corner) should be outside palm disc
    EXPECT_FALSE(HandAnatomyDecomposer::isInsidePalmDisc(Point2D(10.0f, 10.0f), res.central_palm_disc));

    // Fingertip (middle tip) should be outside central palm disc
    EXPECT_FALSE(HandAnatomyDecomposer::isInsidePalmDisc(res.digits[2].tip, res.central_palm_disc));
}

TEST(HandAnatomyDecomposerTest, DecomposesLeftHandLandmarks) {
    const int w = 512;
    const int h = 512;
    auto const lms = HandLandmarkDetector::generateHeuristicLandmarks(w, h, true);

    auto const res = HandAnatomyDecomposer::decompose(lms, static_cast<float>(w), static_cast<float>(h));
    EXPECT_TRUE(res.is_valid);
    ASSERT_EQ(res.digits.size(), 5);

    // On left hand, Thumb is on the right (+X) and Pinky is on the left (-X)
    EXPECT_GT(res.digits[0].tip.x, res.digits[4].tip.x);

    // Palm center should be inside palm disc
    EXPECT_TRUE(HandAnatomyDecomposer::isInsidePalmDisc(Point2D(256.0f, 320.0f), res.central_palm_disc));
}
