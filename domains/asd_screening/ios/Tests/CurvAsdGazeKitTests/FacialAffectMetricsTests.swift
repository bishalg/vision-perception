import XCTest
@testable import CurvAsdGazeKit

final class FacialAffectMetricsTests: XCTestCase {

    // MARK: - Neutral Baseline

    func testNeutralBaselineAllZero() {
        let neutral = FacialAffectMetrics.neutral
        XCTAssertEqual(neutral.socialSmileIntensity, 0.0, accuracy: 0.001)
        XCTAssertEqual(neutral.surpriseScore, 0.0, accuracy: 0.001)
        XCTAssertEqual(neutral.distressScore, 0.0, accuracy: 0.001)
        XCTAssertEqual(neutral.expressivityIndex, 0.0, accuracy: 0.001)
    }

    // MARK: - Joy / Duchenne Social Smile

    func testDuchenneSocialSmileDetection() {
        // Genuine (Duchenne) smile: both mouth and cheek muscles fire
        let smilingSample = FacialAffectMetrics(
            mouthSmileLeft: 0.80, mouthSmileRight: 0.78,
            cheekSquintLeft: 0.55, cheekSquintRight: 0.52
        )
        XCTAssertGreaterThan(smilingSample.socialSmileIntensity, 0.40,
            "Duchenne smile should exceed 0.40 threshold")

        // Non-Duchenne (polite / forced) smile: only mouth, no cheek squint
        let politeSample = FacialAffectMetrics(
            mouthSmileLeft: 0.60, mouthSmileRight: 0.58
        )
        XCTAssertLessThan(politeSample.socialSmileIntensity, 0.40,
            "Non-Duchenne smile should fall below 0.40 threshold")
    }

    // MARK: - Surprise / Attention Engagement

    func testSurpriseEngagementDetection() {
        let surprisedSample = FacialAffectMetrics(
            browInnerUp: 0.70,
            browOuterUpLeft: 0.60, browOuterUpRight: 0.62,
            eyeWideLeft: 0.50, eyeWideRight: 0.48,
            jawOpen: 0.35
        )
        XCTAssertGreaterThan(surprisedSample.surpriseScore, 0.30,
            "High surprise should exceed 0.30 threshold")
        XCTAssertEqual(surprisedSample.distressScore, 0.0, accuracy: 0.001,
            "Surprised face should show no distress")
    }

    // MARK: - Distress / Negative Affect

    func testDistressDetection() {
        let distressedSample = FacialAffectMetrics(
            browDownLeft: 0.50, browDownRight: 0.48,
            mouthFrownLeft: 0.40, mouthFrownRight: 0.38,
            mouthRollLower: 0.30
        )
        XCTAssertGreaterThan(distressedSample.distressScore, 0.25,
            "Distress should exceed 0.25 threshold")
        XCTAssertEqual(distressedSample.socialSmileIntensity, 0.0, accuracy: 0.001,
            "Distressed face should show no smile")
    }

    // MARK: - Flat Affect Detection

    func testFlatAffectLowExpressivity() {
        // All muscles barely moving — flat affect
        let flatSample = FacialAffectMetrics(
            mouthSmileLeft: 0.02, mouthSmileRight: 0.01,
            browInnerUp: 0.01
        )
        XCTAssertLessThan(flatSample.expressivityIndex, 0.005,
            "Flat affect should produce very low expressivity variance")
    }

    func testHighExpressivityInAnimatedFace() {
        // Alternating muscles: high variance
        // Args must match FacialAffectMetrics init order: smile → eyeWide → browDown
        let animatedSample = FacialAffectMetrics(
            mouthSmileLeft: 0.90, mouthSmileRight: 0.88,
            eyeWideLeft: 0.75, eyeWideRight: 0.73,
            browDownLeft: 0.70, browDownRight: 0.68
        )
        XCTAssertGreaterThan(animatedSample.expressivityIndex, 0.01,
            "Animated face should produce high expressivity variance")
    }

    // MARK: - GazeFrame Integration

    func testGazeFrameComposeWithAffect() {
        let sample = GazeTelemetrySample(
            timestampMs: 1000.0, xNorm: 0.5, yNorm: 0.5, confidence: 0.95
        )
        let affect = FacialAffectMetrics(
            mouthSmileLeft: 0.72, mouthSmileRight: 0.70,
            cheekSquintLeft: 0.48, cheekSquintRight: 0.45
        )
        let frame = GazeFrame(
            gaze: sample,
            affect: affect,
            activeAOINames: ["Social_Actor"],
            aoiDwellMs: 320.0,
            stimulusEventTag: "character_laugh_onset",
            qualityVerdict: .optimal
        )

        XCTAssertEqual(frame.activeAOINames, ["Social_Actor"])
        XCTAssertEqual(frame.stimulusEventTag, "character_laugh_onset")
        XCTAssertGreaterThan(frame.affect.socialSmileIntensity, 0.40)
        XCTAssertEqual(frame.qualityVerdict, .optimal)
    }

    // MARK: - SessionHeader

    func testSessionHeaderSchemaVersion() {
        let header = SessionHeader(
            sessionId: "ABCD1234",
            participantToken: "child-token-uuid-v4",
            stimulusBattery: "GeoPref_v1",
            stimulusDurationSec: 30.0,
            captureSource: .trueDepth
        )
        XCTAssertEqual(header.schemaVersion, "1.1.0")
        XCTAssertEqual(header.captureSource, .trueDepth)
        XCTAssertFalse(header.sessionStartUtc.isEmpty)
        XCTAssertEqual(header.stimulusBattery, "GeoPref_v1")
    }
}
