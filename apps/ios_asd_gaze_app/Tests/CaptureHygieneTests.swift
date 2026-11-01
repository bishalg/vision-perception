@testable import AsdGazeCollector
import CoreGraphics
import CurvAsdGazeKit
import UIKit
import XCTest

/// WP-G.0: capture hygiene. TRK-1 is covered in the kit (`GazeProcessingActorTests`);
/// here CAL-1, CAL-3 and the orientation lock.
@MainActor
final class CaptureHygieneTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!

    override func setUp() {
        super.setUp()
        suiteName = "CaptureHygieneTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    // MARK: CAL-3 no fabricated target point

    private let measured = CGPoint(x: 0.4, y: 0.6)

    func testStarvedTargetIsRetriedOnceThenFailsWithoutRecordingAPoint() {
        var acc = CalibrationAccumulator(targetCount: 5)
        XCTAssertEqual(acc.submit(medianRaw: measured), .accepted(isComplete: false))
        XCTAssertEqual(acc.submit(medianRaw: nil), .retryTarget)
        XCTAssertEqual(acc.submit(medianRaw: nil), .failed)
        XCTAssertEqual(acc.rawPoints, [measured], "no point is invented for the starved target")
        XCTAssertFalse(acc.canFit, "a failed calibration must never be fitted or persisted")
        XCTAssertEqual(acc.submit(medianRaw: measured), .failed, "failure is terminal until a new attempt")
    }

    func testRetryThatCollectsSamplesRecoversAndRetriesResetPerTarget() {
        var acc = CalibrationAccumulator(targetCount: 3)
        XCTAssertEqual(acc.submit(medianRaw: nil), .retryTarget)
        XCTAssertEqual(acc.submit(medianRaw: measured), .accepted(isComplete: false))
        XCTAssertEqual(acc.submit(medianRaw: nil), .retryTarget, "next target gets its own single retry")
        XCTAssertEqual(acc.submit(medianRaw: measured), .accepted(isComplete: false))
        XCTAssertEqual(acc.submit(medianRaw: measured), .accepted(isComplete: true))
        XCTAssertTrue(acc.canFit)
        XCTAssertEqual(acc.rawPoints.count, 3)
    }

    func testIncompleteOrFitFailedAccumulatorCannotFit() {
        var acc = CalibrationAccumulator(targetCount: 2)
        _ = acc.submit(medianRaw: measured)
        XCTAssertFalse(acc.canFit)
        _ = acc.submit(medianRaw: measured)
        XCTAssertTrue(acc.canFit)
        acc.markFitFailed()
        XCTAssertFalse(acc.canFit)
    }

    func testCalibrationStarvedOfSamplesNeverPersists() {
        // Mirrors BubbleCalibrationView: persist only when `canFit`.
        var acc = CalibrationAccumulator(targetCount: 2)
        _ = acc.submit(medianRaw: measured)
        _ = acc.submit(medianRaw: nil)
        _ = acc.submit(medianRaw: nil)
        let controller = GazeSessionController()
        controller.calibrationBinding = CalibrationBinding(participantKey: "child-A", sessionId: "S1")
        defer { BoundCalibrationStore.reset() }
        if acc.canFit { XCTFail("must not fit") }
        XCTAssertFalse(controller.calibrationProfile.isCalibrated)
        XCTAssertFalse(BoundCalibrationStore.load(for: controller.calibrationBinding).isCalibrated)
    }

    // MARK: CAL-1 per participant and session

    func testSwitchingChildRotatesCalibrationSubjectKey() {
        let store = ChildProfileStore(userDefaults: defaults)
        let first = store.calibrationSubjectKey
        XCTAssertEqual(store.calibrationSubjectKey, first)
        store.profile.childName = "Another child"
        XCTAssertNotEqual(store.calibrationSubjectKey, first)
        let second = store.calibrationSubjectKey
        store.reset()
        XCTAssertNotEqual(store.calibrationSubjectKey, second)
    }

    func testSubjectKeyIsStableAcrossRelaunchForSameChild() {
        let key = ChildProfileStore(userDefaults: defaults).calibrationSubjectKey
        XCTAssertEqual(ChildProfileStore(userDefaults: defaults).calibrationSubjectKey, key)
    }

    func testCalibrationOfOneChildIsNotAppliedToAnother() {
        let store = ChildProfileStore(userDefaults: defaults)
        let childA = store.calibrationBinding(sessionId: "S1")
        let fitted = GazeCalibrationProfile(a: 1.1, b: 0, c: 0.01, d: 0, e: 0.9, f: 0, sampleCount: 5, createdAt: Date())
        BoundCalibrationStore.save(fitted, binding: childA)
        defer { BoundCalibrationStore.reset() }
        XCTAssertTrue(BoundCalibrationStore.load(for: childA).isCalibrated)

        store.profile.childName = "Child B"
        let childB = store.calibrationBinding(sessionId: "S1")
        XCTAssertNotEqual(childA, childB)
        XCTAssertFalse(BoundCalibrationStore.load(for: childB).isCalibrated)
        XCTAssertFalse(BoundCalibrationStore.load(for: store.calibrationBinding(sessionId: "S2")).isCalibrated)
    }

    func testWizardOffersNoSkipUseSavedCalibrationPath() {
        for locale in [AppLocale.english, .nepali, .hindi] {
            XCTAssertEqual(Localizer.tr("wizard_skip_calibrate", lang: locale), "wizard_skip_calibrate",
                           "the 'skip, use saved calibration' string must not exist")
        }
        XCTAssertNotEqual(Localizer.tr("calibrate_try_again", lang: .english), "calibrate_try_again")
    }

    // MARK: Orientation lock

    func testPhoneCaptureLocksToPortrait() {
        for current in [UIInterfaceOrientation.portrait, .landscapeLeft, .landscapeRight] {
            XCTAssertEqual(CaptureOrientationLock.captureMask(isPad: false, current: current), .portrait)
        }
    }

    func testPadCaptureKeepsTheOrientationInEffect() {
        XCTAssertEqual(CaptureOrientationLock.captureMask(isPad: true, current: .landscapeLeft), .landscapeLeft)
        XCTAssertEqual(CaptureOrientationLock.captureMask(isPad: true, current: .landscapeRight), .landscapeRight)
        XCTAssertEqual(CaptureOrientationLock.captureMask(isPad: true, current: .portrait), .portrait)
        XCTAssertEqual(CaptureOrientationLock.captureMask(isPad: true, current: .portraitUpsideDown), .portraitUpsideDown)
    }

    func testLockIsReferenceCountedAndReleasesWhenLastHolderLeaves() {
        let lock = CaptureOrientationLock.shared
        XCTAssertEqual(lock.holders, 0)
        lock.acquire()
        lock.acquire()
        XCTAssertNotNil(lock.lockedMask)
        lock.release()
        XCTAssertNotNil(lock.lockedMask, "nested views must not release the wizard's hold")
        lock.release()
        XCTAssertNil(lock.lockedMask)
        XCTAssertEqual(lock.holders, 0)
        XCTAssertEqual(lock.supportedMask, CaptureOrientationLock.unlockedMask(isPad: UIDevice.current.userInterfaceIdiom == .pad))
    }
}
