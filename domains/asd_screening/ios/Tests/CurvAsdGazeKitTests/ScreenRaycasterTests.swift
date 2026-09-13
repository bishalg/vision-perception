import XCTest
import simd
@testable import CurvAsdGazeKit

final class ScreenRaycasterTests: XCTestCase {
    func testScreenCenterRaycast() {
        let raycaster = ScreenRaycaster(
            screenWidthPoints: 820.0,
            screenHeightPoints: 1180.0,
            screenWidthMeters: 0.20,
            screenHeightMeters: 0.25
        )

        // Vector straight ahead at 50 cm
        let lookAt = simd_float3(0.0, 0.0, -0.5)
        let result = raycaster.projectGazeToScreen(lookAtPoint: lookAt, distanceMeters: 0.5)

        XCTAssertTrue(result.isWithinScreen)
        XCTAssertEqual(result.normalizedPoint.x, 0.5, accuracy: 0.001)
        XCTAssertEqual(result.normalizedPoint.y, 0.5, accuracy: 0.001)
        XCTAssertEqual(result.screenPoint.x, 410.0, accuracy: 0.1)
        XCTAssertEqual(result.screenPoint.y, 590.0, accuracy: 0.1)
        XCTAssertEqual(result.distanceCm, 50.0, accuracy: 0.1)
    }

    func testScreenOffscreenClamping() {
        let raycaster = ScreenRaycaster(
            screenWidthPoints: 800.0,
            screenHeightPoints: 1000.0,
            screenWidthMeters: 0.20,
            screenHeightMeters: 0.25
        )

        // Very far right: effectiveX = 0.50m on a 0.20m wide screen
        let lookAt = simd_float3(0.50, 0.0, -0.5)
        let result = raycaster.projectGazeToScreen(lookAtPoint: lookAt, distanceMeters: 0.5)

        XCTAssertFalse(result.isWithinScreen)
        XCTAssertEqual(result.normalizedPoint.x, 1.0, accuracy: 0.001)
        XCTAssertEqual(result.screenPoint.x, 800.0, accuracy: 0.1)
    }

    func testQualityGateValidation() {
        let gate = GazeQualityGate()

        // 1. Valid sample
        let validSample = GazeTelemetrySample(
            timestampMs: 1000.0,
            xNorm: 0.5,
            yNorm: 0.5,
            confidence: 0.95,
            leftEyeOpenness: 0.9,
            rightEyeOpenness: 0.9,
            headPitchRad: 0.05,
            headYawRad: -0.05,
            distanceToScreenCm: 45.0
        )
        let evalValid = gate.evaluateSample(validSample)
        XCTAssertTrue(evalValid.isValid)
        XCTAssertEqual(evalValid.verdict, .optimal)

        // 2. Too close sample
        let closeSample = GazeTelemetrySample(
            timestampMs: 1016.0,
            xNorm: 0.5,
            yNorm: 0.5,
            confidence: 0.95,
            distanceToScreenCm: 20.0
        )
        let evalClose = gate.evaluateSample(closeSample)
        XCTAssertFalse(evalClose.isValid)
        XCTAssertTrue(evalClose.failureReasons.contains { $0.contains("too close") })

        // 3. Excessive head turn
        let turnedSample = GazeTelemetrySample(
            timestampMs: 1033.0,
            xNorm: 0.5,
            yNorm: 0.5,
            confidence: 0.95,
            headYawRad: 0.85 // ~48 degrees
        )
        let evalTurned = gate.evaluateSample(turnedSample)
        XCTAssertFalse(evalTurned.isValid)
        XCTAssertTrue(evalTurned.failureReasons.contains { $0.contains("Excessive head yaw") })
    }

    func testStimulusAOITrackerHitTesting() {
        let tracker = StimulusAOITracker.makeGeoPrefBattery()

        // Gaze falling on left side (Social Actor)
        let socialHits = tracker.activeAOIs(atTimeSec: 5.0, xNorm: 0.25, yNorm: 0.50)
        XCTAssertEqual(socialHits, ["Social_Actor"])

        // Gaze falling on right side (Geometric Pattern)
        let geomHits = tracker.activeAOIs(atTimeSec: 12.0, xNorm: 0.75, yNorm: 0.50)
        XCTAssertEqual(geomHits, ["Geometric_Pattern"])

        // Beyond video duration
        let expiredHits = tracker.activeAOIs(atTimeSec: 45.0, xNorm: 0.25, yNorm: 0.50)
        XCTAssertTrue(expiredHits.isEmpty)
    }

