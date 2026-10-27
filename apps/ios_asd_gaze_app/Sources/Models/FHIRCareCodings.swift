import Foundation

/// Codes used by `FHIRCareMapper` (FHIR-5, ADR-0003 decision 6).
///
/// HL7 terminology is used where a real code exists (observation category
/// `survey`, MedicationStatement category `patientspecified`, UCUM units).
/// Everything else is a caregiver-entered observation with no standard concept,
/// so it uses a local CodeSystem under `namespaceBase`. No SNOMED finding codes
/// are used: these records are caregiver notes, not findings (ADR-0003 decision 3).
///
/// Kept separate from `ClinicalInteroperabilityStandards.swift` so this package
/// does not touch the gaze mapper's codes.
public enum FHIRCareCodings {

    /// A single coding: `system`, `code`, `display`.
    public struct Coding: Sendable, Equatable {
        public let system: String
        public let code: String
        public let display: String

        public var fhir: [String: AnyCodableValue] {
            ["system": .string(system), "code": .string(code), "display": .string(display)]
        }
    }

    /// Base of the local CodeSystems. Same default as ADR-0003 decision 7
    /// (`FHIRNamespaces.local`, owned by WP-3.1); should be pointed at that constant
    /// once both packages are integrated so the base is changed in one place.
    public static let namespaceBase = "http://curv.org/fhir"

    // MARK: - HL7 / UCUM systems

    public static let observationCategorySystem = "http://terminology.hl7.org/CodeSystem/observation-category"
    public static let medicationStatementCategorySystem =
        "http://terminology.hl7.org/CodeSystem/medication-statement-category"
    public static let ucumSystem = "http://unitsofmeasure.org"

    public static let surveyCategory = Coding(system: observationCategorySystem, code: "survey", display: "Survey")
    public static let patientSpecifiedCategory = Coding(
        system: medicationStatementCategorySystem,
        code: "patientspecified",
        display: "Patient Specified"
    )

    // MARK: - Local systems

    public static let careObservationSystem = "\(namespaceBase)/CodeSystem/care-observation"
    public static let moodSystem = "\(namespaceBase)/CodeSystem/caregiver-mood"
    public static let moodContextSystem = "\(namespaceBase)/CodeSystem/mood-context"
    public static let socialRecognitionSystem = "\(namespaceBase)/CodeSystem/social-recognition"
    public static let diaryMethodSystem = "\(namespaceBase)/CodeSystem/diary-entry-method"
    public static let milestoneStatusSystem = "\(namespaceBase)/CodeSystem/milestone-status"
    public static let developmentalDomainSystem = "\(namespaceBase)/CodeSystem/developmental-domain"

    // MARK: - Observation and component codes (care-observation)

    public static let moodDiaryEntry = care("mood-diary-entry", "Mood diary entry")
    public static let developmentalMilestone = care("developmental-milestone", "Developmental milestone")
    public static let moodIntensity = care("mood-intensity", "Mood intensity (0 to 1)")
    public static let moodDuration = care("mood-duration", "Mood duration")
    public static let socialRecognition = care("social-recognition", "Social recognition")
    public static let moodContext = care("mood-context", "Context")
    public static let developmentalDomain = care("developmental-domain", "Developmental domain")
    public static let milestoneAge = care("milestone-age", "Milestone age window")
    public static let milestoneChecklistItem = care("milestone-checklist-item", "Milestone checklist item")

    private static func care(_ code: String, _ display: String) -> Coding {
        Coding(system: careObservationSystem, code: code, display: display)
    }

    // MARK: - Value sets

    public static func moodCoding(_ emotion: PediatricEmotion) -> Coding {
        Coding(system: moodSystem, code: emotion.rawValue, display: emotion.title)
    }

    public static func moodContextCoding(_ tag: MoodContextTag) -> Coding {
        let code: String
        switch tag {
        case .postSchool: code = "post-school"
        case .transitionPeriod: code = "transition-period"
        case .loudEnvironment: code = "loud-environment"
        case .mealtime: code = "mealtime"
        case .bedtime: code = "bedtime"
        case .therapy: code = "therapy-session"
        case .familyVisit: code = "family-visit"
        case .screens: code = "screens"
        }
        return Coding(system: moodContextSystem, code: code, display: tag.rawValue)
    }

    public static func socialRecognitionCoding(_ recognition: SocialRecognition) -> Coding {
        let code: String
        switch recognition {
        case .respondedToName: code = "responded-to-name"
        case .recognizedFamiliarPerson: code = "recognized-familiar-person"
        case .sharedAttention: code = "shared-attention"
        case .notObserved: code = "not-observed"
        }
        return Coding(system: socialRecognitionSystem, code: code, display: recognition.title)
    }

    public static func diaryMethodCoding(_ source: MoodDiarySource) -> Coding {
        switch source {
        case .manual:
            return Coding(system: diaryMethodSystem, code: "caregiver-entered", display: "Entered by caregiver")
        case .arkitSuggested:
            return Coding(
                system: diaryMethodSystem,
                code: "device-suggested",
                display: "Suggested by the app, confirmed by caregiver"
            )
        }
    }

    public static func milestoneStatusCoding(achieved: Bool) -> Coding {
        achieved
            ? Coding(system: milestoneStatusSystem, code: "achieved", display: "Achieved")
            : Coding(system: milestoneStatusSystem, code: "not-yet", display: "Not yet")
    }

    public static func developmentalDomainCoding(_ domain: DevelopmentalDomain) -> Coding {
        let code: String
        switch domain {
        case .socialEmotional: code = "social-emotional"
        case .languageCommunication: code = "language-communication"
        case .cognitive: code = "cognitive"
        case .sensoryMotor: code = "sensory-motor"
        }
        return Coding(system: developmentalDomainSystem, code: code, display: domain.rawValue)
    }
}
