import XCTest
@testable import CurvAsdGazeKit

#if ASD_RESEARCH_CLASSIFIER
final class BiomarkerClassificationEngineTests: XCTestCase {
    func testTypicalProfileProducesTypicalStratification() {
        let engine = BiomarkerClassificationEngine.shared

        // Typical toddler: 78% social preference, strong eye dwell (34%), low flat affect, high smile reactivity
        let typicalReport = GazeSessionReportSummary.fixture(
            sessionId: "TYP001",
            durationSec: 60.0,
            totalFrames: 3600,
            validFrames: 3450,
            validRatio: 0.958,
            socialPreferenceScore: 0.72,
            socialDwellSec: 42.0,
            geometricDwellSec: 14.0,
            eyeDwellRatio: 0.36,
            mouthDwellRatio: 0.12,
            firstEyeLatencyMs: 380.0,
            socialSmileIntensityMean: 0.60,
            smileReactivityIndex: 0.55,
            flatAffectScore: 0.08,
            surpriseScoreMean: 0.32,
            qualityVerdict: .optimal
        )

        let result = engine.classifySync(summary: typicalReport)

        XCTAssertLessThan(result.riskScore, 0.35, "Typical profile should yield low risk probability")
        XCTAssertEqual(result.riskStratification, .typical)
        XCTAssertGreaterThanOrEqual(result.confidence, 0.70)
        XCTAssertGreaterThan(result.inferenceLatencyMs, 0.0)
    }

    func testAtypicalProfileProducesElevatedRiskStratification() {
        let engine = BiomarkerClassificationEngine.shared

        // Atypical visual attention: negative social preference (geometric focus -0.65),
        // reduced eye fixation (0.06), prolonged saccade latency (680ms), elevated flat affect (0.58)
        let atypicalReport = GazeSessionReportSummary.fixture(
            sessionId: "ATYP001",
            durationSec: 60.0,
            totalFrames: 3600,
            validFrames: 3200,
            validRatio: 0.889,
            socialPreferenceScore: -0.65,
            socialDwellSec: 8.0,
            geometricDwellSec: 46.0,
            eyeDwellRatio: 0.06,
            mouthDwellRatio: 0.22,
            firstEyeLatencyMs: 680.0,
            socialSmileIntensityMean: 0.10,
            smileReactivityIndex: 0.05,
            flatAffectScore: 0.62,
            surpriseScoreMean: 0.08,
            qualityVerdict: .optimal
        )

        let result = engine.classifySync(summary: atypicalReport)

        XCTAssertGreaterThan(result.riskScore, 0.65, "Atypical profile should yield elevated risk probability")
        XCTAssertEqual(result.riskStratification, .elevatedRisk)
        XCTAssertFalse(result.primaryContributingFeature.isEmpty)
        XCTAssertNotNil(result.featureImportances["socialPreference"])
    }

    func testBorderlineProfileProducesMonitoringRecommendation() {
        let engine = BiomarkerClassificationEngine.shared

        // Borderline profile: 50% split (0.0 score), moderate eye dwell (0.19), 520ms latency
        let borderlineReport = GazeSessionReportSummary.fixture(
            sessionId: "BORD001",
            durationSec: 60.0,
            totalFrames: 3600,
            validFrames: 3100,
            validRatio: 0.861,
            socialPreferenceScore: 0.05,
            socialDwellSec: 28.0,
            geometricDwellSec: 26.0,
            eyeDwellRatio: 0.19,
            mouthDwellRatio: 0.16,
            firstEyeLatencyMs: 520.0,
            socialSmileIntensityMean: 0.30,
            smileReactivityIndex: 0.25,
            flatAffectScore: 0.28,
            surpriseScoreMean: 0.18,
            qualityVerdict: .optimal
        )

        let result = engine.classifySync(summary: borderlineReport)

        XCTAssertGreaterThanOrEqual(result.riskScore, 0.35)
        XCTAssertLessThan(result.riskScore, 0.65)
        XCTAssertEqual(result.riskStratification, .monitor)
    }

    func testAsyncClassificationInvocation() async {
        let engine = BiomarkerClassificationEngine()
        let report = GazeSessionReportSummary.fixture(
            sessionId: "ASYNC001",
            durationSec: 30.0,
            totalFrames: 1800,
            validFrames: 1700,
            validRatio: 0.944,
            socialPreferenceScore: 0.45,
            socialDwellSec: 18.0,
            geometricDwellSec: 12.0
        )

        let result = await engine.classify(summary: report)
        XCTAssertGreaterThan(result.riskScore, 0.0)
        XCTAssertLessThan(result.riskScore, 1.0)
    }

