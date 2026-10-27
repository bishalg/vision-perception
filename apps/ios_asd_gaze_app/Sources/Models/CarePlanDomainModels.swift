import CurvAsdGazeKit
import Foundation
import SwiftUI

// MARK: - Clinical Interoperability (STAGE 1: Semantic Metadata Decoration)

/// Categories of developmental care plan interventions.
public enum CareTaskCategory: String, Codable, CaseIterable, Sendable {
    case observation = "Gaze Screening"
    case attention = "Joint Attention"
    case language = "Name & Social"
    case play = "Reciprocal Play"

    public var systemIcon: String {
        switch self {
        case .observation: return "eyes"
        case .attention: return "scope"
        case .language: return "bubble.left.and.exclamationmark.bubble.right"
        case .play: return "figure.2.and.child.holdinghands"
        }
    }

    /// Standard SNOMED CT concept for this care task category.
    public var snomedConcept: ClinicalCoding {
        switch self {
        case .observation: return StandardCodingSystems.SNOMED.eyeTrackingProcedure
        case .attention: return StandardCodingSystems.SNOMED.impairedJointAttention
        case .language: return StandardCodingSystems.SNOMED.delayedLanguageMilestone
        case .play: return StandardCodingSystems.SNOMED.stereotypicMovement
        }
    }
}

/// A single daily intervention task in the CareKit protocol.
public struct CarePlanTask: Identifiable, Codable, Sendable, Equatable {
    public let id: String
    public var title: String
    public var subtitle: String
    public var icon: String
    public var category: CareTaskCategory
    public var isCompleted: Bool
    public var completedAt: Date?

    /// Standard SNOMED CT concept for the behavioral finding this task addresses.
    /// Enables FHIR/ObsRO mapping and OMOP CDM OBSERVATION table population.
    public var snomedFinding: ClinicalCoding {
        category.snomedConcept
    }

    /// Standard LOINC code for outcome measurement (if applicable).
    /// Populated for tasks that produce quantifiable observations.
    public var loincOutcome: ClinicalCoding? {
        switch category {
        case .observation: return StandardCodingSystems.LOINC.gazeTrackingPanel
        case .attention: return StandardCodingSystems.LOINC.visualReactionTime
        case .language: return nil
        case .play: return nil
        }
    }

    public init(
        id: String,
        title: String,
        subtitle: String,
        icon: String,
        category: CareTaskCategory,
        isCompleted: Bool = false,
        completedAt: Date? = nil
    ) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.icon = icon
        self.category = category
        self.isCompleted = isCompleted
        self.completedAt = completedAt
    }
}

/// Caregiver mood and engagement rating for the child.
public enum ChildMood: String, Codable, CaseIterable, Identifiable, Sendable {
    case engaged = "Engaged"
    case calm = "Calm"
    case distracted = "Distracted"
    case overwhelmed = "Overwhelmed"

    public var id: String { rawValue }

    public var emoji: String {
        switch self {
        case .engaged: return "😊"
        case .calm: return "🙂"
        case .distracted: return "😐"
        case .overwhelmed: return "😫"
        }
    }

    public var localizedLabel: String {
        switch self {
        case .engaged: return "Very Engaged"
        case .calm: return "Calm & Content"
        case .distracted: return "Distracted"
        case .overwhelmed: return "Sensory Sensitive"
        }
    }

    /// Standard SNOMED CT concept for this mood/affect observation.
    /// Maps to ObsRO entries in OMOP CDM OBSERVATION table.
    public var snomedAffect: ClinicalCoding {
        switch self {
        case .engaged: return StandardCodingSystems.SNOMED.flatAffect // inverted for positive
        case .calm: return StandardCodingSystems.SNOMED.flatAffect
        case .distracted: return StandardCodingSystems.SNOMED.impairedJointAttention
        case .overwhelmed: return StandardCodingSystems.SNOMED.stereotypicMovement
        }
    }

    /// LOINC code for affect assessment observation.
    public var loincAffect: ClinicalCoding {
        StandardCodingSystems.LOINC.behavioralAffectAssessment
    }
}

/// Daily sensory and observation diary entry (one per calendar day).
public struct DailyDiaryEntry: Identifiable, Codable, Sendable, Equatable {
    public var id: UUID
    public var date: Date
    public var mood: ChildMood
    public var notes: String
    public var eyeContactRating: Int // 1 to 5 scale
    public var jointAttentionCount: Int
    /// Caregiver-rated sensory load (1 calm … 5 overloaded).
    public var sensoryLoad: Int
    public var lastEditedAt: Date?

