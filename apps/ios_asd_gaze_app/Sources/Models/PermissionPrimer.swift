import AVFoundation
import Foundation
#if canImport(CoreMotion)
import CoreMotion
#endif
#if canImport(UserNotifications)
import UserNotifications
#endif
#if canImport(Speech)
import Speech
#endif
#if canImport(Photos)
import Photos
#endif
#if canImport(HealthKit)
import HealthKit
#endif

// MARK: - Permission primers (store-collection-build.md S-11)

/// Current iOS permission state, as shown on the primer and in Privacy & Your Data.
public enum PermissionStatus: Equatable, Sendable {
    case notDetermined
    case granted
    case limited
    case denied
    case restricted
    /// iOS does not ask for this permission at run time.
    case noPromptNeeded
    /// iOS cannot report this state; the system asks each time it is needed.
    case onRequest
    /// The device or this build does not support it.
    case unavailable

    public var displayText: String {
        switch self {
        case .notDetermined: return ConsentL10n.text("perm.status.notasked", "Not asked yet")
        case .granted: return ConsentL10n.text("perm.status.allowed", "Allowed")
        case .limited: return ConsentL10n.text("perm.status.limited", "Limited")
        case .denied: return ConsentL10n.text("perm.status.denied", "Off")
        case .restricted: return ConsentL10n.text("perm.status.restricted", "Restricted on this device")
        case .noPromptNeeded: return ConsentL10n.text("perm.status.noprompt", "No permission needed")
        case .onRequest: return ConsentL10n.text("perm.status.onrequest", "On request")
        case .unavailable: return ConsentL10n.text("perm.status.unavailable", "Not available on this device")
        }
    }
}

