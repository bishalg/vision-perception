import XCTest
import CoreGraphics
import simd
@testable import CurvAsdGazeKit

final class GazeProcessingActorTests: XCTestCase {
    func testActorInitializationAndCalibrationUpdate() async {
        let actor = GazeProcessingActor()
        let initialProfile = await actor.calibrationProfile
        XCTAssertEqual(initialProfile.a, 1.0)
        XCTAssertEqual(initialProfile.e, 1.0)

        let customProfile = GazeCalibrationProfile(
            a: 1.08,
            b: 0.0,
            c: 0.02,
            d: 0.0,
            e: 0.95,
            f: -0.01,
            sampleCount: 5,
            createdAt: Date()
        )
        await actor.updateCalibrationProfile(customProfile)
        let updated = await actor.calibrationProfile
        XCTAssertEqual(updated.a, 1.08, accuracy: 1e-6)
        XCTAssertEqual(updated.e, 0.95, accuracy: 1e-6)

        await actor.resetCalibration()
        let reset = await actor.calibrationProfile
        XCTAssertEqual(reset.a, 1.0)
    }

    func testActorVolatileBufferingCapacity7200Frames() async {
        let actor = GazeProcessingActor()
        await actor.startSession()

        let isRecording = await actor.isRecording
        XCTAssertTrue(isRecording)

        // Simulate 7,200 frames (2 minutes at 60 Hz)
        for i in 0..<7200 {
            let t = Double(i) / 60.0
            let state = await actor.processSimulatedFrame(timeSec: t, target: nil)
            XCTAssertGreaterThanOrEqual(state.currentGazePoint.x, 0.0)
            XCTAssertLessThanOrEqual(state.currentGazePoint.x, 1.0)
        }

        let frameCount = await actor.getRecordedFramesCount()
        XCTAssertEqual(frameCount, 7200, "Actor must preserve all 7,200 volatile telemetry frames in RAM")

        let (summary, frames) = await actor.finalizeSession()
        XCTAssertEqual(frames.count, 7200)
        XCTAssertEqual(summary.totalFrames, 7200)
        XCTAssertGreaterThan(summary.durationSec, 110.0)
        XCTAssertGreaterThan(summary.validRatio, 0.85)

        let recordingPostFinalize = await actor.isRecording
        XCTAssertFalse(recordingPostFinalize)
    }

    func testActorProcessesFaceAnchorSnapshot() async {
        let actor = GazeProcessingActor()
        await actor.startSession()

        // Create synthetic face pose at 48cm with neutral eye transforms
        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.02, -0.48, 1.0)

        let snapshot = FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0.0, -0.05, 0.48),
            leftEyeTransform: matrix_identity_float4x4,
            rightEyeTransform: matrix_identity_float4x4,
            blendShapes: [
                "eyeBlinkLeft": 0.05,
                "eyeBlinkRight": 0.05,
                "mouthSmileLeft": 0.70,
                "mouthSmileRight": 0.68
            ]
        )

        let uiState = await actor.processFaceAnchor(snapshot: snapshot, elapsedSec: 0.5)

        XCTAssertTrue(uiState.currentQuality.isValid)
        XCTAssertEqual(uiState.trackingStage, .gazeActive)
        XCTAssertEqual(uiState.distanceCm, 48.0, accuracy: 1.0)
        XCTAssertGreaterThan(uiState.currentAffect.mouthSmileLeft, 0.65)
        XCTAssertGreaterThanOrEqual(uiState.currentGazePoint.x, 0.0)
        XCTAssertLessThanOrEqual(uiState.currentGazePoint.x, 1.0)
    }

    func testUntrackedSnapshotTransitionsToSearchingFace() async {
        let actor = GazeProcessingActor()
        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.0, -0.48, 1.0)

        let lostSnapshot = FaceAnchorSnapshot(
            isTracked: false,
            transform: transform,
            lookAtPoint: simd_float3(0, 0, 0)
        )

        let uiState = await actor.processFaceAnchor(snapshot: lostSnapshot, elapsedSec: 1.0)
        XCTAssertEqual(uiState.trackingStage, .searchingFace)
        XCTAssertFalse(uiState.currentQuality.isValid)
    }
}
