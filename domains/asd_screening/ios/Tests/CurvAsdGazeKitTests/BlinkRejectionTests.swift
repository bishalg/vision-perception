import XCTest
import CoreGraphics
import simd
@testable import CurvAsdGazeKit

final class BlinkRejectionTests: XCTestCase {
    func testQualityGateRejectsBlinkUnderThreshold() {
        let gate = GazeQualityGate(blinkThreshold: 0.45)

        let blinkSample = GazeTelemetrySample(
            timestampMs: 100.0,
            xNorm: 0.50,
            yNorm: 0.50,
            confidence: 0.95,
            leftEyeOpenness: 0.30,
            rightEyeOpenness: 0.32,
            distanceToScreenCm: 48.0
        )

        let eval = gate.evaluateSample(blinkSample)
        XCTAssertFalse(eval.isValid, "Blink sample must be marked invalid")
        XCTAssertTrue(eval.isBlink, "Evaluation must flag isBlink == true")
        XCTAssertEqual(eval.verdict, .rejected, "Blink frames must be assigned .rejected verdict")
        XCTAssertTrue(eval.failureReasons.contains { $0.contains("Blink or ocular occlusion") })
    }

    func testQualityGateRejectsAsymmetricEyeOcclusion() {
        let gate = GazeQualityGate(blinkThreshold: 0.45)

        // Asymmetric occlusion: one eye open (0.85), other eye occluded (0.25)
        let asymmetricSample = GazeTelemetrySample(
            timestampMs: 200.0,
            xNorm: 0.50,
            yNorm: 0.50,
            confidence: 0.95,
            leftEyeOpenness: 0.85,
            rightEyeOpenness: 0.25,
            distanceToScreenCm: 48.0
        )

        let eval = gate.evaluateSample(asymmetricSample)
        XCTAssertFalse(eval.isValid)
        XCTAssertTrue(eval.isBlink, "Either eye occluded must trigger isBlink == true")
        XCTAssertEqual(eval.verdict, .rejected)
    }

    func testQualityGateAcceptsOpenEyes() {
        let gate = GazeQualityGate(blinkThreshold: 0.45)

        let openSample = GazeTelemetrySample(
            timestampMs: 300.0,
            xNorm: 0.50,
            yNorm: 0.50,
            confidence: 0.95,
            leftEyeOpenness: 0.88,
            rightEyeOpenness: 0.92,
            distanceToScreenCm: 48.0
        )

        let eval = gate.evaluateSample(openSample)
        XCTAssertTrue(eval.isValid)
        XCTAssertFalse(eval.isBlink)
        XCTAssertEqual(eval.verdict, .optimal)
    }

    func testActorFreezesGazeDuringBlinkAndSuppressesAOI() async {
        let actor = GazeProcessingActor()
        await actor.startSession()

        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.0, -0.48, 1.0)

