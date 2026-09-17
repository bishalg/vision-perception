import Foundation

/**
 * Internationalization (i18n) engine for Palm Perception mobile clients.
 * Supports English (en), Nepali (ne), and Hindi (hi).
 *
 * Single source of truth: `apps/mobile_overlay/shared/strings/palm_strings.json`
 * (bundled as a resource via project.yml) — the exact same file the Android
 * app ships in its assets. iOS-only readiness copy is kept below so changes
 * do not alter the shared mobile resource.
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

    /// iOS-only copy, using the same key/language lookup and English fallback.
    private static let appTable: [String: [String: String]] = [
        "safety.title": [
            "en": "For entertainment and cultural exploration",
            "hi": "मनोरंजन और सांस्कृतिक जानकारी के लिए",
            "ne": "मनोरञ्जन र सांस्कृतिक जानकारीका लागि"
        ],
        "safety.disclaimer": [
            "en": "Palm readings are cultural interpretations, not scientific predictions. They cannot determine health, lifespan, or future events and are not medical advice.",
            "hi": "हस्तरेखा पाठ सांस्कृतिक व्याख्याएँ हैं, वैज्ञानिक भविष्यवाणियाँ नहीं। वे स्वास्थ्य, आयु या भविष्य की घटनाएँ निर्धारित नहीं कर सकते और चिकित्सा सलाह नहीं हैं।",
            "ne": "हस्तरेखा पठन सांस्कृतिक व्याख्या हो, वैज्ञानिक भविष्यवाणी होइन। यसले स्वास्थ्य, आयु वा भविष्यका घटना निर्धारण गर्न सक्दैन र यो चिकित्सकीय सल्लाह होइन।"
        ],
        "safety.privacy": [
            "en": "Privacy Policy",
            "hi": "गोपनीयता नीति",
            "ne": "गोपनीयता नीति"
        ],
        "sheet.emptyTitle": [
            "en": "No reading available",
            "hi": "कोई पाठ उपलब्ध नहीं है",
            "ne": "पठन उपलब्ध छैन"
        ],
        "sheet.emptyBody": [
            "en": "No palm lines are available for a reading. Retake a clear photo, then review and adjust the lines before continuing.",
            "hi": "पाठ के लिए हथेली की रेखाएँ उपलब्ध नहीं हैं। फिर से स्पष्ट फ़ोटो लें, फिर आगे बढ़ने से पहले रेखाओं की जाँच और सुधार करें।",
            "ne": "पठनका लागि हस्तरेखाहरू उपलब्ध छैनन्। फेरि स्पष्ट फोटो खिच्नुहोस्, त्यसपछि अगाडि बढ्नुअघि रेखाहरू जाँचेर मिलाउनुहोस्।"
        ]
    ]

    public static func tr(_ key: String, lang: AppLanguage = currentLanguage) -> String {
        let translations = appTable[key] ?? table[key]
        return translations?[lang.rawValue] ?? translations?["en"] ?? key
    }
}