/// Every system permission the app can ask for. Each gets an in-app primer before the
/// system prompt: what is used, why, what is never stored, and how to change it later.
public enum PermissionKind: String, CaseIterable, Identifiable, Sendable {
    case camera
    case motion
    case notifications
    case speechAndMicrophone
    case photos
    case healthKit
    case ageRange

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .camera: return ConsentL10n.text("perm.camera.title", "Camera")
        case .motion: return ConsentL10n.text("perm.motion.title", "Motion")
        case .notifications: return ConsentL10n.text("perm.notifications.title", "Notifications")
        case .speechAndMicrophone: return ConsentL10n.text("perm.speech.title", "Speech and microphone")
        case .photos: return ConsentL10n.text("perm.photos.title", "Photos")
        case .healthKit: return ConsentL10n.text("perm.health.title", "Health")
        case .ageRange: return ConsentL10n.text("perm.age.title", "Age range")
        }
    }

    public var systemImage: String {
        switch self {
        case .camera: return "camera"
        case .motion: return "gyroscope"
        case .notifications: return "bell.badge"
        case .speechAndMicrophone: return "mic"
        case .photos: return "photo.on.rectangle"
        case .healthKit: return "heart"
        case .ageRange: return "person.badge.shield.checkmark"
        }
    }

    public var whatWeUse: String {
        switch self {
        case .camera:
            return ConsentL10n.text("perm.camera.use", "The front camera, while a session is running.")
        case .motion:
            return ConsentL10n.text("perm.motion.use", "How the iPhone or iPad moves during a session.")
        case .notifications:
            return ConsentL10n.text("perm.notifications.use", "Reminders that this app schedules on this device.")
        case .speechAndMicrophone:
            return ConsentL10n.text("perm.speech.use", "The microphone and speech recognition, only while you hold the dictate button.")
        case .photos:
            return ConsentL10n.text("perm.photos.use", "Only the photo you pick to attach to a note.")
        case .healthKit:
            return ConsentL10n.text("perm.health.use", "State of Mind entries in the Health app.")
        case .ageRange:
            return ConsentL10n.text("perm.age.use", "The age range you declared to Apple, if you choose to share it.")
        }
    }

    public var why: String {
        switch self {
        case .camera:
            return ConsentL10n.text("perm.camera.why", "To measure where your child looks on the screen, the head position and facial expressions.")
        case .motion:
            return ConsentL10n.text("perm.motion.why", "To tell movement of the device apart from your child's head movement.")
        case .notifications:
            return ConsentL10n.text("perm.notifications.why", "To remind you at the times you choose.")
        case .speechAndMicrophone:
            return ConsentL10n.text("perm.speech.why", "So you can speak a note instead of typing it.")
        case .photos:
            return ConsentL10n.text("perm.photos.why", "So a note can include a picture you choose.")
        case .healthKit:
            return ConsentL10n.text("perm.health.why", "So mood entries match between this app and the Health app.")
        case .ageRange:
            return ConsentL10n.text("perm.age.why", "So the app can follow Apple's age rules for this account.")
        }
    }

    public var neverStored: String {
        switch self {
        case .camera:
            return ConsentL10n.text("perm.camera.never", "No photos, no video. Camera frames are processed in memory and thrown away.")
        case .motion:
            return ConsentL10n.text("perm.motion.never", "No location and no movement history outside a session.")
        case .notifications:
            return ConsentL10n.text("perm.notifications.never", "No session data is put in a reminder.")
        case .speechAndMicrophone:
            return ConsentL10n.text("perm.speech.never", "No audio recording is saved. Only the text you dictate is kept.")
        case .photos:
            return ConsentL10n.text("perm.photos.never", "The rest of your photo library is never read.")
        case .healthKit:
            return ConsentL10n.text("perm.health.never", "No other Health data is read, and none is sent anywhere.")
        case .ageRange:
            return ConsentL10n.text("perm.age.never", "No birth date and no ID document.")
        }
    }

    /// Extra note for permissions iOS does not prompt for, or cannot report.
    public var statusNote: String? {
        switch self {
        case .motion:
            return ConsentL10n.text("perm.motion.note", "iOS does not ask for permission for device motion. It is used only while a session runs.")
        case .ageRange:
            return ConsentL10n.text("perm.age.note", "iOS asks each time the app requests it, so the current choice cannot be shown here.")
        default:
            return nil
        }
    }

    public var howToChangeLater: String {
        ConsentL10n.text("perm.changelater", "You can change this any time in iOS Settings > EarlyGaze.")
    }

    /// Reads the current state from the owning framework. Never prompts.
    @MainActor
    public func currentStatus() async -> PermissionStatus {
        switch self {
        case .camera:
            switch AVCaptureDevice.authorizationStatus(for: .video) {
            case .notDetermined: return .notDetermined
            case .authorized: return .granted
            case .denied: return .denied
            case .restricted: return .restricted
            @unknown default: return .denied
            }

        case .motion:
            #if canImport(CoreMotion)
            return CMMotionManager().isDeviceMotionAvailable ? .noPromptNeeded : .unavailable
            #else
            return .unavailable
            #endif

        case .notifications:
            #if canImport(UserNotifications)
            let settings = await UNUserNotificationCenter.current().notificationSettings()
            switch settings.authorizationStatus {
            case .notDetermined: return .notDetermined
            case .authorized, .provisional, .ephemeral: return .granted
            case .denied: return .denied
            @unknown default: return .denied
            }
            #else
            return .unavailable
            #endif

        case .speechAndMicrophone:
            return Self.speechAndMicrophoneStatus()

        case .photos:
            #if canImport(Photos)
            switch PHPhotoLibrary.authorizationStatus(for: .readWrite) {
            case .notDetermined: return .notDetermined
            case .authorized: return .granted
            case .limited: return .limited
            case .denied: return .denied
            case .restricted: return .restricted
            @unknown default: return .denied
            }
            #else
            return .unavailable
            #endif

        case .healthKit:
            #if canImport(HealthKit)
            guard HKHealthStore.isHealthDataAvailable() else { return .unavailable }
            // Only the write status is visible to apps; read access is never reported by iOS.
            switch HKHealthStore().authorizationStatus(for: HKStateOfMindType.stateOfMindType()) {
            case .notDetermined: return .notDetermined
            case .sharingAuthorized: return .granted
            case .sharingDenied: return .denied
            @unknown default: return .denied
            }
            #else
            return .unavailable
            #endif

        case .ageRange:
            return .onRequest
        }
    }

    @MainActor
    private static func speechAndMicrophoneStatus() -> PermissionStatus {
        #if canImport(Speech)
        let speech: PermissionStatus
        switch SFSpeechRecognizer.authorizationStatus() {
        case .notDetermined: speech = .notDetermined
        case .authorized: speech = .granted
        case .denied: speech = .denied
        case .restricted: speech = .restricted
        @unknown default: speech = .denied
        }
        #else
        let speech = PermissionStatus.unavailable
        #endif

        let microphone: PermissionStatus
        switch AVAudioApplication.shared.recordPermission {
        case .undetermined: microphone = .notDetermined
        case .granted: microphone = .granted
        case .denied: microphone = .denied
        @unknown default: microphone = .denied
        }

        // Both are needed for dictation; report the more limiting of the two.
        for status in [PermissionStatus.restricted, .denied, .unavailable, .notDetermined]
        where speech == status || microphone == status {
            return status
        }
        return .granted
    }
}
