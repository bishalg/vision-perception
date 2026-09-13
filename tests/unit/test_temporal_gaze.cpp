#include "curv/domains/DriverGazePack.hpp"
#include "curv/temporal/DynamicTimeWarping.hpp"
#include "curv/temporal/GazeTypes.hpp"
#include "curv/temporal/OculomotorClassifier.hpp"
#include <gtest/gtest.h>
#include <cmath>

using namespace CurvEngine::temporal;
using namespace CurvEngine::domains;

TEST(TemporalGazeTest, IvtFixationAndSaccadeParsing) {
    std::vector<GazeSample> samples;

    // Phase 1: Fixation 1 at (100, 100) for 200 ms (sample every 10 ms)
    for (int t = 0; t <= 200; t += 10) {
        GazeSample s;
        s.x = 100.0 + (t % 3 - 1.0) * 0.5; // slight micro-tremor
        s.y = 100.0 + ((t / 10) % 3 - 1.0) * 0.5;
        s.timestamp_ms = static_cast<double>(t);
        s.confidence = 0.98;
        samples.push_back(s);
    }

    // Phase 2: Saccade jump from (100, 100) to (500, 400) over 30 ms (high velocity)
    samples.push_back({230.0, 200.0, 210.0, 0.95});
    samples.push_back({380.0, 310.0, 220.0, 0.95});
    samples.push_back({500.0, 400.0, 230.0, 0.95});

    // Phase 3: Fixation 2 at (500, 400) for 250 ms
    for (int t = 240; t <= 500; t += 10) {
        GazeSample s;
        s.x = 500.0 + (t % 3 - 1.0) * 0.5;
        s.y = 400.0 + ((t / 10) % 3 - 1.0) * 0.5;
        s.timestamp_ms = static_cast<double>(t);
        s.confidence = 0.98;
        samples.push_back(s);
    }

    IvtConfig cfg;
    cfg.velocity_threshold = 200.0;
    cfg.min_fixation_duration_ms = 80.0;

    Scanpath scanpath = OculomotorClassifier::classifyIvt(samples, cfg);

    ASSERT_EQ(scanpath.fixations.size(), 2);
    ASSERT_EQ(scanpath.saccades.size(), 1);

    // Verify spatial centroids
    EXPECT_NEAR(scanpath.fixations[0].x, 100.0, 2.0);
    EXPECT_NEAR(scanpath.fixations[0].y, 100.0, 2.0);
    EXPECT_NEAR(scanpath.fixations[1].x, 500.0, 2.0);
    EXPECT_NEAR(scanpath.fixations[1].y, 400.0, 2.0);

    // Verify saccade connectivity
    EXPECT_NEAR(scanpath.saccades[0].start_x, 100.0, 2.0);
    EXPECT_NEAR(scanpath.saccades[0].end_x, 500.0, 2.0);
    EXPECT_GT(scanpath.saccades[0].amplitude, 450.0);
}

TEST(TemporalGazeTest, IdtFixationClustering) {
    std::vector<GazeSample> samples;

    // Cluster around (200, 200) for 150 ms
    for (int t = 0; t <= 150; t += 10) {
        samples.push_back({200.0 + (t % 4), 200.0 + (t % 3), static_cast<double>(t), 1.0});
    }
    // Shift far away
    samples.push_back({600.0, 600.0, 170.0, 1.0});
    // Cluster around (600, 600) for 150 ms
    for (int t = 180; t <= 330; t += 10) {
        samples.push_back({600.0 + (t % 4), 600.0 + (t % 3), static_cast<double>(t), 1.0});
    }

    IdtConfig cfg;
    cfg.dispersion_threshold = 20.0;
    cfg.window_duration_ms = 80.0;

    Scanpath scanpath = OculomotorClassifier::classifyIdt(samples, cfg);
    EXPECT_GE(scanpath.fixations.size(), 1);
}

