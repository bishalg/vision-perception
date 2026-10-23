import Foundation
import SwiftUI

// MARK: - Developmental domains (CDC / AAP)

/// Four CDC / AAP surveillance domains used for milestone grouping and gauges.
public enum DevelopmentalDomain: String, Codable, CaseIterable, Sendable, Identifiable {
    case socialEmotional = "Social-Emotional"
    case languageCommunication = "Language / Communication"
    case cognitive = "Cognitive"
    case sensoryMotor = "Sensory & Motor Regulation"

    public var id: String { rawValue }

    public var systemImage: String {
        switch self {
        case .socialEmotional: return "heart.circle.fill"
        case .languageCommunication: return "text.bubble.fill"
        case .cognitive: return "brain.head.profile.fill"
        case .sensoryMotor: return "figure.walk.motion"
        }
    }

    public var shortLabel: String {
        switch self {
        case .socialEmotional: return "Social"
        case .languageCommunication: return "Language"
        case .cognitive: return "Cognitive"
        case .sensoryMotor: return "Sensory"
        }
    }
}

public enum CDCSurveillanceAge: Int, Codable, CaseIterable, Sendable, Identifiable {
    case months9 = 9
    case months12 = 12
    case months18 = 18
    case months24 = 24
    case months30 = 30
    case months36 = 36

    public var id: Int { rawValue }
    public var title: String { "\(rawValue)-month" }
}

/// One educational checklist row inside a milestone.
public struct MilestoneChecklistItem: Identifiable, Codable, Sendable, Equatable {
    public var id: String
    public var title: String
    public var whatToLookFor: String
    public var howToPrompt: String
    public var whenToConsult: String
    public var isCompleted: Bool

    public init(
        id: String = UUID().uuidString,
        title: String,
        whatToLookFor: String,
        howToPrompt: String,
        whenToConsult: String,
        isCompleted: Bool = false
    ) {
        self.id = id
        self.title = title
        self.whatToLookFor = whatToLookFor
        self.howToPrompt = howToPrompt
        self.whenToConsult = whenToConsult
        self.isCompleted = isCompleted
    }
}

/// CDC / AAP milestone at a surveillance age and domain.
public struct Milestone: Identifiable, Codable, Sendable, Equatable {
    public let id: String
    public let ageMonths: Int
    public var title: String
    public var dueDateString: String
    public var domain: DevelopmentalDomain
    public var items: [MilestoneChecklistItem]
    public var isCompleted: Bool

    /// Legacy string checklist for CareKit JSON export.
    public var checklist: [String] { items.map(\.title) }

    public var completionRatio: Double {
        guard !items.isEmpty else { return isCompleted ? 1 : 0 }
        return Double(items.filter(\.isCompleted).count) / Double(items.count)
    }

    public init(
        id: String,
        ageMonths: Int,
        title: String,
        dueDateString: String,
        domain: DevelopmentalDomain,
        items: [MilestoneChecklistItem],
        isCompleted: Bool = false
    ) {
        self.id = id
        self.ageMonths = ageMonths
        self.title = title
        self.dueDateString = dueDateString
        self.domain = domain
        self.items = items
        self.isCompleted = isCompleted || items.allSatisfy(\.isCompleted)
    }
}

/// Compatibility alias used by existing dashboard and export code.
public typealias MilestoneItem = Milestone

/// M-CHAT-R/F band wrapper (Low 0–2, Medium 3–7 Follow-Up, High 8–20).
public struct ScreeningRiskScore: Sendable, Codable, Equatable {
    public let totalFailed: Int
    public let riskLevel: MCHATRiskLevel
    public let failedQuestionIds: [Int]
    public let requiresFollowUpInterview: Bool
    public let clinicalRecommendation: String
    public let completionDate: Date

    public init(from result: MCHATScoreResult) {
        self.totalFailed = result.totalFailed
        self.riskLevel = result.riskLevel
        self.failedQuestionIds = result.failedQuestionIds
        self.requiresFollowUpInterview = result.requiresFollowUpInterview
        self.clinicalRecommendation = result.clinicalRecommendation
        self.completionDate = result.completionDate
    }
}

public enum StimulusContextTag: String, Codable, Sendable, CaseIterable {
    case faceToFace = "Face-to-Face"
    case screenStimuli = "Screen Stimuli"
}

/// Session-level gaze review card (ResearchKit active-task summary). No raw video.
public struct GazeTelemetrySession: Identifiable, Sendable, Equatable {
    public let id: String
    public let sessionId: String
    public let startedAt: Date
    public let totalValidTimeSec: Double
    public let sessionDurationSec: Double
    public let socialTargetRatio: Double
    public let trackingConfidenceIndex: Double
    /// `nil` when not measured (PLC-1); render "Not observed".
    public let saccadicLatencyMs: Double?
    public let stimulusTag: StimulusContextTag
    public let heatmapSummary: String