    func testScreenRaycasterAutoDeviceDefaults() {
        let raycaster = ScreenRaycaster()
        XCTAssertGreaterThan(raycaster.screenWidthPoints, 300.0)
        XCTAssertGreaterThan(raycaster.screenHeightPoints, 600.0)
        XCTAssertGreaterThan(raycaster.screenWidthMeters, 0.05)
        XCTAssertGreaterThan(raycaster.screenHeightMeters, 0.10)
    }

    func testCalibrationTargetSimulation() {
        let controller = GazeSessionController()
        let target = CGPoint(x: 0.22, y: 0.24)
        controller.calibrationTarget = target
        controller.startSession()

        controller.tickSimulatedFrame(timeSec: 1.0)
        let gaze = controller.currentGazePoint

        // Gaze should be within 0.05 of the calibration target
        let distance = hypot(gaze.x - target.x, gaze.y - target.y)
        XCTAssertLessThan(distance, 0.05)
    }

    func test3DCameraSpaceRaycastCenter() {
        let raycaster = ScreenRaycaster(
            screenWidthPoints: 393.0,
            screenHeightPoints: 852.0,
            screenWidthMeters: 0.0715,
            screenHeightMeters: 0.154
        )

        var faceTransform = matrix_identity_float4x4
        faceTransform.columns.0 = simd_float4(-1, 0, 0, 0)
        faceTransform.columns.1 = simd_float4(0, 1, 0, 0)
        faceTransform.columns.2 = simd_float4(0, 0, -1, 0)
        faceTransform.columns.3 = simd_float4(0.0, -0.099, -0.45, 1.0)

        // Center of screen: lookAt in face space corresponds to screen center (0.50, 0.50)
        let lookAt = simd_float3(0.0, 0.030, -0.45)
        let result = raycaster.projectGazeToScreen(
            faceTransform: faceTransform,
            lookAtPoint: lookAt
        )

        XCTAssertTrue(result.isWithinScreen)
        XCTAssertEqual(result.normalizedPoint.x, 0.5, accuracy: 0.02)
        XCTAssertEqual(result.normalizedPoint.y, 0.5, accuracy: 0.02)
    }

    func test3DCameraSpaceRaycastTopLeft() {
        let raycaster = ScreenRaycaster(
            screenWidthPoints: 393.0,
            screenHeightPoints: 852.0,
            screenWidthMeters: 0.0715,
            screenHeightMeters: 0.154
        )

        var faceTransform = matrix_identity_float4x4
        faceTransform.columns.0 = simd_float4(-1, 0, 0, 0)
        faceTransform.columns.1 = simd_float4(0, 1, 0, 0)
        faceTransform.columns.2 = simd_float4(0, 0, -1, 0)
        faceTransform.columns.3 = simd_float4(0.0, -0.099, -0.45, 1.0)

        // Target 2 Top-Left (0.22, 0.24): looking up-left corresponds to face local X > 0, Y > 0
        let lookAt = simd_float3(0.020, 0.070, -0.45)
        let result = raycaster.projectGazeToScreen(
            faceTransform: faceTransform,
            lookAtPoint: lookAt
        )

        XCTAssertTrue(result.isWithinScreen)
        XCTAssertEqual(result.normalizedPoint.x, 0.22, accuracy: 0.02)
        XCTAssertEqual(result.normalizedPoint.y, 0.24, accuracy: 0.02)
    }

    func testReverseRaycastIsRejectedAsOutOfBounds() {
        let raycaster = ScreenRaycaster(
            screenWidthPoints: 393.0,
            screenHeightPoints: 852.0,
            screenWidthMeters: 0.0715,
            screenHeightMeters: 0.154
        )

        var faceTransform = matrix_identity_float4x4
        faceTransform.columns.0 = simd_float4(-1, 0, 0, 0)
        faceTransform.columns.1 = simd_float4(0, 1, 0, 0)
        faceTransform.columns.2 = simd_float4(0, 0, -1, 0)
        // Face is in front of screen (Z = -0.45m)
        faceTransform.columns.3 = simd_float4(0.0, -0.099, -0.45, 1.0)

        // lookAtPoint pointing backwards (behind the head, towards -Z in camera space, away from Z=0 screen)
        // targetCam.z < eyeOriginCam.z, so ray direction points away from screen (t < 0)
        let lookAtBackwards = simd_float3(0.0, 0.0, 1.0) // In face local space, +Z is behind when columns.2.z = -1
        let result = raycaster.projectGazeToScreen(
            faceTransform: faceTransform,
            lookAtPoint: lookAtBackwards
        )

        // When ray direction is pointing backwards away from the screen, it must not be considered within screen
        XCTAssertFalse(result.isWithinScreen)
    }
}
