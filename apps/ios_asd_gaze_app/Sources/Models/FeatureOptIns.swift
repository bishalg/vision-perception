import Foundation

// MARK: - Per-feature opt-ins (store-collection-build.md S-12)

/// Every optional feature. All are off until the guardian turns them on; the camera
/// observation itself, including facial-expression measurements (always recorded by the
/// kit, S-12 as amended 2026-11-08), is covered by the consent document and is not listed here.
public enum FeatureOptIn: String, CaseIterable, Identifiable, Sendable {
    case ageRangeCheck
    case clinicDelivery
    case reminders
    case healthKitMood
    case dictation
    case photoAttachments

    public var id: String { rawValue }

    /// UserDefaults key for the persisted Bool.
    public var storageKey: String { "org.curv.asd.optin.\(rawValue)" }

    /// `true` when the feature itself checks `FeatureOptIns` before it runs (S-15). Every
    /// feature is gated at its entry point and in the service behind it, in every build.
    public var isGated: Bool { true }

    /// The opt-ins whose feature exists in this build (S-16). The store build has no diary,
    /// care alerts or Health mood surface, so it shows only the age-range check and clinic
    /// delivery; the stored flags of the hidden features stay `false`.
    public static func available(storeBuild: Bool = StoreBuild.isStoreBuild) -> [FeatureOptIn] {
        storeBuild ? [.ageRangeCheck, .clinicDelivery] : allCases
    }

    public var title: String {
        switch self {
        case .ageRangeCheck: return ConsentL10n.text("optin.age.title", "Age range check")
        case .clinicDelivery: return ConsentL10n.text("optin.delivery.title", "Clinic delivery")
        case .reminders: return ConsentL10n.text("optin.reminders.title", "Reminders")
        case .healthKitMood: return ConsentL10n.text("optin.health.title", "Mood in the Health app")
        case .dictation: return ConsentL10n.text("optin.dictation.title", "Dictation for notes")
        case .photoAttachments: return ConsentL10n.text("optin.photos.title", "Photo attachments")
        }
    }

    public var systemImage: String {
        switch self {
        case .ageRangeCheck: return "person.badge.shield.checkmark"
        case .clinicDelivery: return "paperplane"
        case .reminders: return "bell"
        case .healthKitMood: return "heart.text.square"
        case .dictation: return "mic"
        case .photoAttachments: return "photo"
        }
    }

    public var oneLineWhatItDoes: String {
        switch self {
        case .ageRangeCheck:
            return ConsentL10n.text("optin.age.what", "Asks iOS for the age range you declared, during setup.")
        case .clinicDelivery:
            return ConsentL10n.text("optin.delivery.what", "Sends sessions to your clinic's console after your clinic enrols this device.")
        case .reminders:
            return ConsentL10n.text("optin.reminders.what", "Sends you reminders on this device.")
        case .healthKitMood:
            return ConsentL10n.text("optin.health.what", "Reads and saves mood entries in the Health app.")
        case .dictation:
            return ConsentL10n.text("optin.dictation.what", "Lets you speak a note instead of typing it.")
        case .photoAttachments:
            return ConsentL10n.text("optin.photos.what", "Lets you attach a photo to a note.")
        }
    }

    /// Row description in Privacy & Your Data.
    public var rowDescription: String { oneLineWhatItDoes }

    /// Extra text on the detail page: what turning the feature off does (S-18).
    public var whenTurnedOff: String? {
        switch self {
        case .clinicDelivery:
            return ConsentL10n.text(
                "optin.delivery.off",
                "Turning this off stops all sending: nothing new is queued and nothing waiting is sent. Sessions already waiting stay on this device and are sent when you turn it back on. A transfer already under way may finish. Sessions recorded while this is off stay on this device only. Delete all data also removes waiting sessions."
            )
        case .ageRangeCheck, .reminders, .healthKitMood, .dictation, .photoAttachments:
            return nil
        }
    }

    public var whatIsStored: String {
        switch self {
        case .ageRangeCheck:
            return ConsentL10n.text("optin.age.stored", "Only whether iOS shared an age range. The answer stays on this device.")
        case .clinicDelivery:
            return ConsentL10n.text("optin.delivery.stored", "A device code from your clinic, kept in the iOS Keychain, and a queue of sessions waiting to be sent.")
        case .reminders:
            return ConsentL10n.text("optin.reminders.stored", "The reminder times you choose, on this device.")
        case .healthKitMood:
            return ConsentL10n.text("optin.health.stored", "Mood entries, in the Health app and in this app on this device.")
        case .dictation:
            return ConsentL10n.text("optin.dictation.stored", "Only the text of your note.")
        case .photoAttachments:
            return ConsentL10n.text("optin.photos.stored", "A copy of the photo you pick, on this device only.")
        }
    }