    public init(
        id: String,
        sessionId: String,
        startedAt: Date,
        totalValidTimeSec: Double,
        sessionDurationSec: Double,
        socialTargetRatio: Double,
        trackingConfidenceIndex: Double,
        saccadicLatencyMs: Double?,
        stimulusTag: StimulusContextTag,
        heatmapSummary: String
    ) {
        self.id = id
        self.sessionId = sessionId
        self.startedAt = startedAt
        self.totalValidTimeSec = totalValidTimeSec
        self.sessionDurationSec = sessionDurationSec
        self.socialTargetRatio = socialTargetRatio
        self.trackingConfidenceIndex = trackingConfidenceIndex
        self.saccadicLatencyMs = saccadicLatencyMs
        self.stimulusTag = stimulusTag
        self.heatmapSummary = heatmapSummary
    }

    /// "42%" for a measured ratio, "Not observed" for an absent one (PLC-2).
    static func percent(_ ratio: Double?) -> String {
        guard let ratio else { return "Not observed" }
        return String(format: "%.0f%%", ratio * 100)
    }

    public init(from record: GazeSessionRecord) {
        let socialRatio = max(0, min(1, (record.socialPreferenceScore + 1.0) / 2.0))
        let confidence: Double
        switch record.qualityVerdict.lowercased() {
        case "optimal": confidence = min(1, record.validFrameRatio)
        case "acceptable": confidence = record.validFrameRatio * 0.85
        default: confidence = record.validFrameRatio * 0.6
        }
        self.init(
            id: record.id,
            sessionId: record.sessionId,
            startedAt: record.startedAt,
            totalValidTimeSec: record.durationSec * record.validFrameRatio,
            sessionDurationSec: record.durationSec,
            socialTargetRatio: socialRatio,
            trackingConfidenceIndex: confidence,
            saccadicLatencyMs: record.firstEyeLatencyMs,
            stimulusTag: record.durationSec >= 45 ? .screenStimuli : .faceToFace,
            heatmapSummary: "Eye dwell \(Self.percent(record.eyeDwellRatio)) · Mouth dwell \(Self.percent(record.mouthDwellRatio)) · Valid frames \(Self.percent(record.validFrameRatio))"
        )
    }
}

public enum BehavioralObservationTag: String, Codable, CaseIterable, Sendable, Identifiable {
    case jointAttention = "Joint Attention"
    case turnTaking = "Turn-taking"
    case repetitiveBehavior = "Repetitive Behavior"

    public var id: String { rawValue }
    public var systemImage: String {
        switch self {
        case .jointAttention: return "scope"
        case .turnTaking: return "arrow.left.arrow.right"
        case .repetitiveBehavior: return "repeat.circle"
        }
    }
}

public struct TodayObservation: Identifiable, Codable, Sendable, Equatable {
    public var id: UUID
    public var date: Date
    public var notes: String
    public var tags: [BehavioralObservationTag]
    public var hasOnDeviceMedia: Bool

    public init(
        id: UUID = UUID(),
        date: Date = Date(),
        notes: String = "",
        tags: [BehavioralObservationTag] = [],
        hasOnDeviceMedia: Bool = false
    ) {
        self.id = id
        self.date = date
        self.notes = notes
        self.tags = tags
        self.hasOnDeviceMedia = hasOnDeviceMedia
    }
}

public struct ClinicalTerm: Identifiable, Sendable {
    public let id: String
    public let term: String
    public let explanation: String
}

public enum ClinicalGlossary {
    public static let pointingContrast = ClinicalTerm(
        id: "pointing",
        term: "Pointing",
        explanation: "Imperative pointing asks for an object or help. Declarative pointing shares interest (“look at that”) without requesting the object. Loss of declarative pointing is a CDC surveillance flag."
    )
    public static let jointAttention = ClinicalTerm(
        id: "ja",
        term: "Joint attention",
        explanation: "The child coordinates gaze between a person and an object, checking that you are looking at the same thing. It is a social-communication building block, not a diagnosis."
    )
    public static let geopref = ClinicalTerm(
        id: "geopref",
        term: "Social vs geometric preference",
        explanation: "During GeoPref-style clips, typically developing toddlers look more at faces than at repeating geometric patterns. This app records looking time only — it does not diagnose ASD."
    )
}

public struct AttentionTrendPoint: Identifiable, Sendable {
    public let id: String
    public let dayLabel: String
    public let socialPreferenceScore: Double
    public let sessionDurationSec: Double
    public let stimulusTag: StimulusContextTag
    public let isLatest: Bool
}
