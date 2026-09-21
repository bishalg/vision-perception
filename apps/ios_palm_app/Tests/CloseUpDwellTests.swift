import XCTest
@testable import PalmScanner
import CurvCameraOverlay

// MARK: - Close-Up Dwell Gate Unit Tests
//
// Validates that the Close-Up mosaic scan requires a continuous 0.6-second
// "hold steady" dwell before auto-firing a tile capture.
// Mirrors the fingertip dwell gate tests in OrbitCaptureTests.swift.

final class CloseUpDwellTests: XCTestCase {

    // MARK: - CloseUpTileGuidance.evaluate()

    func testEvaluateReturnsReadyWhenPoseAndLightingOK() {
        let guidance = CloseUpTileGuidance()
        let photometrics = PhotometricSample(meanLuminance: 0.45, glareRatio: 0.05)
        let pose = SubjectPose(
            centroid: CGPoint(x: 0.5, y: 0.5),
            scale: 0.55,
            rollDeg: 0,
            aspectRatio: 1.0
        )
        let result = guidance.evaluate(pose: pose, photometrics: photometrics, target: .thenar)
        XCTAssertTrue(result.ready, "Should be ready when pose.scale >= 0.52 and lighting is OK")
    }

    func testEvaluateNotReadyWhenScaleTooLow() {
        let guidance = CloseUpTileGuidance()
        let photometrics = PhotometricSample(meanLuminance: 0.45, glareRatio: 0.05)
        let pose = SubjectPose(
            centroid: CGPoint(x: 0.5, y: 0.5),
            scale: 0.30,   // Too small for thenar (needs >= 0.52)
            rollDeg: 0,
            aspectRatio: 1.0
        )
        let result = guidance.evaluate(pose: pose, photometrics: photometrics, target: .thenar)
        XCTAssertFalse(result.ready, "Should NOT be ready when scale < minScale")
    }

    func testEvaluateNotReadyWhenLuminanceTooLow() {
        let guidance = CloseUpTileGuidance()
        let photometrics = PhotometricSample(meanLuminance: 0.05, glareRatio: 0.0) // below min
        let pose = SubjectPose(
            centroid: CGPoint(x: 0.5, y: 0.5),
            scale: 0.60,
            rollDeg: 0,
            aspectRatio: 1.0
        )
        let result = guidance.evaluate(pose: pose, photometrics: photometrics, target: .distal)
        XCTAssertFalse(result.ready, "Should NOT be ready when luminance is too low")
    }

    func testEvaluateNotReadyWhenNoPose() {
        let guidance = CloseUpTileGuidance()
        let photometrics = PhotometricSample(meanLuminance: 0.45, glareRatio: 0.05)
        let result = guidance.evaluate(pose: nil, photometrics: photometrics, target: .midPalm)
        XCTAssertFalse(result.ready, "Should NOT be ready with no pose")
        XCTAssertFalse(result.message.isEmpty, "Should provide a coaching message when no pose")
    }

    // MARK: - CloseUpDwellGate logic (pure value type, no UIKit)

    func testDwellGateAccumulatesProgressMonotonically() {
        var gate = CloseUpDwellGate(requiredDuration: 0.6)
        let t0: CFTimeInterval = 1000.0

        let p0 = gate.update(evalReady: true, now: t0)
        XCTAssertEqual(p0, 0.0, accuracy: 0.01, "At t=0, progress should be 0")

        let p1 = gate.update(evalReady: true, now: t0 + 0.3)
        XCTAssertEqual(p1, 0.5, accuracy: 0.05, "At t=0.3s of 0.6s gate, progress ~50%")

        let p2 = gate.update(evalReady: true, now: t0 + 0.6)
        XCTAssertEqual(p2, 1.0, accuracy: 0.01, "At t=0.6s, progress should be 1.0")
    }

    func testDwellGateFiresOnlyAfterRequiredDuration() {
        var gate = CloseUpDwellGate(requiredDuration: 0.6)
        let t0: CFTimeInterval = 1000.0

        XCTAssertFalse(gate.shouldFire(evalReady: true, now: t0), "Should NOT fire at t=0")
        XCTAssertFalse(gate.shouldFire(evalReady: true, now: t0 + 0.3), "Should NOT fire at 0.3s")
        XCTAssertFalse(gate.shouldFire(evalReady: true, now: t0 + 0.59), "Should NOT fire at 0.59s")
        XCTAssertTrue(gate.shouldFire(evalReady: true, now: t0 + 0.60), "Should fire at exactly 0.6s")
    }

