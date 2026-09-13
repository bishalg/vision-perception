#include "curv/domains/AsdBiomarkerPack.hpp"
#include "curv/temporal/GazeTypes.hpp"
#include <gtest/gtest.h>
#include <cmath>

using namespace CurvEngine::temporal;
using namespace CurvEngine::domains;

TEST(AsdBiomarkersTest, GeoPrefSocialVsGeometricPreference) {
    std::vector<AreaOfInterest> aois = {
        {"Social_Actor", 0.0, 0.0, 500.0, 800.0},
        {"Geometric_Pattern", 500.0, 0.0, 1000.0, 800.0}
    };

    // 1. Social focus (Child dwells on Left Social Side)
    std::vector<GazeSample> social_samples;
    for (int t = 0; t <= 1000; t += 20) {
        social_samples.push_back({250.0 + (t % 3), 400.0 + (t % 3), static_cast<double>(t), 1.0});
    }
    auto social_report = AsdBiomarkerPack::evaluateSession(social_samples, aois);
    EXPECT_TRUE(social_report.is_valid);
    EXPECT_EQ(social_report.quality_verdict, "optimal");
    EXPECT_NEAR(social_report.biomarkers.social_dwell_ratio, 1.0, 1e-3);
    EXPECT_NEAR(social_report.biomarkers.object_dwell_ratio, 0.0, 1e-3);
    EXPECT_NEAR(social_report.biomarkers.social_preference_score, 1.0, 1e-3);

    // 2. Geometric focus (Child dwells on Right Geometric Side)
    std::vector<GazeSample> geo_samples;
    for (int t = 0; t <= 1000; t += 20) {
        geo_samples.push_back({750.0 + (t % 3), 400.0 + (t % 3), static_cast<double>(t), 1.0});
    }
    auto geo_report = AsdBiomarkerPack::evaluateSession(geo_samples, aois);
    EXPECT_TRUE(geo_report.is_valid);
    EXPECT_NEAR(geo_report.biomarkers.object_dwell_ratio, 1.0, 1e-3);
    EXPECT_NEAR(geo_report.biomarkers.social_preference_score, -1.0, 1e-3);
}

TEST(AsdBiomarkersTest, FacialCoreFeatureHierarchy) {
    std::vector<AreaOfInterest> aois = {
        {"Eye_Region", 200.0, 200.0, 400.0, 280.0},
        {"Mouth_Region", 250.0, 350.0, 350.0, 420.0}
    };

    std::vector<GazeSample> face_samples;
    // Phase 1: Look at eyes for 600 ms (t=0..600)
    for (int t = 0; t <= 600; t += 20) {
        face_samples.push_back({300.0 + (t % 2), 240.0 + (t % 2), static_cast<double>(t), 1.0});
    }
    // Phase 2: Saccade to mouth over 30 ms
    face_samples.push_back({300.0, 290.0, 615.0, 1.0});
    // Phase 3: Look at mouth for 400 ms (t=630..1030)
    for (int t = 630; t <= 1030; t += 20) {
        face_samples.push_back({300.0 + (t % 2), 380.0 + (t % 2), static_cast<double>(t), 1.0});
    }

    auto report = AsdBiomarkerPack::evaluateSession(face_samples, aois);
    EXPECT_TRUE(report.is_valid);
    EXPECT_GT(report.biomarkers.eye_dwell_ratio, 0.45);
    EXPECT_GT(report.biomarkers.mouth_dwell_ratio, 0.25);
    EXPECT_NEAR(report.biomarkers.first_eye_latency_ms, 0.0, 1e-2);
}

TEST(AsdBiomarkersTest, TransitionEntropyAndRepetitionMetrics) {
    // Deterministic alternating sequence: A -> B -> A -> B -> A -> B
    std::vector<std::string> deterministic = {"A", "B", "A", "B", "A", "B"};
    double ent_det = AsdBiomarkerPack::computeTransitionEntropy(deterministic);
    EXPECT_NEAR(ent_det, 0.0, 1e-6); // Probability P(B|A)=1, P(A|B)=1 -> 0 entropy

    // Repeat bigram ratio on loop A-B-A-B-A-B
    double repeat_ratio = AsdBiomarkerPack::computeRepeatBigramRatio(deterministic);
    EXPECT_GT(repeat_ratio, 0.5);

    // Stochastic sequence: A -> B, A -> C, A -> D
    std::vector<std::string> stochastic = {"A", "B", "A", "C", "A", "D"};
    double ent_stoch = AsdBiomarkerPack::computeTransitionEntropy(stochastic);
    EXPECT_GT(ent_stoch, 0.5);
}

TEST(AsdBiomarkersTest, ResearchKitAndCareKitExportCompliance) {
    std::vector<AreaOfInterest> aois = {{"Social", 0.0, 0.0, 500.0, 500.0}};
    std::vector<GazeSample> samples;
    for (int t = 0; t <= 500; t += 20) {
        samples.push_back({200.0, 200.0, static_cast<double>(t), 1.0});
    }

    auto report = AsdBiomarkerPack::evaluateSession(samples, aois);

    // 1. ResearchKit Active Step JSON
    auto ork_json = report.toResearchKitJson();
    EXPECT_EQ(ork_json["identifier"], "ORKActiveStepResult");
    EXPECT_EQ(ork_json["type"], "social_attention_gaze_result");
    EXPECT_TRUE(ork_json["biomarkers"].contains("social_preference_score"));
    EXPECT_TRUE(ork_json["biomarkers"].contains("eye_dwell_ratio"));

    // 2. CareKit Activity Tracking JSON
    auto ock_json = report.toCareKitJson();
    EXPECT_EQ(ock_json["care_plan_element"], "OCKCarePlanActivity");
    EXPECT_EQ(ock_json["completion_status"], "completed");
    EXPECT_TRUE(ock_json["summary_metrics"].contains("social_preference_index"));
}

TEST(AsdBiomarkersTest, QualityGatingRejection) {
    std::vector<AreaOfInterest> aois = {{"Scene", 0.0, 0.0, 500.0, 500.0}};
    std::vector<GazeSample> noisy_samples;
    // 80% invalid/low confidence samples
    for (int t = 0; t <= 1000; t += 20) {
        double conf = (t % 100 == 0) ? 0.9 : 0.2; // Only 20% valid
        noisy_samples.push_back({200.0, 200.0, static_cast<double>(t), conf});
    }

    AsdSessionConfig cfg;
    cfg.min_valid_ratio = 0.60;

    auto report = AsdBiomarkerPack::evaluateSession(noisy_samples, aois, cfg);
    EXPECT_FALSE(report.is_valid);
    EXPECT_EQ(report.quality_verdict, "low_quality_reject");
}
