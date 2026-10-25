@testable import AsdGazeCollector
import XCTest

/// CAM-1 / CAM-3: camera permission state machine, idle timer and scene phase.
@MainActor
final class CameraAccessCoordinatorTests: XCTestCase {
    private final class FakeAuthorizer: CameraAuthorizing {
        var status: CameraAuthorizationStatus
        var grantOnRequest: Bool
        private(set) var requestCount = 0
        init(status: CameraAuthorizationStatus, grantOnRequest: Bool = true) {
            self.status = status
            self.grantOnRequest = grantOnRequest
        }
        func requestAccess() async -> Bool {
            requestCount += 1
            status = grantOnRequest ? .authorized : .denied
            return grantOnRequest
        }
    }

    private final class FakeIdleTimer: IdleTimerControlling {
        private(set) var writes: [Bool] = []
        var isIdleTimerDisabled: Bool { didSet { writes.append(isIdleTimerDisabled) } }
        init(initial: Bool = false) { isIdleTimerDisabled = initial }
    }

    private func make(
        _ status: CameraAuthorizationStatus,
        grant: Bool = true,
        idle: FakeIdleTimer = FakeIdleTimer()
    ) -> (CameraAccessCoordinator, FakeAuthorizer, FakeIdleTimer) {
        let auth = FakeAuthorizer(status: status, grantOnRequest: grant)
        return (CameraAccessCoordinator(authorizer: auth, idleTimer: idle), auth, idle)
    }

    // MARK: Authorization (CAM-1)

    func testAuthorizedDoesNotPrompt() async {
        let (c, auth, _) = make(.authorized)
        let state = await c.ensureAccess()
        XCTAssertEqual(state, .granted)
        XCTAssertEqual(auth.requestCount, 0)
    }

    func testNotDeterminedRequestsAndGrants() async {
        let (c, auth, _) = make(.notDetermined, grant: true)
        let state = await c.ensureAccess()
        XCTAssertEqual(state, .granted)
        XCTAssertEqual(auth.requestCount, 1)
        XCTAssertEqual(c.state, .granted)
    }

    func testNotDeterminedRequestsAndDenies() async {
        let (c, auth, _) = make(.notDetermined, grant: false)
        let state = await c.ensureAccess()
        XCTAssertEqual(state, .denied)
        XCTAssertEqual(auth.requestCount, 1)
    }

    func testDeniedNeverPromptsAgain() async {
        let (c, auth, _) = make(.denied)
        let state = await c.ensureAccess()
        XCTAssertEqual(state, .denied)
        XCTAssertEqual(auth.requestCount, 0)
        XCTAssertTrue(state.showsDeniedView)
    }

    func testRestrictedNeverPrompts() async {
        let (c, auth, _) = make(.restricted)
        let state = await c.ensureAccess()
        XCTAssertEqual(state, .restricted)
        XCTAssertEqual(auth.requestCount, 0)
        XCTAssertTrue(state.showsDeniedView)
    }

    func testInitialStateIsUnknownAndNotAllowed() {
        let (c, _, _) = make(.authorized)
        XCTAssertEqual(c.state, .unknown)
        XCTAssertFalse(c.state.showsDeniedView)
        XCTAssertFalse(c.canStartSession)
    }

    func testNoSessionStartsWhenDenied() async {
        let (c, _, idle) = make(.denied)
        _ = await c.ensureAccess()
        XCTAssertFalse(c.canStartSession)
        c.sessionDidStart()
        XCTAssertFalse(c.isSessionActive)
        XCTAssertFalse(idle.isIdleTimerDisabled)
        XCTAssertTrue(idle.writes.isEmpty)
    }

    func testUserTurnsCameraOnInSettingsThenReturns() async {
        let (c, auth, _) = make(.denied)
        _ = await c.ensureAccess()
        auth.status = .authorized
        _ = c.scenePhaseChanged(.active)
        XCTAssertEqual(c.state, .granted)
    }

    func testRevokedInSettingsWhileAwayBecomesDenied() async {
        let (c, auth, _) = make(.authorized)
        _ = await c.ensureAccess()
        auth.status = .denied
        _ = c.scenePhaseChanged(.active)
        XCTAssertEqual(c.state, .denied)
    }

    // MARK: Denied view content and Settings link

    func testDeniedContentOffersSettingsButRestrictedDoesNot() {
        XCTAssertTrue(CameraDeniedContent.make(for: .denied).offersOpenSettings)
        XCTAssertFalse(CameraDeniedContent.make(for: .restricted).offersOpenSettings)
        XCTAssertNotEqual(
            CameraDeniedContent.make(for: .denied).message,
            CameraDeniedContent.make(for: .restricted).message)
    }

    func testSettingsDeepLinkIsAppSettingsURL() {
        XCTAssertNotNil(CameraAccessCoordinator.settingsURL)
        XCTAssertEqual(CameraAccessCoordinator.settingsURL?.absoluteString, UIApplication.openSettingsURLString)
    }