    /// True when the caregiver has added notes or changed defaults for the day.
    public var hasCaregiverInput: Bool {
        lastEditedAt != nil || !notes.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// Standard SNOMED CT affect concept for the logged mood (ObsRO).
    public var snomedAffect: ClinicalCoding { mood.snomedAffect }

    /// Standard LOINC affect assessment code.
    public var loincAffect: ClinicalCoding { mood.loincAffect }

    /// LOINC code for eye contact rating (qualitative).
    public var loincEyeContact: ClinicalCoding {
        StandardCodingSystems.LOINC.visualReactionTime
    }

    public init(
        id: UUID = UUID(),
        date: Date = Date(),
        mood: ChildMood = .calm,
        notes: String = "",
        eyeContactRating: Int = 3,
        jointAttentionCount: Int = 2,
        sensoryLoad: Int = 3,
        lastEditedAt: Date? = nil
    ) {
        self.id = id
        self.date = date
        self.mood = mood
        self.notes = notes
        self.eyeContactRating = max(1, min(5, eyeContactRating))
        self.jointAttentionCount = max(0, jointAttentionCount)
        self.sensoryLoad = max(1, min(5, sensoryLoad))
        self.lastEditedAt = lastEditedAt
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            id: (try? container.decode(UUID.self, forKey: .id)) ?? UUID(),
            date: (try? container.decode(Date.self, forKey: .date)) ?? Date(),
            mood: (try? container.decode(ChildMood.self, forKey: .mood)) ?? .calm,
            notes: (try? container.decode(String.self, forKey: .notes)) ?? "",
            eyeContactRating: (try? container.decode(Int.self, forKey: .eyeContactRating)) ?? 3,
            jointAttentionCount: (try? container.decode(Int.self, forKey: .jointAttentionCount)) ?? 2,
            sensoryLoad: (try? container.decode(Int.self, forKey: .sensoryLoad)) ?? 3,
            lastEditedAt: try? container.decode(Date.self, forKey: .lastEditedAt)
        )
    }
}

/// Caregiver-logged daily medication / supplement schedule, tracked CareKit-style:
/// one multi-dose daily task per item, each scheduled dose time toggleable for today.
/// The presence of an item is caregiver-entered, not a prescription.
public struct MedicationItem: Identifiable, Codable, Sendable, Equatable {
    public let id: String
    public var name: String
    public var dose: String
    public var instructions: String
    /// "HH:MM" dose times, e.g. ["08:00", "20:00"].
    public var schedule: [String]
    /// Per-dose completion for today, aligned with `schedule`.
    public var dosesTakenToday: [Bool]
    public var lastTakenAt: Date?
    public var reminderSlots: [CareReminderSlot]
    public var linkedConditionId: String?
    public var notes: String

    public init(
        id: String,
        name: String,
        dose: String,
        instructions: String,
        schedule: [String],
        dosesTakenToday: [Bool] = [],
        lastTakenAt: Date? = nil,
        reminderSlots: [CareReminderSlot] = [],
        linkedConditionId: String? = nil,
        notes: String = ""
    ) {
        self.id = id
        self.name = name
        self.dose = dose
        self.instructions = instructions
        self.schedule = schedule
        self.dosesTakenToday = Self.alignedDoses(dosesTakenToday, to: schedule)
        self.lastTakenAt = lastTakenAt
        self.reminderSlots = reminderSlots.synced(to: schedule)
        self.linkedConditionId = linkedConditionId
        self.notes = notes
    }

    /// Decode leniently: persisted `dosesTakenToday` from an older schedule
    /// shape is resized (prefix preserved, remainder false) instead of
    /// silently disabling doses.
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let schedule = try container.decode([String].self, forKey: .schedule)
        self.init(
            id: try container.decode(String.self, forKey: .id),
            name: try container.decode(String.self, forKey: .name),
            dose: try container.decode(String.self, forKey: .dose),
            instructions: try container.decode(String.self, forKey: .instructions),
            schedule: schedule,
            dosesTakenToday: (try? container.decode([Bool].self, forKey: .dosesTakenToday)) ?? [],
            lastTakenAt: try? container.decode(Date.self, forKey: .lastTakenAt),
            reminderSlots: (try? container.decode([CareReminderSlot].self, forKey: .reminderSlots)) ?? [],
            linkedConditionId: try? container.decode(String.self, forKey: .linkedConditionId),
            notes: (try? container.decode(String.self, forKey: .notes)) ?? ""
        )
    }

    public mutating func setSchedule(_ times: [String]) {
        schedule = times
        dosesTakenToday = Self.alignedDoses(dosesTakenToday, to: times)
        reminderSlots = reminderSlots.synced(to: times)
    }

    /// Resize a taken-array to the schedule length (prefix preserved).
    private static func alignedDoses(_ doses: [Bool], to schedule: [String]) -> [Bool] {
        if doses.count == schedule.count { return doses }
        var fixed = Array(doses.prefix(schedule.count))
        while fixed.count < schedule.count { fixed.append(false) }
        return fixed
    }
}

/// Longitudinal social visual preference data point.
public struct LongitudinalAttentionPoint: Identifiable, Codable, Sendable {
    public let id: String
    public let dayLabel: String
    public let socialPreferenceScore: Double // -1.0 to +1.0
    public let validFramePercentage: Double
    public let sessionCount: Int

    public init(
        id: String,
        dayLabel: String,
        socialPreferenceScore: Double,
        validFramePercentage: Double,
        sessionCount: Int
    ) {
        self.id = id
        self.dayLabel = dayLabel
        self.socialPreferenceScore = socialPreferenceScore
        self.validFramePercentage = validFramePercentage
        self.sessionCount = sessionCount
    }
}
