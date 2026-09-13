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
}
