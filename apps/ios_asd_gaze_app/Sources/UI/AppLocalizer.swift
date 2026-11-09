import SwiftUI

/// Trilingual localization dictionary supporting English, Nepali, and Hindi.
public enum AppLocale: String, CaseIterable, Identifiable {
    case english = "en"
    case nepali = "ne"
    case hindi = "hi"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .english: return "English"
        case .nepali: return "नेपाली (Nepali)"
        case .hindi: return "हिन्दी (Hindi)"
        }
    }

    /// Automatically detects user's preferred language from iOS Settings.
    public static var current: AppLocale {
        let preferred = Locale.preferredLanguages.first?.prefix(2).lowercased() ?? Locale.current.language.languageCode?.identifier.prefix(2).lowercased() ?? "en"
        if preferred.hasPrefix("ne") {
            return .nepali
        } else if preferred.hasPrefix("hi") {
            return .hindi
        }
        return .english
    }
}

public enum Localizer {
    /// Strings live in `Resources/Localizable.xcstrings` (WP-LN.1, i18n-readiness I-2). hi and ne
    /// values are machine drafts (`needs_review`) until native review (H-10).
    private static let missing = "\u{1}__asd_missing__"
    private static let table = "Localizable"

    private static func lookup(_ key: String, in lang: AppLocale) -> String? {
        guard let path = Bundle.main.path(forResource: lang.rawValue, ofType: "lproj"),
              let bundle = Bundle(path: path) else { return nil }
        let value = bundle.localizedString(forKey: key, value: missing, table: table)
        return value == missing ? nil : value
    }

    /// Translates `key` for `lang` (default: the device's preferred language). Fallback order:
    /// requested language, then English, then the key itself.
    public static func tr(_ key: String, lang: AppLocale? = nil) -> String {
        let active = lang ?? AppLocale.current
        return lookup(key, in: active) ?? lookup(key, in: .english) ?? key
    }
}