TEST(TemporalGazeTest, AoiSequenceEncoding) {
    Scanpath scanpath;
    // Fixation 1 in Zone A
    Fixation f1;
    f1.x = 50.0; f1.y = 50.0; f1.duration_ms = 200.0;
    // Fixation 2 also in Zone A
    Fixation f2;
    f2.x = 60.0; f2.y = 55.0; f2.duration_ms = 200.0;
    // Fixation 3 in Zone B
    Fixation f3;
    f3.x = 300.0; f3.y = 300.0; f3.duration_ms = 250.0;

    scanpath.fixations = {f1, f2, f3};

    std::vector<AreaOfInterest> aois = {
        {"A_Nav", 0.0, 0.0, 150.0, 150.0},
        {"B_Feed", 200.0, 200.0, 400.0, 400.0}
    };

    std::string token_dedup = OculomotorClassifier::encodeAoiSequence(scanpath, aois, true);
    EXPECT_EQ(token_dedup, "AB");

    std::string token_full = OculomotorClassifier::encodeAoiSequence(scanpath, aois, false);
    EXPECT_EQ(token_full, "AAB");
}

TEST(TemporalGazeTest, DynamicTimeWarpingTrajectoryMatching) {
    // 1. Identical trajectories should yield 0 distance
    std::vector<cv::Point2d> line_a = {
        {0.0, 0.0}, {10.0, 0.0}, {20.0, 0.0}, {30.0, 0.0}
    };
    EXPECT_NEAR(DynamicTimeWarping::computeDistance(line_a, line_a), 0.0, 1e-6);

    // 2. Gesture classification
    std::vector<cv::Point2d> horizontal_swipe = {
        {10.0, 100.0}, {50.0, 100.0}, {100.0, 100.0}, {150.0, 100.0}
    };
    std::vector<cv::Point2d> vertical_swipe = {
        {100.0, 10.0}, {100.0, 50.0}, {100.0, 100.0}, {100.0, 150.0}
    };

    std::vector<GestureTemplate> templates = {
        {"swipe_horizontal", horizontal_swipe},
        {"swipe_vertical", vertical_swipe}
    };

    // Query with slightly noisy horizontal trajectory
    std::vector<cv::Point2d> query_horizontal = {
        {12.0, 102.0}, {48.0, 99.0}, {105.0, 101.0}, {145.0, 100.0}
    };

    auto match = DynamicTimeWarping::classifyGesture(query_horizontal, templates);
    EXPECT_EQ(match.matched_gesture, "swipe_horizontal");
    EXPECT_GT(match.normalized_score, 0.85);
}

TEST(TemporalGazeTest, DriverVisualAttentionSafetyEvaluation) {
    AreaOfInterest windshield{"Windshield", 200.0, 100.0, 800.0, 600.0};
    AreaOfInterest phone_mount{"PhoneMount", 50.0, 650.0, 180.0, 800.0};

    // Scenario 1: Alert driver scanning windshield
    std::vector<GazeSample> alert_samples;
    for (int t = 0; t <= 1000; t += 20) {
        alert_samples.push_back({450.0 + (t % 10), 350.0 + (t % 5), static_cast<double>(t), 1.0});
    }
    auto alert_report = DriverGazePack::evaluateVisualAttention(alert_samples, windshield, {phone_mount});
    EXPECT_EQ(alert_report.attention_state, "alert_scanning");
    EXPECT_EQ(alert_report.critical_off_road_glances, 0);

    // Scenario 2: Distracted driver looking down at phone for 2.2 seconds (NHTSA danger threshold)
    std::vector<GazeSample> distracted_samples;
    for (int t = 0; t <= 2200; t += 20) {
        distracted_samples.push_back({100.0 + (t % 4), 700.0 + (t % 4), static_cast<double>(t), 1.0});
    }
    auto distracted_report = DriverGazePack::evaluateVisualAttention(distracted_samples, windshield, {phone_mount});
    EXPECT_EQ(distracted_report.attention_state, "critical_distraction");
    EXPECT_GE(distracted_report.critical_off_road_glances, 1);
    EXPECT_NEAR(distracted_report.off_road_dwell_ratio, 1.0, 1e-3);
}