    public var whatIsNeverStored: String {
        switch self {
        case .ageRangeCheck:
            return ConsentL10n.text("optin.age.never", "No birth date and no ID document.")
        case .clinicDelivery:
            return ConsentL10n.text("optin.delivery.never", "No camera images and no child's name are ever sent.")
        case .reminders:
            return ConsentL10n.text("optin.reminders.never", "Reminders are never sent from a server and contain no session data.")
        case .healthKitMood:
            return ConsentL10n.text("optin.health.never", "Health data never leaves this device through this app.")
        case .dictation:
            return ConsentL10n.text("optin.dictation.never", "No audio recording is saved.")
        case .photoAttachments:
            return ConsentL10n.text("optin.photos.never", "The app cannot see the rest of your photo library.")
        }
    }

    public var requiredPermissions: [PermissionKind] {
        switch self {
        case .ageRangeCheck: return [.ageRange]
        case .clinicDelivery: return []
        case .reminders: return [.notifications]
        case .healthKitMood: return [.healthKit]
        case .dictation: return [.speechAndMicrophone]
        case .photoAttachments: return [.photos]
        }
    }
}

/// The guardian's opt-in choices, persisted in UserDefaults. All default to `false`.
///
/// The features read these flags before they run (S-15): clinic delivery in `ResearchKitSyncService`
/// (through `ClinicDeliveryAuthorization`), reminders in `CareAlertScheduler`, HealthKit in
/// `HealthKitMoodStore`, dictation and photos in `ObservationQuickLogBar`.
@MainActor
public final class FeatureOptIns: ObservableObject {
    public static let shared = FeatureOptIns()

    private let defaults: UserDefaults

    @Published public var ageRangeCheck: Bool { didSet { save(.ageRangeCheck, ageRangeCheck) } }
    @Published public var clinicDelivery: Bool { didSet { save(.clinicDelivery, clinicDelivery) } }
    @Published public var reminders: Bool { didSet { save(.reminders, reminders) } }
    @Published public var healthKitMood: Bool { didSet { save(.healthKitMood, healthKitMood) } }
    @Published public var dictation: Bool { didSet { save(.dictation, dictation) } }
    @Published public var photoAttachments: Bool { didSet { save(.photoAttachments, photoAttachments) } }

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        ageRangeCheck = defaults.bool(forKey: FeatureOptIn.ageRangeCheck.storageKey)
        clinicDelivery = defaults.bool(forKey: FeatureOptIn.clinicDelivery.storageKey)
        reminders = defaults.bool(forKey: FeatureOptIn.reminders.storageKey)
        healthKitMood = defaults.bool(forKey: FeatureOptIn.healthKitMood.storageKey)
        dictation = defaults.bool(forKey: FeatureOptIn.dictation.storageKey)
        photoAttachments = defaults.bool(forKey: FeatureOptIn.photoAttachments.storageKey)
    }

    public func isOn(_ feature: FeatureOptIn) -> Bool {
        switch feature {
        case .ageRangeCheck: return ageRangeCheck
        case .clinicDelivery: return clinicDelivery
        case .reminders: return reminders
        case .healthKitMood: return healthKitMood
        case .dictation: return dictation
        case .photoAttachments: return photoAttachments
        }
    }

    public func set(_ feature: FeatureOptIn, _ value: Bool) {
        switch feature {
        case .ageRangeCheck: ageRangeCheck = value
        case .clinicDelivery: clinicDelivery = value
        case .reminders: reminders = value
        case .healthKitMood: healthKitMood = value
        case .dictation: dictation = value
        case .photoAttachments: photoAttachments = value
        }
    }

    /// Turns every opt-in off (used when consent is withdrawn). Covers the hidden features too,
    /// so a flag left on by an earlier build cannot linger (S-16).
    public func resetAll() {
        for feature in FeatureOptIn.allCases { set(feature, false) }
    }

    private func save(_ feature: FeatureOptIn, _ value: Bool) {
        defaults.set(value, forKey: feature.storageKey)
    }
}
