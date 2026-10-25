import XCTest
@testable import CurvAsdGazeKit

final class BiomarkerDetailModelTests: XCTestCase {

    // MARK: - 1. Clinical Social Attention (GeoPref) Math & Boundary Verification

    func testSocialPreferenceFormulaZeroDwellSafety() {
        // Degenerate case: 0 social dwell and 0 geometric dwell
        let socialDwell = 0.0
        let geometricDwell = 0.0
        let totalDwell = socialDwell + geometricDwell

        let score = totalDwell > 0.0 ? (socialDwell - geometricDwell) / totalDwell : 0.0
        XCTAssertFalse(score.isNaN, "Score must not be NaN")
        XCTAssertFalse(score.isInfinite, "Score must not be Infinite")
        XCTAssertEqual(score, 0.0)

        // Target ratio in UI graph uses max(0.1, totalDwell)
        let targetRatio = socialDwell / max(0.1, socialDwell + geometricDwell)
        XCTAssertEqual(targetRatio, 0.0)
        XCTAssertFalse(targetRatio.isNaN)
    }

    func testSocialPreferenceFormulaExtremeBounds() {
        // Pure Social Attention (+1.0)
        let pureSocial = (30.0 - 0.0) / (30.0 + 0.0)
        XCTAssertEqual(pureSocial, 1.0, accuracy: 0.001)

        // Pure Geometric Attention (-1.0)
        let pureGeometric = (0.0 - 30.0) / (0.0 + 30.0)
        XCTAssertEqual(pureGeometric, -1.0, accuracy: 0.001)

        // Equal Distribution (0.0)
        let equalDwell = (15.0 - 15.0) / (15.0 + 15.0)
        XCTAssertEqual(equalDwell, 0.0, accuracy: 0.001)
    }

    // MARK: - 2. Biomarker Classification Engine Stress & Boundary Tests

    #if ASD_RESEARCH_CLASSIFIER

    func testClassificationEngineZeroValidFramesSafety() {
        let engine = BiomarkerClassificationEngine.shared

        let zeroReport = GazeSessionReportSummary.fixture(
            sessionId: "ZERO001",
            durationSec: 0.0,
            totalFrames: 0,
            validFrames: 0,
            validRatio: 0.0,
            socialPreferenceScore: 0.0,
            socialDwellSec: 0.0,
            geometricDwellSec: 0.0,
            eyeDwellRatio: 0.0,
            mouthDwellRatio: 0.0,
            firstEyeLatencyMs: 0.0,
            socialSmileIntensityMean: 0.0,
            smileReactivityIndex: 0.0,
            flatAffectScore: 1.0,
            surpriseScoreMean: 0.0,
            qualityVerdict: .rejected
        )

        let result = engine.classifySync(summary: zeroReport)
        XCTAssertFalse(result.riskScore.isNaN, "Risk score must never be NaN")
        XCTAssertFalse(result.confidence.isNaN, "Confidence must never be NaN")
        XCTAssertGreaterThanOrEqual(result.riskScore, 0.0)
        XCTAssertLessThanOrEqual(result.riskScore, 1.0)
        XCTAssertGreaterThanOrEqual(result.confidence, 0.70)
        XCTAssertFalse(result.primaryContributingFeature.isEmpty)
    }

    func testClassificationEngineLatencyMonotonicity() {
        let engine = BiomarkerClassificationEngine.shared

        // Base report with typical features
        func makeReport(latency: Double) -> GazeSessionReportSummary {
            GazeSessionReportSummary.fixture(
                sessionId: "LATTEST",
                durationSec: 30.0,
                totalFrames: 1800,
                validFrames: 1700,
                validRatio: 0.94,
                socialPreferenceScore: 0.50,
                socialDwellSec: 20.0,
                geometricDwellSec: 10.0,
                eyeDwellRatio: 0.30,
                firstEyeLatencyMs: latency,
                socialSmileIntensityMean: 0.40,
                smileReactivityIndex: 0.35,
                flatAffectScore: 0.10,
                qualityVerdict: .optimal
            )
        }

        let fastResult = engine.classifySync(summary: makeReport(latency: 320.0))
        let slowResult = engine.classifySync(summary: makeReport(latency: 850.0))

        XCTAssertLessThan(
            fastResult.riskScore,
            slowResult.riskScore,
            "Prolonged saccade latency must monotonically increase ASD risk probability"
        )
    }

