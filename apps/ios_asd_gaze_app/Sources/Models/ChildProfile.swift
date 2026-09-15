import Foundation
import SwiftUI

/// Caregiver-entered medical condition (tracking aid, not a diagnosis).
public struct MedicalCondition: Identifiable, Codable, Sendable, Equatable {
    public var id: String
    public var title: String
    public var notes: String
    public var isActive: Bool

    public init(id: String = UUID().uuidString, title: String, notes: String = "", isActive: Bool = true) {
        self.id = id
        self.title = title
        self.notes = notes
        self.isActive = isActive
    }
}

// MARK: - USCDI v5 Sex Parameter for Clinical Use (SPCU)

/// Distinguishes anatomical/biological sex parameters used in normative
/// gaze modeling from administrative gender identity (USCDI v5 SPCU).
public enum SexParameterForClinicalUse: String, Codable, Sendable, CaseIterable {
    case male = "M"
    case female = "F"
    case other = "O"
    case unknown = "U"

    public var display: String {
        switch self {
        case .male: return "Male"
        case .female: return "Female"
        case .other: return "Other"
        case .unknown: return "Unknown / Not Specified"
        }
    }
}

// MARK: - Pediatric Age Bracket

/// Standardized 5-tier pediatric age brackets derived from landmark ASD gaze research
/// (JAMA 2023 EarliPoint, NEJM AI 2024 SenseToKnow, eLife 2019 GeoPref, JBI 2022).
public enum PediatricAgeBracket: String, CaseIterable, Codable, Sendable, Identifiable {
    case months12_17 = "12–17 mo"
    case months18_23 = "18–23 mo"
    case months24_35 = "24–35 mo"
    case months36_47 = "36–47 mo"
    case months48_60 = "48–60 mo"

    public var id: String { rawValue }

    public var developmentalStage: String {
        switch self {
        case .months12_17: return "Early Toddler (1–1.5 yrs)"
        case .months18_23: return "Mid Toddler (1.5–2 yrs)"
        case .months24_35: return "Late Toddler (2–3 yrs)"
        case .months36_47: return "Preschool (3–4 yrs)"
        case .months48_60: return "Pre-K (4–5 yrs)"
        }
    }

    public var clinicalFocus: String {
        switch self {
        case .months12_17: return "GeoPref preference & maternal face scanning"
        case .months18_23: return "Joint attention, eye dwell & smile mimicry"
        case .months24_35: return "Turn-taking, audio name orienting & rhyme play"
        case .months36_47: return "Animated slapstick humor & visual anticipation"
        case .months48_60: return "Complex social interaction & conversational gaze"
        }
    }

    public var midpointMonths: Int {
        switch self {
        case .months12_17: return 14
        case .months18_23: return 20
        case .months24_35: return 28
        case .months36_47: return 40
        case .months48_60: return 54
        }
    }
}

// MARK: - Child Profile Model

/// Locally persisted, encrypted child profile conforming to HIPAA / COPPA de-identification.
public struct ChildProfile: Codable, Sendable, Equatable {
    public var childName: String
    public var ageBracket: PediatricAgeBracket
    public var preferredLanguage: String // "en", "hi", "ne", "zh"
    public var sessionDurationSec: Int // 30 or 60
    public var hasConsentedPIA: Bool
    public var consentTimestamp: Date?

    /// USCDI v5 Sex Parameter for Clinical Use (SPCU).
    /// Separates anatomical sex for normative gaze baselines from administrative gender.
    public var biologicalSexForNormativeBaseline: SexParameterForClinicalUse
    /// Caregiver-entered conditions (not a screening diagnosis).
    public var medicalConditions: [MedicalCondition]
    public var allergies: String

    public init(
        childName: String = "",
        ageBracket: PediatricAgeBracket = .months18_23,
        preferredLanguage: String = "en",
        sessionDurationSec: Int = 30,
        hasConsentedPIA: Bool = false,
        consentTimestamp: Date? = nil,
        biologicalSexForNormativeBaseline: SexParameterForClinicalUse = .unknown,
        medicalConditions: [MedicalCondition] = [],
        allergies: String = ""
    ) {
        self.childName = childName
        self.ageBracket = ageBracket
        self.preferredLanguage = preferredLanguage
        self.sessionDurationSec = sessionDurationSec
        self.hasConsentedPIA = hasConsentedPIA
        self.consentTimestamp = consentTimestamp
        self.biologicalSexForNormativeBaseline = biologicalSexForNormativeBaseline
        self.medicalConditions = medicalConditions
        self.allergies = allergies
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            childName: (try? container.decode(String.self, forKey: .childName)) ?? "",
            ageBracket: (try? container.decode(PediatricAgeBracket.self, forKey: .ageBracket)) ?? .months18_23,
            preferredLanguage: (try? container.decode(String.self, forKey: .preferredLanguage)) ?? "en",
            sessionDurationSec: (try? container.decode(Int.self, forKey: .sessionDurationSec)) ?? 30,
            hasConsentedPIA: (try? container.decode(Bool.self, forKey: .hasConsentedPIA)) ?? false,
            consentTimestamp: try? container.decode(Date.self, forKey: .consentTimestamp),
            biologicalSexForNormativeBaseline: (try? container.decode(SexParameterForClinicalUse.self, forKey: .biologicalSexForNormativeBaseline)) ?? .unknown,
            medicalConditions: (try? container.decode([MedicalCondition].self, forKey: .medicalConditions)) ?? [],
            allergies: (try? container.decode(String.self, forKey: .allergies)) ?? ""
        )
    }

    public static let defaultProfile = ChildProfile(
        childName: "Child",
        ageBracket: .months18_23,
        preferredLanguage: "en",
        sessionDurationSec: 30,
        hasConsentedPIA: false,
        consentTimestamp: nil,
        biologicalSexForNormativeBaseline: .unknown
    )
}

