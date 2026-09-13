#include "curv/surface/SurfaceRelief.hpp"

#include <gtest/gtest.h>

TEST(SurfaceRelief, EstimatesRelativeHeights) {
    CurvEngine::surface::OrbitFrameMeta frame;
    frame.bin_id = "thumb_toward";
    frame.gravity_xyz = {0.0f, 0.0f, -1.0f};
    frame.primary_regions = {"venus", "jupiter"};
    const auto samples = CurvEngine::surface::SurfaceRelief::estimateFromOrbitFrames({frame}, {"venus", "mars"});
    ASSERT_EQ(samples.size(), 2u);
    EXPECT_GT(samples[0].view_count, 0);
}