    func testClassificationEngineEyeDwellMonotonicity() {
        let engine = BiomarkerClassificationEngine.shared

        func makeReport(eyeRatio: Double) -> GazeSessionReportSummary {
            GazeSessionReportSummary.fixture(
                sessionId: "EYETEST",
                durationSec: 30.0,
                totalFrames: 1800,
                validFrames: 1700,
                validRatio: 0.94,
                socialPreferenceScore: 0.40,
                socialDwellSec: 18.0,
                geometricDwellSec: 12.0,
                eyeDwellRatio: eyeRatio,
                firstEyeLatencyMs: 450.0,
                socialSmileIntensityMean: 0.35,
                smileReactivityIndex: 0.30,
                flatAffectScore: 0.15,
                qualityVerdict: .optimal
            )
        }

        let highEyeResult = engine.classifySync(summary: makeReport(eyeRatio: 0.45))
        let lowEyeResult = engine.classifySync(summary: makeReport(eyeRatio: 0.05))

        XCTAssertLessThan(
            highEyeResult.riskScore,
            lowEyeResult.riskScore,
            "High eye dwell must yield lower ASD risk score than attenuated eye dwell"
        )
    }

    #endif

    // MARK: - 3. Session Controller Biomarker Output Integrity

    @MainActor
    func testSessionControllerEmptySessionProducesSafeReport() async {
        let controller = GazeSessionController()
        // Stop without recording any frames
        let summary = await controller.stopSession()

        XCTAssertEqual(summary.totalFrames, 0)
        XCTAssertEqual(summary.validFrames, 0)
        XCTAssertEqual(summary.validRatio, 0.0)
        XCTAssertEqual(summary.socialPreferenceScore, 0.0)
        XCTAssertEqual(summary.socialDwellSec, 0.0)
        XCTAssertEqual(summary.geometricDwellSec, 0.0)
        XCTAssertEqual(summary.qualityVerdict, GazeQualityGate.Verdict.rejected)
        XCTAssertFalse(summary.sessionId.isEmpty)
        // Never started: nothing may be labelled as live TrueDepth data.
        XCTAssertNotEqual(summary.captureSource, .trueDepth)
    }

    #if ASD_SIMULATION_ENABLED
    @MainActor
    func testSessionControllerDwellTimeConservation() async {
        let controller = GazeSessionController()
        controller.startSimulatedSession()

        // Tick 60 frames = exactly 1.0 second
        for i in 0..<60 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }

        let summary = await controller.stopSession()
        XCTAssertEqual(summary.totalFrames, 60)
        // PLC-5: duration is last minus first sample time (59 intervals of 1/60 s),
        // not frame count times an assumed 60 fps interval.
        XCTAssertEqual(summary.durationSec, 59.0 / 60.0, accuracy: 0.001)
        XCTAssertGreaterThan(summary.validFrames, 0)
        XCTAssertGreaterThanOrEqual(summary.socialDwellSec, 0.0)
        XCTAssertGreaterThanOrEqual(summary.geometricDwellSec, 0.0)
        XCTAssertLessThanOrEqual(summary.socialDwellSec + summary.geometricDwellSec, 1.05)
        XCTAssertLessThanOrEqual(summary.socialDwellSec + summary.geometricDwellSec, summary.durationSec + 1e-9)
    }
    #endif

    // MARK: - 4. Quality Gate Blink Rejection Precision

    func testQualityGateBlinkThresholdStrictness() {
        let gate = GazeQualityGate(blinkThreshold: 0.45)

        // Open eye sample (0.80) -> Valid
        let openSample = GazeTelemetrySample(
            timestampMs: 100,
            xNorm: 0.5,
            yNorm: 0.5,
            confidence: 0.95,
            leftEyeOpenness: 0.80,
            rightEyeOpenness: 0.82,
            distanceToScreenCm: 45.0
        )
        let openEval = gate.evaluateSample(openSample)
        XCTAssertTrue(openEval.isValid)
        XCTAssertFalse(openEval.isBlink)
        XCTAssertEqual(openEval.verdict, GazeQualityGate.Verdict.optimal)

        // Left eye blink (0.30 < 0.45) -> Rejected Blink
        let blinkSample = GazeTelemetrySample(
            timestampMs: 116,
            xNorm: 0.5,
            yNorm: 0.5,
            confidence: 0.95,
            leftEyeOpenness: 0.30,
            rightEyeOpenness: 0.80,
            distanceToScreenCm: 45.0
        )
        let blinkEval = gate.evaluateSample(blinkSample)
        XCTAssertFalse(blinkEval.isValid)
        XCTAssertTrue(blinkEval.isBlink)
        XCTAssertEqual(blinkEval.verdict, GazeQualityGate.Verdict.rejected)
    }
}