    // PLC-4
    func testNilLatencyRenormalises() throws {
        let engine = BiomarkerClassificationEngine()

        // Every present metric at its maximum-risk extreme; latency absent. With the
        // latency weight (0.14) dropped and the rest re-normalised, the contributions
        // must sum to 1.0 (un-normalised they would sum to 0.86).
        let report = GazeSessionReportSummary.fixture(
            socialPreferenceScore: -1.0,
            eyeDwellRatio: 0.0,
            mouthDwellRatio: nil,
            firstEyeLatencyMs: nil,
            smileReactivityIndex: 0.0,
            flatAffectScore: 1.0
        )
        let result = engine.classifySync(summary: report)

        XCTAssertNil(result.featureImportances["firstEyeLatency"], "An absent metric must not contribute")
        let total = result.featureImportances.values.reduce(0, +)
        XCTAssertEqual(total, 1.0, accuracy: 1e-9)
        XCTAssertEqual(result.featureImportances["socialPreference"] ?? 0, 0.42 / 0.86, accuracy: 1e-9)
        // logit = (1.0 - 0.38) * 6.5 → sigmoid ≈ 0.9825 (≈ 0.958 without re-normalisation).
        XCTAssertEqual(result.riskScore, 1.0 / (1.0 + exp(-(1.0 - 0.38) * 6.5)), accuracy: 1e-9)
        XCTAssertEqual(result.executionDevice, .fallbackHeuristic)
        XCTAssertFalse(result.primaryContributingFeature.contains("CoreML"))
        XCTAssertFalse(result.primaryContributingFeature.contains("Latency"))
    }

    func testNilEyeDwellAndLatencyRenormalises() {
        let engine = BiomarkerClassificationEngine()
        let report = GazeSessionReportSummary.fixture(
            socialPreferenceScore: 0.2,
            eyeDwellRatio: nil,
            firstEyeLatencyMs: nil,
            smileReactivityIndex: 0.25,
            flatAffectScore: 0.3
        )
        let result = engine.classifySync(summary: report)
        XCTAssertNil(result.featureImportances["eyeDwell"])
        XCTAssertNil(result.featureImportances["firstEyeLatency"])
        XCTAssertEqual(Set(result.featureImportances.keys), ["socialPreference", "flatAffect", "smileReactivity"])
        XCTAssertFalse(result.riskScore.isNaN)
    }

    func testHeuristicFeatureLabelsNeverClaimCoreML() {
        let engine = BiomarkerClassificationEngine()
        for social in stride(from: -1.0, through: 1.0, by: 0.5) {
            let report = GazeSessionReportSummary.fixture(
                socialPreferenceScore: social,
                eyeDwellRatio: 0.1,
                firstEyeLatencyMs: 700
            )
            let result = engine.classifySync(summary: report)
            XCTAssertEqual(result.executionDevice, .fallbackHeuristic)
            XCTAssertFalse(result.primaryContributingFeature.contains("CoreML"))
        }
    }

    func testVaultSchemasIncludeClassificationBlock() {
        let report = GazeSessionReportSummary.fixture(
            sessionId: "VAULT001",
            durationSec: 45.0,
            totalFrames: 2700,
            validFrames: 2550,
            validRatio: 0.944,
            socialPreferenceScore: 0.62,
            socialDwellSec: 32.0,
            geometricDwellSec: 13.0
        )

        let classification = BiomarkerClassificationResult(
            riskScore: 0.22,
            confidence: 0.95,
            riskStratification: .typical,
            primaryContributingFeature: "Typical Social Visual Attention",
            featureImportances: ["socialPreference": 0.10],
            executionDevice: .appleNeuralEngine,
            evaluationTimestamp: Date(),
            inferenceLatencyMs: 1.1
        )

        let rk = SessionVaultManager.makeResearchKitJson(report: report, classification: classification)
        XCTAssertNotNil(rk["classification"])
        let rkClass = rk["classification"] as? [String: Any]
        XCTAssertEqual(rkClass?["riskStratification"] as? String, RiskStratification.typical.rawValue)
        XCTAssertEqual(rkClass?["riskScore"] as? Double, 0.22)

        let ck = SessionVaultManager.makeCareKitJson(report: report, classification: classification)
        let ckOutcome = ck["outcome"] as? [String: Any]
        XCTAssertEqual(ckOutcome?["riskScore"] as? Double, 0.22)
        XCTAssertEqual(ckOutcome?["riskStratification"] as? String, RiskStratification.typical.rawValue)
    }
}
#endif
