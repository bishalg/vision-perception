import XCTest
@testable import CurvAsdGazeKit

final class BiomarkerClassificationEngineTests: XCTestCase {
    func testTypicalProfileProducesTypicalStratification() {
        let engine = BiomarkerClassificationEngine.shared

        // Typical toddler: 78% social preference, strong eye dwell (34%), low flat affect, high smile reactivity
        let typicalReport = GazeSessionReportSummary(
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
        let atypicalReport = GazeSessionReportSummary(
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
        let borderlineReport = GazeSessionReportSummary(
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
        let report = GazeSessionReportSummary(
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

    func testVaultSchemasIncludeClassificationBlock() {
        let report = GazeSessionReportSummary(
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
