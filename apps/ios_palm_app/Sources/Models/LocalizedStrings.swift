import Foundation

/**
 * Internationalization (i18n) engine for Palm Perception mobile clients.
 * Supports English (en), Nepali (ne), and Hindi (hi).
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

    private static let table: [String: [AppLanguage: String]] = [
        "guidance.searching": [
            .english: "Place your palm inside the frame",
            .nepali: "हात फ्रेम भित्र राख्नुहोस्",
            .hindi: "हथेली को फ्रेम के अंदर रखें"
        ],
        "guidance.center": [
            .english: "Center your palm",
            .nepali: "हातलाई केन्द्रमा राख्नुहोस्",
            .hindi: "हथेली को केंद्र में रखें"
        ],
        "guidance.closer": [
            .english: "Move closer",
            .nepali: "हात नजिक ल्याउनुहोस्",
            .hindi: "हथेली पास लाएं"
        ],
        "guidance.further": [
            .english: "Move further away",
            .nepali: "हात अलि टाढा लैजानुहोस्",
            .hindi: "हथेली थोड़ा दूर ले जाएं"
        ],
        "guidance.flat": [
            .english: "Hold palm flat facing camera",
            .nepali: "हात सीधा क्यामेरा तर्फ देखाउनुहोस्",
            .hindi: "हथेली को कैमरे के सामने सीधा रखें"
        ],
        "guidance.dark": [
            .english: "Too dark — add more light",
            .nepali: "धेरै अँध्यारो छ — थप उज्यालो चाहिन्छ",
            .hindi: "बहुत अंधेरा है — रोशनी बढ़ाएं"
        ],
        "guidance.glare": [
            .english: "Angle palm away from glare",
            .nepali: "सिधा प्रकाशबाट हात अलि ढल्काउनुहोस्",
            .hindi: "सीधी रोशनी से हथेली थोड़ा घुमाएं"
        ],
        "guidance.steady": [
            .english: "Hold steady...",
            .nepali: "हात स्थिर राख्नुहोस्...",
            .hindi: "स्थिर रखें..."
        ],
        "guidance.ready": [
            .english: "Capturing palm lines...",
            .nepali: "हातका रेखाहरू खिचिँदैछ...",
            .hindi: "हथेली की रेखाएं कैप्चर हो रही हैं..."
        ],
        "line.heart": [
            .english: "Heart Line",
            .nepali: "हृदय रेखा",
            .hindi: "हृदय रेखा"
        ],
        "line.head": [
            .english: "Head Line",
            .nepali: "मस्तिष्क रेखा",
            .hindi: "मस्तिष्क रेखा"
        ],
        "line.life": [
            .english: "Life Line",
            .nepali: "जीवन रेखा",
            .hindi: "जीवन रेखा"
        ],
        "line.fate": [
            .english: "Fate Line",
            .nepali: "भाग्य रेखा",
            .hindi: "भाग्य रेखा"
        ],
        "button.capture": [
            .english: "Capture",
            .nepali: "खिच्नुहोस्",
            .hindi: "कैप्चर"
        ],
        "button.retake": [
            .english: "Retake",
            .nepali: "पुनः खिच्नुहोस्",
            .hindi: "फिर से लें"
        ],
        "sheet.title": [
            .english: "Samudrika Palm Analysis",
            .nepali: "सामुद्रिक हस्तरेखा विश्लेषण",
            .hindi: "सामुद्रिक हस्तरेखा विश्लेषण"
        ],
        "status.processing": [
            .english: "Mapping palm lines...",
            .nepali: "हातका रेखाहरू म्याप हुँदैछ...",
            .hindi: "हथेली की रेखाएं मैप हो रही हैं..."
        ],
        "result.title": [
            .english: "Palm Line Overlay",
            .nepali: "हस्तरेखा ओभरले",
            .hindi: "हस्तरेखा ओवरले"
        ],
        "result.landmark": [
            .english: "Landmark-guided crease overlay on your capture",
            .nepali: "तपाईंको फोटोमा ल्यान्डमार्क-आधारित रेखा ओभरले",
            .hindi: "आपकी फोटो पर लैंडमार्क-आधारित रेखा ओवरले"
        ],
        "result.native": [
            .english: "Native CurvEngine metrics + crease overlay",
            .nepali: "नेटिभ CurvEngine मेट्रिक्स + रेखा ओभरले",
            .hindi: "नेटिव CurvEngine मेट्रिक्स + रेखा ओवरले"
        ]
    ]

    public static func tr(_ key: String, lang: AppLanguage = currentLanguage) -> String {
        return table[key]?[lang] ?? table[key]?[.english] ?? key
    }
}