    func testAccessibilityIdentifiersAreStable() {
        XCTAssertEqual(CameraDeniedAccessibility.view, "camera-denied-view")
        XCTAssertEqual(CameraDeniedAccessibility.openSettings, "camera-denied-open-settings")
        XCTAssertEqual(UnsupportedDeviceAccessibility.view, "unsupported-device-view")
    }

    // MARK: Idle timer (CAM-3)

    func testIdleTimerDisabledOnlyWhileSessionActive() async {
        let (c, _, idle) = make(.authorized)
        _ = await c.ensureAccess()
        XCTAssertFalse(idle.isIdleTimerDisabled)
        c.sessionDidStart()
        XCTAssertTrue(c.isSessionActive)
        XCTAssertTrue(idle.isIdleTimerDisabled)
        c.sessionDidEnd()
        XCTAssertFalse(c.isSessionActive)
        XCTAssertFalse(idle.isIdleTimerDisabled)
    }

    func testIdleTimerRestoresPreviousValueNotAlwaysFalse() async {
        let (c, _, idle) = make(.authorized, idle: FakeIdleTimer(initial: true))
        _ = await c.ensureAccess()
        c.sessionDidStart()
        c.sessionDidStart()  // idempotent: must not capture "true" as the previous value
        c.sessionDidEnd()
        XCTAssertTrue(idle.isIdleTimerDisabled)
    }

    func testSessionEndWithoutStartDoesNotTouchIdleTimer() async {
        let (c, _, idle) = make(.authorized)
        _ = await c.ensureAccess()
        c.sessionDidEnd()
        XCTAssertTrue(idle.writes.isEmpty)
    }

    func testRepeatedEndIsHarmless() async {
        let (c, _, idle) = make(.authorized)
        _ = await c.ensureAccess()
        c.sessionDidStart()
        c.sessionDidEnd()
        c.sessionDidEnd()
        XCTAssertFalse(idle.isIdleTimerDisabled)
        XCTAssertEqual(idle.writes, [true, false])
    }

    // MARK: Scene phase (CAM-3)

    func testBackgroundDuringSessionStopsAndRestoresIdleTimer() async {
        let (c, _, idle) = make(.authorized)
        _ = await c.ensureAccess()
        c.sessionDidStart()
        let action = c.scenePhaseChanged(.background)
        XCTAssertEqual(action, .stopSession)
        XCTAssertFalse(c.isSessionActive)
        XCTAssertFalse(idle.isIdleTimerDisabled)
        XCTAssertTrue(c.wasInterruptedByBackground)
    }

    func testInactiveDoesNotStopSession() async {
        let (c, _, idle) = make(.authorized)
        _ = await c.ensureAccess()
        c.sessionDidStart()
        XCTAssertEqual(c.scenePhaseChanged(.inactive), .none)
        XCTAssertTrue(c.isSessionActive)
        XCTAssertTrue(idle.isIdleTimerDisabled)
    }

    func testBackgroundWithoutSessionDoesNothing() async {
        let (c, _, _) = make(.authorized)
        _ = await c.ensureAccess()
        XCTAssertEqual(c.scenePhaseChanged(.background), .none)
        XCTAssertFalse(c.wasInterruptedByBackground)
    }

    func testReturningToForegroundReportsInterruptionOnce() async {
        let (c, _, _) = make(.authorized)
        _ = await c.ensureAccess()
        c.sessionDidStart()
        _ = c.scenePhaseChanged(.background)
        XCTAssertEqual(c.scenePhaseChanged(.active), .backgroundInterruptionEnded)
        XCTAssertFalse(c.wasInterruptedByBackground)
        XCTAssertEqual(c.scenePhaseChanged(.active), .none)
    }

    func testReturningToForegroundWithCameraRevokedDoesNotResume() async {
        let (c, auth, _) = make(.authorized)
        _ = await c.ensureAccess()
        c.sessionDidStart()
        _ = c.scenePhaseChanged(.background)
        auth.status = .denied
        _ = c.scenePhaseChanged(.active)
        XCTAssertEqual(c.state, .denied)
        XCTAssertFalse(c.canStartSession)
    }
}

// CNS-1 ordering: no camera prompt before consent.
@MainActor
final class CameraAccessConsentOrderingTests: XCTestCase {
    private final class Auth: CameraAuthorizing {
        var status: CameraAuthorizationStatus = .notDetermined
        private(set) var requests = 0
        func requestAccess() async -> Bool { requests += 1; status = .authorized; return true }
    }
    private final class Idle: IdleTimerControlling { var isIdleTimerDisabled = false }

    func testNoPromptBeforeConsent() async {
        let suite = "CameraConsentOrder_\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let store = ChildProfileStore(userDefaults: defaults)
        let auth = Auth()
        let c = CameraAccessCoordinator(authorizer: auth, idleTimer: Idle())

        let before = await c.ensureAccess(consentStore: store)
        XCTAssertEqual(before, .unknown)
        XCTAssertEqual(auth.requests, 0)

        store.profile.hasConsentedPIA = true
        let after = await c.ensureAccess(consentStore: store)
        XCTAssertEqual(after, .granted)
        XCTAssertEqual(auth.requests, 1)
    }
}
