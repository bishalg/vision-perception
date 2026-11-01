import XCTest
@testable import CurvAsdGazeKit

/// CAL-1: a calibration belongs to one participant and one session.
final class BoundCalibrationTests: XCTestCase {
    private var defaults: UserDefaults!
    private let suite = "BoundCalibrationTests"

    private let fitted = GazeCalibrationProfile(
        a: 1.08, b: 0, c: 0.02, d: 0, e: 0.95, f: -0.01, sampleCount: 5, createdAt: Date()
    )

    override func setUp() {
        defaults = UserDefaults(suiteName: suite)
        defaults.removePersistentDomain(forName: suite)
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
    }

    func testSameBindingLoadsProfile() {
        let binding = CalibrationBinding(participantKey: "child-A", sessionId: "S1")
        BoundCalibrationStore.save(fitted, binding: binding, defaults: defaults)
        XCTAssertEqual(BoundCalibrationStore.load(for: binding, defaults: defaults).a, 1.08, accuracy: 1e-9)
    }

    func testOtherChildGetsIdentity() {
        BoundCalibrationStore.save(fitted, binding: .init(participantKey: "child-A", sessionId: "S1"), defaults: defaults)
        let other = BoundCalibrationStore.load(for: .init(participantKey: "child-B", sessionId: "S1"), defaults: defaults)
        XCTAssertFalse(other.isCalibrated)
    }

    func testLaterSessionOfSameChildGetsIdentity() {
        BoundCalibrationStore.save(fitted, binding: .init(participantKey: "child-A", sessionId: "S1"), defaults: defaults)
        let later = BoundCalibrationStore.load(for: .init(participantKey: "child-A", sessionId: "S2"), defaults: defaults)
        XCTAssertFalse(later.isCalibrated)
    }

    func testNoBindingNeverLoadsAProfile() {
        BoundCalibrationStore.save(fitted, binding: .init(participantKey: "child-A", sessionId: "S1"), defaults: defaults)
        XCTAssertFalse(BoundCalibrationStore.load(for: nil, defaults: defaults).isCalibrated)
    }

    func testResetClearsStoredProfile() {
        let binding = CalibrationBinding(participantKey: "child-A", sessionId: "S1")
        BoundCalibrationStore.save(fitted, binding: binding, defaults: defaults)
        BoundCalibrationStore.reset(defaults: defaults)
        XCTAssertFalse(BoundCalibrationStore.load(for: binding, defaults: defaults).isCalibrated)
    }

    @MainActor
    func testControllerWithoutBindingStartsUncalibratedEvenWithLegacyGlobalProfile() {
        GazeCalibrationStore.save(fitted)
        defer { GazeCalibrationStore.reset() }
        let controller = GazeSessionController()
        XCTAssertFalse(controller.calibrationProfile.isCalibrated)
        controller.reloadCalibrationProfile()
        XCTAssertFalse(controller.calibrationProfile.isCalibrated)
    }
}
