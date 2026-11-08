import Foundation

/// The App Store build (`ASD_STORE_BUILD`, Release only) is a clinic observation capture tool:
/// it records a session and keeps or delivers it for the clinic to review, and it shows no
/// score, band, flag or report on the device (store-collection-build.md S-1 .. S-9).
/// Debug and DemoRelease never define the flag and keep their result screens.
public enum StoreBuild {
    /// `true` only in the App Store (Release) configuration.
    public static var isStoreBuild: Bool {
        #if ASD_STORE_BUILD
        return true
        #else
        return false
        #endif
    }

    /// Info.plist keys for the legal links (S-9). Same URLs as the store listing.
    public static let privacyPolicyURLKey = "ASDPrivacyPolicyURL"
    public static let supportURLKey = "ASDSupportURL"

    /// Mandatory sentences wherever a session or the app is described (S-8, ADR-0004).
    public static let disclaimer = "This is not a diagnostic tool. It does not measure, screen for or diagnose any condition; your clinic reviews each session."

    /// Shown while no clinic upload is configured (S-4: the default store build is local-only).
    public static let localOnlyNotice = "Sessions are stored on this device until your clinic enrols it."

    /// Privacy policy link from the bundle; `nil` if the key is missing or not https.
    public static func privacyPolicyURL(bundle: Bundle = .main) -> URL? {
        httpsURL(bundle.object(forInfoDictionaryKey: privacyPolicyURLKey))
    }

    /// Support link from the bundle; `nil` if the key is missing or not https.
    public static func supportURL(bundle: Bundle = .main) -> URL? {
        httpsURL(bundle.object(forInfoDictionaryKey: supportURLKey))
    }

    /// The name the user sees on the home screen (S-7), never a hard-coded brand.
    public static func displayName(bundle: Bundle = .main) -> String {
        for key in ["CFBundleDisplayName", "CFBundleName"] {
            if let name = bundle.object(forInfoDictionaryKey: key) as? String, !name.isEmpty {
                return name
            }
        }
        return "EarlyGaze"
    }

    static func httpsURL(_ value: Any?) -> URL? {
        guard let text = value as? String,
              let url = URL(string: text.trimmingCharacters(in: .whitespacesAndNewlines)),
              url.scheme?.lowercased() == "https",
              url.host?.isEmpty == false
        else { return nil }
        return url
    }
}

/// Root tabs of the app. The store build has no dashboard (S-2): every verdict surface lives
/// there. The Sessions tab exists only in the store build.
public enum RootTab: Int, CaseIterable, Hashable, Sendable {
    case dashboard
    case capture
    case sessions
    case settings

    public static func visibleTabs(storeBuild: Bool = StoreBuild.isStoreBuild) -> [RootTab] {
        storeBuild ? [.capture, .sessions, .settings] : [.dashboard, .capture, .settings]
    }
}
