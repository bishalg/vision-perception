import CurvAsdGazeKit
import Foundation
#if canImport(ARKit) && !os(macOS)
import ARKit
#endif

// MARK: - Why capture cannot start (SIM-2, SIM-7, CNS-1)

/// Why live capture cannot start. Mirrors the kit's `TrackingUnavailableReason`
/// and adds the app-level `consentRequired`.
public enum DeviceBlockReason: String, CaseIterable, Sendable, Equatable {
    case faceTrackingUnsupported
    case cameraPermissionDenied
    case sessionFailed
    case interrupted
    case consentRequired

    public init(_ reason: TrackingUnavailableReason) {
        switch reason {
        case .faceTrackingUnsupported: self = .faceTrackingUnsupported
        case .cameraPermissionDenied:  self = .cameraPermissionDenied
        case .sessionFailed:           self = .sessionFailed
        case .interrupted:             self = .interrupted
        }
    }
}

/// Plain-language copy for `UnsupportedDeviceView`. Never offers simulation and
/// never uses diagnostic wording (ADR-0004).
public struct UnsupportedDeviceContent: Equatable {
    public let title: String
    public let message: String
    public let systemImage: String

    public static func make(for reason: DeviceBlockReason) -> UnsupportedDeviceContent {
        switch reason {
        case .faceTrackingUnsupported:
            return .init(
                title: "This device cannot run gaze observations",
                message: "Gaze observations need the TrueDepth front camera used by Face ID. Supported devices: iPhone X and later, and iPads with Face ID. You can still use the diary, reminders and notes on this device. Nothing here is a medical assessment.",
                systemImage: "eye.slash")
        case .cameraPermissionDenied:
            return .init(
                title: "Camera access is turned off",
                message: "Gaze observations need the front camera. Turn on Camera for this app in Settings. Frames are analysed on the device and are not recorded as video.",
                systemImage: "camera.fill")
        case .sessionFailed:
            return .init(
                title: "The camera session stopped unexpectedly",
                message: "No observation was recorded. Close any other app using the camera and try again.",
                systemImage: "exclamationmark.triangle")
        case .interrupted:
            return .init(
                title: "Observation interrupted",
                message: "The camera was interrupted, so this observation was not completed. Start again when you are ready.",
                systemImage: "pause.circle")
        case .consentRequired:
            return .init(
                title: "Finish setup first",
                message: "Gaze observations start only after you accept the privacy and consent step in setup.",
                systemImage: "lock.shield")
        }
    }
}

// MARK: - Entry gate

public enum CaptureEntryGate: Equatable {
    case ready(ConsentToken)
    case blocked(DeviceBlockReason)

    /// Hardware is checked first so unsupported devices never see a consent prompt
    /// they cannot use.
    @MainActor
    public static func evaluate(
        store: ChildProfileStore = .shared,
        faceTrackingSupported: Bool = CaptureEntryGate.deviceSupportsFaceTracking
    ) -> CaptureEntryGate {
        guard faceTrackingSupported else { return .blocked(.faceTrackingUnsupported) }
        guard let token = ConsentToken.mint(from: store) else { return .blocked(.consentRequired) }
        return .ready(token)
    }

    public static var deviceSupportsFaceTracking: Bool {
        #if canImport(ARKit) && !os(macOS)
        return ARFaceTrackingConfiguration.isSupported
        #else
        return false
        #endif
    }
}

public extension GazeSessionController {
    /// Mints a consent token from `store` and starts a consented session.
    /// Returns `false` (and starts nothing) when consent is missing. The kit may
    /// still move to `.unavailable`; views observe `trackingStage`.
    @MainActor
    @discardableResult
    func startSessionWithConsent(
        battery: StimulusAOITracker? = nil,
        stimulusId: String? = nil,
        store: ChildProfileStore = .shared
    ) -> Bool {
        guard let token = ConsentToken.mint(from: store) else { return false }
        return startConsentedSession(token: token, battery: battery, stimulusId: stimulusId, store: store)
    }
}
