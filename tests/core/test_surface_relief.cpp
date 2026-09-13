#include "curv/surface/SurfaceRelief.hpp"

#include <array>
#include <unordered_map>
#include <gtest/gtest.h>

namespace {

// 21-point MediaPipe-order scaffold in normalized coordinates.
std::vector<std::array<float, 2>> scaffoldLandmarks() {
    return {{{0.50f, 0.80f}, {0.35f, 0.65f}, {0.30f, 0.55f}, {0.28f, 0.45f}, {0.27f, 0.35f},
            {0.40f, 0.50f}, {0.38f, 0.40f}, {0.37f, 0.30f}, {0.36f, 0.20f}, {0.50f, 0.48f},
            {0.50f, 0.38f}, {0.50f, 0.28f}, {0.50f, 0.18f}, {0.60f, 0.50f}, {0.62f, 0.40f},
            {0.63f, 0.30f}, {0.64f, 0.20f}, {0.70f, 0.52f}, {0.72f, 0.43f}, {0.73f, 0.34f},
            {0.74f, 0.25f}}};
}

const std::unordered_map<std::string, std::vector<int>>& testAnchors() {
    static const std::unordered_map<std::string, std::vector<int>> kAnchors = {
        {"venus", {1, 2, 5}},      {"jupiter", {5, 6, 7}},   {"saturn", {9, 10, 11}},
        {"apollo", {13, 14, 15}},  {"mercury", {17, 18, 19}}, {"luna", {0, 17, 18}},
        {"mars", {5, 9, 13, 17}}};
    return kAnchors;
}

const CurvEngine::surface::ReliefSample& findSample(const std::vector<CurvEngine::surface::ReliefSample>& samples,
                                                   const std::string& id) {
    for (const auto& s : samples) {
        if (s.region_id == id) {
            return s;
        }
    }
    ADD_FAILURE() << "missing region " << id;
    return samples.at(0);
}

} // anonymous namespace

TEST(SurfaceRelief, EstimatesRelativeHeights) {
    CurvEngine::surface::OrbitFrameMeta frame;
    frame.bin_id = "thumb_toward";
    frame.gravity_xyz = {0.0f, 0.0f, -1.0f};
    frame.primary_regions = {"venus", "jupiter"};
    const auto samples = CurvEngine::surface::SurfaceRelief::estimateFromOrbitFrames({frame}, {"venus", "mars"});
    ASSERT_EQ(samples.size(), 2u);
    EXPECT_GT(samples[0].view_count, 0);
}

TEST(SurfaceRelief, MetricParallaxMeasuresElevatedRegion) {
    using CurvEngine::surface::SurfaceRelief;
    const std::vector<std::string> regions = {"venus", "jupiter", "saturn", "apollo", "mercury", "luna", "mars"};
    const auto ref = scaffoldLandmarks();
    // Tilted view: thenar (venus) anchors displaced outward radially (-x for
    // these left-of-center points), everything else rigid.
    auto tilted = ref;
    for (int idx : {1, 2, 5}) {
        tilted[static_cast<size_t>(idx)][0] -= 0.012f;
    }
    CurvEngine::surface::OrbitFrameMeta frame;
    frame.bin_id = "tilt_left";
    frame.gravity_xyz = {0.3f, 0.1f, -0.94f};
    frame.primary_regions = regions;
    frame.landmarks = tilted;
    frame.image_width = 1000;
    frame.image_height = 1000;
    frame.tilt_rad_vs_ref = 0.3f;
    CurvEngine::surface::MetricScale scale;

    const auto samples = SurfaceRelief::estimateMetricRelief({frame}, ref, 300.0f, scale, regions, testAnchors());
    ASSERT_EQ(samples.size(), 7u);
    const auto& venus = findSample(samples, "venus");
    const auto& luna = findSample(samples, "luna");
    // Parallax magnitude (~8 px -> ~2.3 mm at 80 mm / 300 px scale, / 0.3 rad)
    // clamps to the 6 mm sanity bound and outranks the undisplaced mounts.
    EXPECT_GT(venus.height_mm, 2.0f);
    EXPECT_GT(venus.height_mm, luna.height_mm + 1.0f);
    EXPECT_GT(venus.height_relative, luna.height_relative);
    EXPECT_FALSE(venus.measured_depth);
}

TEST(SurfaceRelief, MetricWithoutReferenceFallsBackToProxy) {
    using CurvEngine::surface::SurfaceRelief;
    CurvEngine::surface::OrbitFrameMeta frame;
    frame.bin_id = "flat_center";
    frame.gravity_xyz = {0.0f, 0.0f, -1.0f};
    frame.primary_regions = {"venus", "mars"};
    CurvEngine::surface::MetricScale scale;
    const auto samples =
        SurfaceRelief::estimateMetricRelief({frame}, {}, 0.0f, scale, {"venus", "mars"}, testAnchors());
    ASSERT_EQ(samples.size(), 2u);
    for (const auto& s : samples) {
        EXPECT_FLOAT_EQ(s.height_mm, -1.0f);
        EXPECT_FALSE(s.measured_depth);
    }
    EXPECT_GT(samples[0].view_count, 0);
}

TEST(SurfaceRelief, MeasuredDepthOverridesParallax) {
    using CurvEngine::surface::SurfaceRelief;
    const std::vector<std::string> regions = {"venus", "mars"};
    CurvEngine::surface::OrbitFrameMeta frame;
    frame.bin_id = "flat_center";
    frame.gravity_xyz = {0.0f, 0.0f, -1.0f};
    frame.primary_regions = regions;
    frame.depth_mm = {{"venus", 3.0f}, {"mars", 1.0f}};
    CurvEngine::surface::MetricScale scale;
    const auto samples =
        SurfaceRelief::estimateMetricRelief({frame}, {}, 0.0f, scale, regions, testAnchors());
    ASSERT_EQ(samples.size(), 2u);
    // Plane = median(3, 1) = 2 -> venus +1, mars -1, both flagged measured.
    EXPECT_FLOAT_EQ(samples[0].height_mm, 1.0f);
    EXPECT_FLOAT_EQ(samples[1].height_mm, -1.0f + 0.0f); // -1: 1.0 - 2.0
    EXPECT_TRUE(samples[0].measured_depth);
    EXPECT_TRUE(samples[1].measured_depth);
}
