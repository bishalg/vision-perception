#include "curv/domain/MountReliefAnalyzer.hpp"

#include <string>
#include <gtest/gtest.h>

TEST(MountReliefAnalyzer, EmptySessionReturnsZeroViewMounts) {
    const nlohmann::json session = {{"schema", "curv_orbit_session_v1"}, {"frames", nlohmann::json::array()}};
    const auto map = CurvEngine::domain::MountReliefAnalyzer::analyzeOrbitSession(session);
    EXPECT_EQ(map.mounts.size(), 7u);
    for (const auto& m : map.mounts) {
        EXPECT_EQ(m.view_count, 0);
    }
    EXPECT_FALSE(map.venus_elevated);
}

TEST(MountReliefAnalyzer, FullOrbitProducesVenusEntry) {
    nlohmann::json frames = nlohmann::json::array();
    for (const char* bin : {"flat_center", "tilt_left", "thumb_toward", "tilt_right", "thumb_away", "tilt_forward"}) {
        frames.push_back({{"bin", bin}, {"gravity", {0.1f, 0.2f, -0.9f}}, {"device_roll_deg", 15.0f}});
    }
    const nlohmann::json session = {{"frames", frames}};
    const auto map = CurvEngine::domain::MountReliefAnalyzer::analyzeOrbitSession(session);
    ASSERT_FALSE(map.mounts.empty());
    bool has_venus = false;
    for (const auto& m : map.mounts) {
        if (m.mount == "venus") {
            has_venus = true;
        }
    }
    EXPECT_TRUE(has_venus);
    // Legacy session without landmarks/reference: tilt-proxy fallback, no mm.
    EXPECT_EQ(map.source, "orbit_tilt_proxy_v1");
    const auto j = map.toJson();
    for (const auto& m : j["mounts"]) {
        EXPECT_FALSE(m.contains("height_mm"));
    }
}

namespace {

// Same 21-point scaffold as the surface-relief test (normalized coords).
nlohmann::json scaffoldLandmarksJson() {
    const float pts[21][2] = {{0.50f, 0.80f}, {0.35f, 0.65f}, {0.30f, 0.55f}, {0.28f, 0.45f}, {0.27f, 0.35f},
                              {0.40f, 0.50f}, {0.38f, 0.40f}, {0.37f, 0.30f}, {0.36f, 0.20f}, {0.50f, 0.48f},
                              {0.50f, 0.38f}, {0.50f, 0.28f}, {0.50f, 0.18f}, {0.60f, 0.50f}, {0.62f, 0.40f},
                              {0.63f, 0.30f}, {0.64f, 0.20f}, {0.70f, 0.52f}, {0.72f, 0.43f}, {0.73f, 0.34f},
                              {0.74f, 0.25f}};
    nlohmann::json arr = nlohmann::json::array();
    for (const auto& p : pts) {
        arr.push_back({p[0], p[1]});
    }
    return arr;
}

const nlohmann::json* findMount(const nlohmann::json& mounts, const std::string& name) {
    for (const auto& m : mounts) {
        if (m.value("mount", "") == name) {
            return &m;
        }
    }
    return nullptr;
}

} // anonymous namespace

TEST(MountReliefAnalyzer, MetricSessionReportsMillimeters) {
    auto ref = scaffoldLandmarksJson();
    auto tilted = ref;
    // Thenar anchors displaced outward (-x, left-of-center points).
    for (size_t idx : {1u, 2u, 5u}) {
        tilted[idx][0] = tilted[idx][0].get<float>() - 0.012f;
    }
    nlohmann::json frames = nlohmann::json::array();
    frames.push_back({{"bin", "flat_center"},
                      {"gravity", {0.0f, 0.0f, -1.0f}},
                      {"device_roll_deg", 0.0f},
                      {"landmarks", ref},
                      {"image_width", 1000},
                      {"image_height", 1000}});
    frames.push_back({{"bin", "tilt_left"},
                      {"gravity", {0.3f, 0.1f, -0.94f}},
                      {"device_roll_deg", 18.0f},
                      {"landmarks", tilted},
                      {"image_width", 1000},
                      {"image_height", 1000}});
    const nlohmann::json session = {
        {"schema", "curv_orbit_session_v1"}, {"reference_landmarks", ref}, {"reference_gravity", {0.0f, 0.0f, -1.0f}},
        {"frames", frames}};
    const auto map = CurvEngine::domain::MountReliefAnalyzer::analyzeOrbitSession(session);
    EXPECT_EQ(map.source, "orbit_metric_v1");
    const auto j = map.toJson();
    const auto* venus = findMount(j["mounts"], "venus");
    const auto* luna = findMount(j["mounts"], "luna");
    ASSERT_NE(venus, nullptr);
    ASSERT_NE(luna, nullptr);
    ASSERT_TRUE(venus->contains("height_mm"));
    EXPECT_GT(venus->value("height_mm", 0.0f), 2.0f);
    EXPECT_GT(venus->value("height_mm", 0.0f), luna->value("height_mm", -10.0f) + 1.0f);
    EXPECT_EQ(venus->value("prominence", ""), "elevated");
}

TEST(MountReliefAnalyzer, MeasuredDepthSessionUsesDepthSource) {
    nlohmann::json depth = nlohmann::json::object();
    for (const char* m : {"jupiter", "saturn", "apollo", "mercury", "venus", "luna", "mars"}) {
        depth[m] = (std::string(m) == "venus") ? 3.0f : 1.0f;
    }
    nlohmann::json frames = nlohmann::json::array();
    frames.push_back(
        {{"bin", "flat_center"}, {"gravity", {0.0f, 0.0f, -1.0f}}, {"device_roll_deg", 0.0f}, {"depth_mm", depth}});
    const nlohmann::json session = {{"frames", frames}};
    const auto map = CurvEngine::domain::MountReliefAnalyzer::analyzeOrbitSession(session);
    EXPECT_EQ(map.source, "orbit_depth_v1");
    const auto j = map.toJson();
    const auto* venus = findMount(j["mounts"], "venus");
    ASSERT_NE(venus, nullptr);
    // Plane = median depth 1.0 -> venus reports +2.0 mm.
    EXPECT_FLOAT_EQ(venus->value("height_mm", 0.0f), 2.0f);
}
