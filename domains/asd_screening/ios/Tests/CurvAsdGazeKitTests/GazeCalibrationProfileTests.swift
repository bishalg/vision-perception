import XCTest
import CoreGraphics
@testable import CurvAsdGazeKit

final class GazeCalibrationProfileTests: XCTestCase {
    func testIdentityIsPassthrough() {
        let raw = CGPoint(x: 0.31, y: 0.72)
        let out = GazeCalibrationProfile.identity.apply(to: raw)
        XCTAssertEqual(out.x, raw.x, accuracy: 1e-9)
        XCTAssertEqual(out.y, raw.y, accuracy: 1e-9)
        XCTAssertFalse(GazeCalibrationProfile.identity.isCalibrated)
    }

    /// The core regression this whole fix is about: a raw signal with a
    /// constant offset bias (the dominant real-world error mode — the eye
    /// model's lookAtPoint anchor is systematically off for a given user)
    /// must be corrected back onto the true 5-point target grid.
    func testFitCorrectsConstantOffsetBias() {
        let targets: [CGPoint] = [
            CGPoint(x: 0.50, y: 0.50),
            CGPoint(x: 0.22, y: 0.24),
            CGPoint(x: 0.78, y: 0.24),
            CGPoint(x: 0.22, y: 0.76),
            CGPoint(x: 0.78, y: 0.76)
        ]
        let biasX = 0.12, biasY = -0.08
        let rawPoints = targets.map { CGPoint(x: $0.x + biasX, y: $0.y + biasY) }

        guard let profile = GazeCalibrationProfile.fit(rawPoints: rawPoints, targetPoints: targets) else {
            return XCTFail("fit should succeed for a well-conditioned 5-point grid")
        }
        XCTAssertTrue(profile.isCalibrated)

        for (raw, target) in zip(rawPoints, targets) {
            let corrected = profile.apply(to: raw)
            XCTAssertEqual(corrected.x, target.x, accuracy: 1e-6)
            XCTAssertEqual(corrected.y, target.y, accuracy: 1e-6)
        }
    }

    /// A per-axis scale error (e.g. the screen-width-meters constant being
    /// slightly wrong for a specific device) should also be absorbed.
    func testFitCorrectsScaleAndOffset() {
        let targets: [CGPoint] = [
            CGPoint(x: 0.50, y: 0.50),
            CGPoint(x: 0.22, y: 0.24),
            CGPoint(x: 0.78, y: 0.24),
            CGPoint(x: 0.22, y: 0.76),
            CGPoint(x: 0.78, y: 0.76)
        ]
        // Raw = 1.3x scale around center (0.5, 0.5) plus a small offset.
        let rawPoints = targets.map {
            CGPoint(x: 0.5 + ($0.x - 0.5) * 1.3 + 0.03, y: 0.5 + ($0.y - 0.5) * 1.3 - 0.02)
        }

        guard let profile = GazeCalibrationProfile.fit(rawPoints: rawPoints, targetPoints: targets) else {
            return XCTFail("fit should succeed")
        }
        for (raw, target) in zip(rawPoints, targets) {
            let corrected = profile.apply(to: raw)
            XCTAssertEqual(corrected.x, target.x, accuracy: 1e-6)
            XCTAssertEqual(corrected.y, target.y, accuracy: 1e-6)
        }
    }

    func testFitFailsWithTooFewPoints() {
        let points = [CGPoint(x: 0.5, y: 0.5), CGPoint(x: 0.2, y: 0.2)]
        XCTAssertNil(GazeCalibrationProfile.fit(rawPoints: points, targetPoints: points))
    }

    func testFitFailsWithMismatchedCounts() {
        let raw = [CGPoint(x: 0.5, y: 0.5), CGPoint(x: 0.2, y: 0.2), CGPoint(x: 0.8, y: 0.2)]
        let targets = [CGPoint(x: 0.5, y: 0.5), CGPoint(x: 0.2, y: 0.2)]
        XCTAssertNil(GazeCalibrationProfile.fit(rawPoints: raw, targetPoints: targets))
    }

    func testFitFailsWithDegenerateIdenticalPoints() {
        // All 5 raw points identical -> singular system, must not crash or produce NaN.
        let raw = Array(repeating: CGPoint(x: 0.5, y: 0.5), count: 5)
        let targets: [CGPoint] = [
            CGPoint(x: 0.50, y: 0.50),
            CGPoint(x: 0.22, y: 0.24),
            CGPoint(x: 0.78, y: 0.24),
            CGPoint(x: 0.22, y: 0.76),
            CGPoint(x: 0.78, y: 0.76)
        ]
        XCTAssertNil(GazeCalibrationProfile.fit(rawPoints: raw, targetPoints: targets))
    }

    func testApplyClampsToValidRange() {
        // a=2 doubles x around the origin; a raw point near the edge should
        // clamp into [0,1], not extrapolate off-screen.
        let profile = GazeCalibrationProfile(a: 2, b: 0, c: -0.5, d: 0, e: 1, f: 0, sampleCount: 5, createdAt: Date())
        let out = profile.apply(to: CGPoint(x: 0.95, y: 0.5))
        XCTAssertEqual(out.x, 1.0, accuracy: 1e-9)
    }

    func testStoreRoundTrip() {
        GazeCalibrationStore.reset()
        XCTAssertFalse(GazeCalibrationStore.load().isCalibrated)

        let profile = GazeCalibrationProfile(a: 1.1, b: 0.02, c: -0.03, d: 0.01, e: 0.95, f: 0.04, sampleCount: 5, createdAt: Date())
        GazeCalibrationStore.save(profile)
        let loaded = GazeCalibrationStore.load()
        XCTAssertEqual(loaded.a, profile.a, accuracy: 1e-9)
        XCTAssertEqual(loaded.sampleCount, 5)
        XCTAssertTrue(loaded.isCalibrated)

        GazeCalibrationStore.reset()
        XCTAssertFalse(GazeCalibrationStore.load().isCalibrated)
    }
}