    func testDwellGateResetsWhenReadinessBroken() {
        var gate = CloseUpDwellGate(requiredDuration: 0.6)
        let t0: CFTimeInterval = 1000.0

        // Accumulate to 50%
        _ = gate.update(evalReady: true, now: t0)
        _ = gate.update(evalReady: true, now: t0 + 0.3)

        // Lose readiness
        let resetProgress = gate.update(evalReady: false, now: t0 + 0.4)
        XCTAssertEqual(resetProgress, 0.0, accuracy: 0.01,
                       "Progress must reset to 0 when eval is not ready")

        // Re-acquire should restart from scratch
        let restartProgress = gate.update(evalReady: true, now: t0 + 0.5)
        XCTAssertEqual(restartProgress, 0.0, accuracy: 0.01,
                       "After reset, progress must restart from 0")
    }

    func testDwellGateDoesNotDoubleFire() {
        var gate = CloseUpDwellGate(requiredDuration: 0.6)
        let t0: CFTimeInterval = 1000.0

        // Fire once
        XCTAssertTrue(gate.shouldFire(evalReady: true, now: t0 + 0.60))

        // Explicit reset (caller resets after capturing)
        gate.reset()

        // Should not fire again without re-arming
        XCTAssertFalse(gate.shouldFire(evalReady: true, now: t0 + 0.65),
                        "After reset(), gate should not re-fire without a new ready acquisition")
    }

    func testDwellGateSequentialTilesFireIndependently() {
        var gate = CloseUpDwellGate(requiredDuration: 0.6)
        let t0: CFTimeInterval = 2000.0

        // Tile 1 fires
        XCTAssertTrue(gate.shouldFire(evalReady: true, now: t0 + 0.60))
        gate.reset()

        // Tile 2 starts fresh
        let freshProgress = gate.update(evalReady: true, now: t0 + 0.61)
        XCTAssertEqual(freshProgress, 0.0, accuracy: 0.01, "Tile 2 starts fresh after reset")

        // Tile 2 fires 0.6s after its own re-arm
        XCTAssertTrue(gate.shouldFire(evalReady: true, now: t0 + 0.61 + 0.60),
                       "Tile 2 should fire 0.6s after its own re-arm")
    }

    // MARK: - CloseUpTile metadata

    func testAllTilesHaveUniqueRawValues() {
        let rawValues = CloseUpTile.allCases.map(\.rawValue)
        XCTAssertEqual(rawValues.count, Set(rawValues).count,
                       "All CloseUpTile raw values must be unique")
    }

    func testAllTilesHaveDisplayNames() {
        for tile in CloseUpTile.allCases {
            XCTAssertFalse(tile.displayName.isEmpty,
                           "\(tile.rawValue) must have a displayName")
        }
    }

    func testAllTilesHaveValidMinScale() {
        for tile in CloseUpTile.allCases {
            XCTAssertGreaterThan(tile.minScale, 0.0,
                                 "\(tile.rawValue) minScale must be > 0")
            XCTAssertLessThan(tile.minScale, 1.0,
                              "\(tile.rawValue) minScale must be < 1")
        }
    }

    private func dummyIntrinsics() -> CameraIntrinsics {
        CameraIntrinsics(
            focalLengthX: 1000, focalLengthY: 1000,
            principalPointX: 960, principalPointY: 540,
            imageWidth: 1920, imageHeight: 1080
        )
    }

    func testNextTileReturnsFirstUnfilledTile() {
        let guidance = CloseUpTileGuidance()
        var session = CloseUpMosaicSession()

        // Initially first tile (thenar) should be next
        XCTAssertEqual(guidance.nextTile(in: session), .thenar)

        // Mark thenar as filled — distal should be next
        let dummyFrame = CloseUpTileFrame(
            tile: .thenar,
            timestamp: 0,
            landmarks: [],
            intrinsics: dummyIntrinsics(),
            imageWidth: 1920,
            imageHeight: 1080
        )
        session.tiles.append(dummyFrame)
        XCTAssertEqual(guidance.nextTile(in: session), .distal)
    }

    func testNextTileReturnsNilWhenAllFilled() {
        let guidance = CloseUpTileGuidance()
        var session = CloseUpMosaicSession()

        for tile in CloseUpTile.allCases {
            let frame = CloseUpTileFrame(
                tile: tile,
                timestamp: 0,
                landmarks: [],
                intrinsics: dummyIntrinsics(),
                imageWidth: 1920,
                imageHeight: 1080
            )
            session.tiles.append(frame)
        }
        XCTAssertNil(guidance.nextTile(in: session),
                     "nextTile should return nil when all tiles are captured")
    }

}
