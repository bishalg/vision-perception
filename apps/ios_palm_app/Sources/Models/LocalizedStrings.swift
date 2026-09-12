import Foundation

/**
 * Internationalization (i18n) engine for Palm Perception mobile clients.
 * Supports English (en), Nepali (ne), and Hindi (hi).
 *
 * Single source of truth: `apps/mobile_overlay/shared/strings/palm_strings.json`
 * (bundled as a resource via project.yml) — the exact same file the Android
 * app ships in its assets. There is no per-app string table; edit the JSON.
 * Lookup falls back requested language → English → key itself.
 */
public enum AppLanguage: String, CaseIterable, Identifiable {
    case english = "en"
    case nepali = "ne"
    case hindi = "hi"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .english: return "English"
        case .nepali: return "नेपाली"
        case .hindi: return "हिन्दी"
        }
    }
}

public struct LocalizedStrings {
    public static var currentLanguage: AppLanguage = .english

    /// Shared strings table, decoded once from the bundled JSON.
    private static let table: [String: [String: String]] = {
        guard let url = Bundle.main.url(forResource: "palm_strings", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let decoded = try? JSONDecoder().decode([String: [String: String]].self, from: data)
        else { return [:] }
        return decoded
    }()

    public static func tr(_ key: String, lang: AppLanguage = currentLanguage) -> String {
        return table[key]?[lang.rawValue] ?? table[key]?["en"] ?? key
    }
}
