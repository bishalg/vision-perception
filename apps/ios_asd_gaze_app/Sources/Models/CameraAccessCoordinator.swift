import AVFoundation
import Combine
import Foundation
#if canImport(UIKit)
import UIKit
#endif

// MARK: - Injected seams (CAM-1, CAM-3)

public enum CameraAuthorizationStatus: Equatable, Sendable {
    case notDetermined
    case restricted
    case denied
    case authorized
}

/// Reads and requests camera authorization. The system implementation wraps
/// `AVCaptureDevice`; tests inject a fake.
@MainActor
public protocol CameraAuthorizing: AnyObject {
    var status: CameraAuthorizationStatus { get }
    func requestAccess() async -> Bool
}

/// Controls `UIApplication.isIdleTimerDisabled`. Tests inject a fake.
@MainActor
public protocol IdleTimerControlling: AnyObject {
    var isIdleTimerDisabled: Bool { get set }
}

public final class SystemCameraAuthorizer: CameraAuthorizing {
    public init() {}

    public var status: CameraAuthorizationStatus {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .notDetermined: return .notDetermined
        case .restricted:    return .restricted
        case .denied:        return .denied
        case .authorized:    return .authorized
        @unknown default:    return .denied
        }
    }

    public func requestAccess() async -> Bool {
        await AVCaptureDevice.requestAccess(for: .video)
    }
}

#if canImport(UIKit)
@MainActor
public final class SystemIdleTimer: IdleTimerControlling {
    public init() {}
    public var isIdleTimerDisabled: Bool {
        get { UIApplication.shared.isIdleTimerDisabled }
        set { UIApplication.shared.isIdleTimerDisabled = newValue }
    }
}
#endif

// MARK: - State

public enum CameraAccessState: Equatable, Sendable {
    case unknown
    case requesting
    case granted
    case denied
    case restricted

    /// Denied and restricted both end in `CameraDeniedView`; no session starts.
    public var showsDeniedView: Bool { self == .denied || self == .restricted }
}

/// App scene phase, decoupled from SwiftUI so the state machine is testable.
public enum CameraScenePhase: Equatable, Sendable {
    case active
    case inactive
    case background
}

public enum CameraSceneAction: Equatable, Sendable {
    case none
    /// The app went to the background during a session: the view must stop it.
    case stopSession
    /// Foreground again after a `.stopSession`: the view resumes (preview steps)
    /// or ends the observation honestly (recorded steps).
    case backgroundInterruptionEnded
}

// MARK: - Coordinator

/// CAM-1: permission check before any session. CAM-3: idle timer held only while a
/// session is active and restored on every exit path; background stops the session.
@MainActor
public final class CameraAccessCoordinator: ObservableObject {
    @Published public private(set) var state: CameraAccessState = .unknown
    @Published public private(set) var isSessionActive = false
    @Published public private(set) var wasInterruptedByBackground = false

    private let authorizer: CameraAuthorizing
    private let idleTimer: IdleTimerControlling
    private var idleValueBeforeSession: Bool?

    public init(authorizer: CameraAuthorizing, idleTimer: IdleTimerControlling) {
        self.authorizer = authorizer
        self.idleTimer = idleTimer
    }

    #if canImport(UIKit)
    public convenience init() {
        self.init(authorizer: SystemCameraAuthorizer(), idleTimer: SystemIdleTimer())
    }
    #endif

    public var canStartSession: Bool { state == .granted }

    public static var settingsURL: URL? {
        #if canImport(UIKit)
        return URL(string: UIApplication.openSettingsURLString)
        #else
        return nil
        #endif
    }

    /// Checks (and, when undetermined, requests) camera access. Never prompts when
    /// the status is already decided.
    @discardableResult
    public func ensureAccess() async -> CameraAccessState {
        switch authorizer.status {
        case .authorized: state = .granted
        case .denied:     state = .denied
        case .restricted: state = .restricted
        case .notDetermined:
            state = .requesting
            let granted = await authorizer.requestAccess()
            state = granted ? .granted : .denied
        }
        return state
    }

    /// CNS-1 ordering: the system camera prompt never appears before the parent has
    /// consented. Without consent this returns the unchanged state and asks for nothing.
    @discardableResult
    public func ensureAccess(consentStore store: ChildProfileStore) async -> CameraAccessState {
        guard ConsentToken.mint(from: store) != nil else { return state }
        return await ensureAccess()
    }

    /// Re-reads the status without prompting (returning from Settings).
    private func refreshWithoutPrompt() {
        switch authorizer.status {
        case .authorized: state = .granted
        case .denied:     state = .denied
        case .restricted: state = .restricted
        case .notDetermined: if state != .requesting { state = .unknown }
        }
    }

    public func sessionDidStart() {
        guard canStartSession, !isSessionActive else { return }
        isSessionActive = true
        wasInterruptedByBackground = false
        idleValueBeforeSession = idleTimer.isIdleTimerDisabled
        idleTimer.isIdleTimerDisabled = true
    }

    public func sessionDidEnd() {
        guard isSessionActive else { return }
        isSessionActive = false
        restoreIdleTimer()
    }

    public func scenePhaseChanged(_ phase: CameraScenePhase) -> CameraSceneAction {
        switch phase {
        case .inactive:
            return .none
        case .background:
            guard isSessionActive else { return .none }
            isSessionActive = false
            wasInterruptedByBackground = true
            restoreIdleTimer()
            return .stopSession
        case .active:
            refreshWithoutPrompt()
            if wasInterruptedByBackground {
                wasInterruptedByBackground = false
                return .backgroundInterruptionEnded
            }
            return .none
        }
    }

    private func restoreIdleTimer() {
        guard let previous = idleValueBeforeSession else { return }
        idleValueBeforeSession = nil
        idleTimer.isIdleTimerDisabled = previous
    }
}
