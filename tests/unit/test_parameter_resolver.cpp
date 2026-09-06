#include <cmath>

#include <gtest/gtest.h>

#include "curv/config/DynamicParameterResolver.hpp"

using namespace CurvEngine;
using namespace CurvEngine::config;

namespace {

bool sameConfig(const ridge::StegerConfig& a, const ridge::StegerConfig& b) {
    return a.sigma == b.sigma && a.low_threshold == b.low_threshold && a.high_threshold == b.high_threshold &&
           a.min_segment_length == b.min_segment_length && a.extract_dark_lines == b.extract_dark_lines;
}

} // anonymous namespace

TEST(DynamicParameterResolverTest, Test_DefaultCtorUsesDefaultBaseline) {
    const DynamicParameterResolver resolver;
    const auto cfg = resolver.resolve(500.0f); // ratio exactly 1 -> no scaling
    EXPECT_FLOAT_EQ(cfg.sigma, 1.5f);
    EXPECT_FLOAT_EQ(cfg.low_threshold, 0.5f);
    EXPECT_FLOAT_EQ(cfg.high_threshold, 1.5f);
    EXPECT_FLOAT_EQ(cfg.min_segment_length, 5.0f);
    EXPECT_FALSE(cfg.extract_dark_lines);
}

TEST(DynamicParameterResolverTest, Test_CustomBaselineIsRespected) {
    DynamicParameterResolver::BaselineConfig baseline;
    baseline.base_sigma = 2.0f;
    baseline.base_low_thresh = 0.4f;
    baseline.base_high_thresh = 1.2f;
    baseline.min_segment_length = 8.0f;
    baseline.extract_dark_lines = true;
    const DynamicParameterResolver resolver(baseline);

    const auto cfg = resolver.resolve(500.0f);
    EXPECT_FLOAT_EQ(cfg.sigma, 2.0f);
    EXPECT_FLOAT_EQ(cfg.low_threshold, 0.4f);
    EXPECT_FLOAT_EQ(cfg.high_threshold, 1.2f);
    EXPECT_FLOAT_EQ(cfg.min_segment_length, 8.0f);
    EXPECT_TRUE(cfg.extract_dark_lines);
}

TEST(DynamicParameterResolverTest, Test_TinyDiagonalClampsScalesUpToMinimum) {
    const DynamicParameterResolver resolver;
    // ratio 10/500 = 0.02 -> sqrt 0.14 clamps to 0.5; ratio itself clamps to 0.5.
    const auto cfg = resolver.resolve(10.0f);
    EXPECT_FLOAT_EQ(cfg.sigma, 0.75f);
    EXPECT_FLOAT_EQ(cfg.low_threshold, 0.25f);
    EXPECT_FLOAT_EQ(cfg.high_threshold, 0.75f);
    EXPECT_FLOAT_EQ(cfg.min_segment_length, 2.5f);
}

TEST(DynamicParameterResolverTest, Test_HugeDiagonalClampsScalesDownToMaximum) {
    const DynamicParameterResolver resolver;
    // ratio 6480/500 = 12.96 -> sqrt 3.6 clamps to 3.5; ratio clamps to 2.5.
    const auto cfg = resolver.resolve(6480.0f);
    EXPECT_FLOAT_EQ(cfg.sigma, 5.25f);
    EXPECT_FLOAT_EQ(cfg.low_threshold, 1.25f);
    EXPECT_FLOAT_EQ(cfg.high_threshold, 3.75f);
    EXPECT_FLOAT_EQ(cfg.min_segment_length, 17.5f);
}

TEST(DynamicParameterResolverTest, Test_ModerateDiagonalScalesWithoutClamping) {
    const DynamicParameterResolver resolver;
    // ratio 1.44 -> sqrt 1.2, both inside clamp windows.
    const auto cfg = resolver.resolve(720.0f);
    EXPECT_FLOAT_EQ(cfg.sigma, 1.8f);
    EXPECT_FLOAT_EQ(cfg.low_threshold, 0.72f);
    EXPECT_FLOAT_EQ(cfg.high_threshold, 2.16f);
}

TEST(DynamicParameterResolverTest, Test_WidthHeightOverloadMatchesDiagonalOverload) {
    const DynamicParameterResolver resolver;
    // A 300x400 ROI has a diagonal of exactly 500 pixels.
    const auto from_wh = resolver.resolve(300, 400);
    const auto from_diag = resolver.resolve(500.0f);
    EXPECT_TRUE(sameConfig(from_wh, from_diag));
}

TEST(DynamicParameterResolverTest, Test_DegenerateDimensionsAreFlooredToOne) {
    const DynamicParameterResolver resolver;
    // Zero/negative dimensions are clamped to 1 before the diagonal is taken:
    // diagonal sqrt(2) is far below the reference, so every scale clamps low.
    const auto cfg = resolver.resolve(0, 0);
    EXPECT_FLOAT_EQ(cfg.sigma, 0.75f);
    EXPECT_FLOAT_EQ(cfg.low_threshold, 0.25f);
    EXPECT_FLOAT_EQ(cfg.high_threshold, 0.75f);
}