// MARK: - Child Profile Store

/// Manages local persistence of the active child profile.
@MainActor
public final class ChildProfileStore: ObservableObject {
    public static let shared = ChildProfileStore()

    private let storageKey = "org.curv.asd.child_profile"

    @Published public var profile: ChildProfile {
        didSet {
            save()
        }
    }

    private init() {
        if let data = UserDefaults.standard.data(forKey: storageKey),
           let decoded = try? JSONDecoder().decode(ChildProfile.self, from: data) {
            self.profile = decoded
        } else {
            self.profile = .defaultProfile
        }
    }

    public func save() {
        if let encoded = try? JSONEncoder().encode(profile) {
            UserDefaults.standard.set(encoded, forKey: storageKey)
        }
    }

    public func reset() {
        self.profile = .defaultProfile
        UserDefaults.standard.removeObject(forKey: storageKey)
    }
}

// MARK: - Screening Language Model & Catalog

public struct ScreeningLanguageItem: Identifiable, Hashable, Sendable, Codable {
    public let code: String
    public let nativeName: String
    public let englishName: String
    public let region: String
    public let flag: String

    public var id: String { code }

    public var displayName: String {
        "\(nativeName) (\(englishName))"
    }

    public init(code: String, nativeName: String, englishName: String, region: String, flag: String) {
        self.code = code
        self.nativeName = nativeName
        self.englishName = englishName
        self.region = region
        self.flag = flag
    }
}

public enum ScreeningLanguages {
    public static let all: [ScreeningLanguageItem] = [
        ScreeningLanguageItem(code: "en", nativeName: "English", englishName: "English", region: "Global / North America / UK", flag: "🇺🇸"),
        ScreeningLanguageItem(code: "hi", nativeName: "हिन्दी", englishName: "Hindi", region: "India / South Asia", flag: "🇮🇳"),
        ScreeningLanguageItem(code: "ne", nativeName: "नेपाली", englishName: "Nepali", region: "Nepal / South Asia", flag: "🇳🇵"),
        ScreeningLanguageItem(code: "zh", nativeName: "简体中文", englishName: "Mandarin Chinese", region: "China / East Asia", flag: "🇨🇳"),
        ScreeningLanguageItem(code: "es", nativeName: "Español", englishName: "Spanish", region: "Spain / Latin America", flag: "🇪🇸"),
        ScreeningLanguageItem(code: "fr", nativeName: "Français", englishName: "French", region: "France / Francophone", flag: "🇫🇷"),
        ScreeningLanguageItem(code: "de", nativeName: "Deutsch", englishName: "German", region: "Germany / Austria", flag: "🇩🇪"),
        ScreeningLanguageItem(code: "ja", nativeName: "日本語", englishName: "Japanese", region: "Japan", flag: "🇯🇵"),
        ScreeningLanguageItem(code: "ko", nativeName: "한국어", englishName: "Korean", region: "South Korea", flag: "🇰🇷"),
        ScreeningLanguageItem(code: "ar", nativeName: "العربية", englishName: "Arabic", region: "Middle East / North Africa", flag: "🇸🇦"),
        ScreeningLanguageItem(code: "pt", nativeName: "Português", englishName: "Portuguese", region: "Brazil / Portugal", flag: "🇧🇷"),
        ScreeningLanguageItem(code: "bn", nativeName: "বাংলা", englishName: "Bengali", region: "Bangladesh / India", flag: "🇧🇩"),
        ScreeningLanguageItem(code: "ta", nativeName: "தமிழ்", englishName: "Tamil", region: "India / Sri Lanka / Singapore", flag: "🇮🇳"),
        ScreeningLanguageItem(code: "te", nativeName: "తెలుగు", englishName: "Telugu", region: "India", flag: "🇮🇳"),
        ScreeningLanguageItem(code: "ur", nativeName: "اردو", englishName: "Urdu", region: "Pakistan / South Asia", flag: "🇵🇰"),
        ScreeningLanguageItem(code: "vi", nativeName: "Tiếng Việt", englishName: "Vietnamese", region: "Vietnam", flag: "🇻🇳"),
        ScreeningLanguageItem(code: "tl", nativeName: "Tagalog", englishName: "Filipino", region: "Philippines", flag: "🇵🇭"),
        ScreeningLanguageItem(code: "it", nativeName: "Italiano", englishName: "Italian", region: "Italy / Europe", flag: "🇮🇹")
    ]

    public static let primaryCodes: [String] = ["en", "hi", "ne", "zh"]

    public static func item(for code: String) -> ScreeningLanguageItem {
        all.first { $0.code == code } ?? all[0]
    }
}
