#include "curv/domain/MountReliefAnalyzer.hpp"

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
}