        // 1. Initial valid fixation sample at screen center
        let initialSnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, 0.0, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: [
                "eyeBlinkLeft": 0.05,
                "eyeBlinkRight": 0.05
            ]
        )
        let openState = await actor.processFaceAnchor(snapshot: initialSnapshot, elapsedSec: 0.1)
        XCTAssertTrue(openState.currentQuality.isValid)
        XCTAssertFalse(openState.currentQuality.isBlink)
        XCTAssertEqual(openState.trackingStage, .gazeActive)
        let steadyGaze = openState.currentGazePoint

        // 2. Downward blink artifact snapshot: blendshapes indicate blink (0.95), lookAt shoots down
        let blinkSnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, -0.25, 0.48), // Downward plunge vector
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: [
                "eyeBlinkLeft": 0.95,
                "eyeBlinkRight": 0.95
            ]
        )
        let blinkState = await actor.processFaceAnchor(snapshot: blinkSnapshot, elapsedSec: 0.116)

        // Gaze point must remain frozen at the steady fixation point rather than plunging downwards!
        XCTAssertEqual(blinkState.currentGazePoint.x, steadyGaze.x, accuracy: 0.02, "Calibrated gaze must freeze at pre-blink coordinate")
        XCTAssertEqual(blinkState.currentGazePoint.y, steadyGaze.y, accuracy: 0.02, "Calibrated gaze must not plunge down to bottom edge during blink")
        XCTAssertFalse(blinkState.currentQuality.isValid)
        XCTAssertTrue(blinkState.currentQuality.isBlink)
        XCTAssertTrue(blinkState.activeAOIs.isEmpty, "AOI hits must be suppressed during blink artifact")
    }

    func testPostBlinkDebounceWindowSuppressesJitter() async {
        let actor = GazeProcessingActor()
        await actor.startSession()

        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.0, -0.48, 1.0)

        // Establish steady fixation
        let steadySnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, 0.0, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: ["eyeBlinkLeft": 0.02, "eyeBlinkRight": 0.02]
        )
        _ = await actor.processFaceAnchor(snapshot: steadySnapshot, elapsedSec: 0.1)

        // Trigger blink
        let blinkSnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, -0.20, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: ["eyeBlinkLeft": 0.95, "eyeBlinkRight": 0.95]
        )
        _ = await actor.processFaceAnchor(snapshot: blinkSnapshot, elapsedSec: 0.133)

        // Reopening snapshot (eyes open, but within 3-frame debounce recovery window)
        let reopenedSnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, 0.0, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: ["eyeBlinkLeft": 0.05, "eyeBlinkRight": 0.05]
        )

        // Frame 1 of recovery: debouncing
        let rec1 = await actor.processFaceAnchor(snapshot: reopenedSnapshot, elapsedSec: 0.150)
        XCTAssertFalse(rec1.currentQuality.isValid, "Recovery frame 1 must remain invalid while settling")

        // Frame 2 of recovery: debouncing
        let rec2 = await actor.processFaceAnchor(snapshot: reopenedSnapshot, elapsedSec: 0.166)
        XCTAssertFalse(rec2.currentQuality.isValid, "Recovery frame 2 must remain invalid while settling")

        // Frame 3 of recovery: debouncing
        let rec3 = await actor.processFaceAnchor(snapshot: reopenedSnapshot, elapsedSec: 0.183)
        XCTAssertFalse(rec3.currentQuality.isValid, "Recovery frame 3 must remain invalid while settling")

        // Frame 4: fully recovered!
        let recovered = await actor.processFaceAnchor(snapshot: reopenedSnapshot, elapsedSec: 0.200)
        XCTAssertTrue(recovered.currentQuality.isValid, "Frame 4 after debounce window must resume valid active gaze")
        XCTAssertEqual(recovered.trackingStage, .gazeActive)
    }

    func testBlinkFramesExcludedFromSessionSummary() async {
        let actor = GazeProcessingActor()
        await actor.startSession()

        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.0, -0.48, 1.0)

        let openSnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, 0.0, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: ["eyeBlinkLeft": 0.02, "eyeBlinkRight": 0.02]
        )

        let blinkSnapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, -0.30, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: ["eyeBlinkLeft": 0.98, "eyeBlinkRight": 0.98]
        )

        // 10 valid frames
        for i in 0..<10 {
            _ = await actor.processFaceAnchor(snapshot: openSnapshot, elapsedSec: Double(i) * 0.016)
        }
        // 5 blink frames
        for i in 10..<15 {
            _ = await actor.processFaceAnchor(snapshot: blinkSnapshot, elapsedSec: Double(i) * 0.016)
        }

        let (summary, frames) = await actor.finalizeSession()
        XCTAssertEqual(frames.count, 15)
        XCTAssertEqual(summary.totalFrames, 15)
        // Only the 10 open-eye frames should be counted as validFrames
        XCTAssertEqual(summary.validFrames, 10, "Blink frames must be excluded from validFrames count")
        XCTAssertEqual(summary.validRatio, 10.0 / 15.0, accuracy: 1e-4)
    }
}
