import XCTest
import CoreGraphics
@testable import CurvAsdGazeKit

final class TrackingPipelineTests: XCTestCase {
    func testTrackingStageProperties() {
        let searching = TrackingStage.searchingFace
        XCTAssertFalse(searching.hasFace)
        XCTAssertFalse(searching.hasEyes)
        XCTAssertFalse(searching.isGazeActive)
        XCTAssertEqual(searching.label, "Searching for face…")

        let face = TrackingStage.faceAligned(distanceCm: 45.0)
        XCTAssertTrue(face.hasFace)
        XCTAssertFalse(face.hasEyes)
        XCTAssertFalse(face.isGazeActive)
        XCTAssertEqual(face.label, "Face aligned • 45 cm")

        let eyes = TrackingStage.eyesTracking(leftOpenness: 0.85, rightOpenness: 0.90)
        XCTAssertTrue(eyes.hasFace)
        XCTAssertTrue(eyes.hasEyes)
        XCTAssertFalse(eyes.isGazeActive)
        XCTAssertEqual(eyes.label, "Eyes detected")

        let gaze = TrackingStage.gazeActive
        XCTAssertTrue(gaze.hasFace)
        XCTAssertTrue(gaze.hasEyes)
        XCTAssertTrue(gaze.isGazeActive)
        XCTAssertEqual(gaze.label, "Gaze active ✓")
    }

    func testControllerInitialStateIsInvalidAndSearching() {
        let controller = GazeSessionController()
        XCTAssertFalse(controller.currentQuality.isValid, "Controller should start with invalid quality until real frames arrive")
        XCTAssertEqual(controller.trackingStage, .searchingFace, "Controller should start in .searchingFace stage")
    }

    func testCrossControllerCalibrationSynchronization() {
        let controllerA = GazeSessionController()
        let controllerB = GazeSessionController()

        // Create a distinct calibrated profile
        let customProfile = GazeCalibrationProfile(
            a: 1.15,
            b: 0.0,
            c: 0.04,
            d: 0.0,
            e: 0.92,
            f: -0.03,
            sampleCount: 5,
            createdAt: Date()
        )

        let expectation = expectation(description: "Controller B reloads profile on notification")

        // Observe notification
        let observer = NotificationCenter.default.addObserver(
            forName: GazeSessionController.didUpdateCalibrationNotification,
            object: nil,
            queue: .main
        ) { _ in
            expectation.fulfill()
        }

        controllerA.applyAndPersist(calibrationProfile: customProfile)

        wait(for: [expectation], timeout: 2.0)
        NotificationCenter.default.removeObserver(observer)

        XCTAssertTrue(controllerA.calibrationProfile.isCalibrated)
        XCTAssertEqual(controllerB.calibrationProfile.a, 1.15, accuracy: 1e-6)
        XCTAssertEqual(controllerB.calibrationProfile.e, 0.92, accuracy: 1e-6)

        // Reset
        controllerA.resetCalibration()
        XCTAssertFalse(controllerB.calibrationProfile.isCalibrated)
    }

    func testUnavailableStageProperties() {
        let reasons: [TrackingUnavailableReason] = [.faceTrackingUnsupported, .cameraPermissionDenied, .sessionFailed, .interrupted]
        var labels = Set<String>()
        for reason in reasons {
            let stage = TrackingStage.unavailable(reason: reason)
            XCTAssertFalse(stage.hasFace)
            XCTAssertFalse(stage.hasEyes)
            XCTAssertFalse(stage.isGazeActive)
            labels.insert(stage.label)
        }
        XCTAssertEqual(labels.count, reasons.count, "Each unavailable reason needs a distinct label")
        XCTAssertEqual(
            TrackingStage.unavailable(reason: .faceTrackingUnsupported).label,
            "This device cannot run gaze observations"
        )
    }

    func testTickWithoutSimulatedSessionLeavesStageUntouched() {
        let controller = GazeSessionController()
        controller.calibrationTarget = CGPoint(x: 0.5, y: 0.5)
        for i in 0..<5 {
            controller.tickSimulatedFrame(timeSec: Double(i) * 0.016)
        }
        XCTAssertEqual(controller.trackingStage, .searchingFace)
        XCTAssertFalse(controller.currentQuality.isValid)
    }

    #if ASD_SIMULATION_ENABLED
    func testSimulationModeProducesSmoothedGazeAndActiveStage() {
        let controller = GazeSessionController()
        controller.startSimulatedSession()
        controller.calibrationTarget = CGPoint(x: 0.5, y: 0.5)

        // Advance simulation by 5 frames
        for i in 0..<5 {
            controller.tickSimulatedFrame(timeSec: Double(i) * 0.016)
        }

        XCTAssertEqual(controller.trackingStage, .gazeActive)
        XCTAssertTrue(controller.currentQuality.isValid)
        XCTAssertEqual(controller.currentGazePoint.x, 0.5, accuracy: 0.05)
        XCTAssertEqual(controller.currentGazePoint.y, 0.5, accuracy: 0.05)
    }
    #endif
}
