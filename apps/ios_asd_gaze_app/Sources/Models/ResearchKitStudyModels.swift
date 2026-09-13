import CurvAsdGazeKit
import Foundation

// MARK: - AnyCodable Value Wrapper

/// Sendable & Codable wrapper for heterogeneous dictionary values in ResearchKit exports.
public enum AnyCodableValue: Sendable, Codable, Equatable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case dictionary([String: AnyCodableValue])
    case array([AnyCodableValue])
    case null

    public init(_ value: Any?) {
        guard let value else { self = .null; return }
        switch value {
        case let s as String: self = .string(s)
        case let i as Int: self = .int(i)
        case let d as Double: self = .double(d)
        case let f as Float: self = .double(Double(f))
        case let b as Bool: self = .bool(b)
        case let dict as [String: Any]:
            self = .dictionary(dict.mapValues { AnyCodableValue($0) })
        case let arr as [Any]:
            self = .array(arr.map { AnyCodableValue($0) })
        default:
            self = .string(String(describing: value))
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let b = try? container.decode(Bool.self) { self = .bool(b) }
        else if let i = try? container.decode(Int.self) { self = .int(i) }
        else if let d = try? container.decode(Double.self) { self = .double(d) }
        else if let s = try? container.decode(String.self) { self = .string(s) }
        else if let dict = try? container.decode([String: AnyCodableValue].self) { self = .dictionary(dict) }
        else if let arr = try? container.decode([AnyCodableValue].self) { self = .array(arr) }
        else { self = .null }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let s): try container.encode(s)
        case .int(let i): try container.encode(i)
        case .double(let d): try container.encode(d)
        case .bool(let b): try container.encode(b)
        case .dictionary(let dict): try container.encode(dict)
        case .array(let arr): try container.encode(arr)
        case .null: try container.encodeNil()
        }
    }

    public var anyValue: Any? {
        switch self {
        case .string(let s): return s
        case .int(let i): return i
        case .double(let d): return d
        case .bool(let b): return b
        case .dictionary(let dict): return dict.mapValues { $0.anyValue }
        case .array(let arr): return arr.map { $0.anyValue }
        case .null: return nil
        }
    }
}

// MARK: - ResearchKit Consent Schemas

public enum ORKConsentSectionType: String, Sendable, Codable, CaseIterable {
    case overview = "overview"
    case dataGathering = "dataGathering"
    case cameraPrivacy = "cameraPrivacy"
    case studySurvey = "studySurvey"
    case timeCommitment = "timeCommitment"
    case withdrawing = "withdrawing"
}

public struct ORKConsentSection: Sendable, Codable, Identifiable {
    public var id: String { type.rawValue }
    public let type: ORKConsentSectionType
    public let title: String
    public let summary: String
    public let content: String

    public init(type: ORKConsentSectionType, title: String, summary: String, content: String) {
        self.type = type
        self.title = title
        self.summary = summary
        self.content = content
    }
}

public struct ORKConsentSignature: Sendable, Codable, Equatable {
    public let signatureId: String
    public let guardianFullName: String
    public let signedDate: Date
    public let signatureToken: String
    public let consentVersion: String

    public init(
        signatureId: String = UUID().uuidString,
        guardianFullName: String,
        signedDate: Date = Date(),
        signatureToken: String,
        consentVersion: String = "v1.3-IRB2026"
    ) {
        self.signatureId = signatureId
        self.guardianFullName = guardianFullName
        self.signedDate = signedDate
        self.signatureToken = signatureToken
        self.consentVersion = consentVersion
    }
}

public struct ORKConsentDocument: Sendable, Codable {
    public let title: String
    public let studyIdentifier: String
    public let irbProtocolId: String
    public let institution: String
    public let principalInvestigator: String
    public let sections: [ORKConsentSection]
    public var signatures: [ORKConsentSignature]

    public init(
        title: String = "Pediatric Social Visual Attention Clinical Research Study",
        studyIdentifier: String = "CURV-ASD-GAZE-2026",
        irbProtocolId: String = "IRB-ASD-2026-088",
        institution: String = "Center for Pediatric Neurodevelopment & Vision Research",
        principalInvestigator: String = "Dr. Elena Vasquez, MD, PhD",
        sections: [ORKConsentSection] = ORKConsentDocument.defaultSections(),
        signatures: [ORKConsentSignature] = []
    ) {
        self.title = title
        self.studyIdentifier = studyIdentifier
        self.irbProtocolId = irbProtocolId
        self.institution = institution
        self.principalInvestigator = principalInvestigator
        self.sections = sections
        self.signatures = signatures
    }

    public static func defaultSections() -> [ORKConsentSection] {
        [
            ORKConsentSection(
                type: .overview,
                title: "Study Purpose",
                summary: "Observational study evaluating early visual orienting markers in toddlers.",
                content: "This clinical research study investigates how toddlers engage with naturalistic social and geometric video stimuli. The objective is to validate non-invasive digital phenotyping algorithms for early neurodevelopmental assessment."
            ),
            ORKConsentSection(
                type: .cameraPrivacy,
                title: "Strict Zero Raw Video Guarantee",
                summary: "No facial photos, videos, or raw camera buffers are ever stored or uploaded.",
                content: "Per US COPPA, GDPR-K, and IRB Protocol #2026-088, the front-facing TrueDepth camera processes facial geometry exclusively in volatile device RAM. Once 2D screen gaze coordinates are derived, all video frames are immediately deallocated. Only numerical telemetry is saved."
            ),
            ORKConsentSection(
                type: .dataGathering,
                title: "Data Collected",
                summary: "De-identified gaze coordinates, head pose dynamics, and developmental milestones.",
                content: "The study records timestamps, normalized screen gaze points (x, y), fixation durations, and caregiver-reported developmental questionnaire responses. All records use an anonymous participant token."
            ),
            ORKConsentSection(
                type: .studySurvey,
                title: "M-CHAT-R/F Survey",
                summary: "20 standard developmental questions completed by the legal guardian.",
                content: "Caregivers complete the Modified Checklist for Autism in Toddlers. Responses help correlate behavioral observations with physiological eye-tracking metrics."
            ),
            ORKConsentSection(
                type: .withdrawing,
                title: "Voluntary Participation",
                summary: "You may pause, exit, or withdraw your consent at any time without penalty.",
                content: "Participation is 100% voluntary. Withdrawing from the research study does NOT affect your access to local CareKit care plan features, medication tracking, or clinical pediatric appointments."
            )
        ]
    }
}

// MARK: - M-CHAT-R/F Standardized Survey Engine

public struct MCHATQuestion: Sendable, Codable, Identifiable {
    public let id: Int
    public let text: String
    /// The answer that indicates an "atypical / failed" developmental signal.
    public let failAnswer: Bool
    public let isCritical: Bool

    public init(id: Int, text: String, failAnswer: Bool, isCritical: Bool = false) {
        self.id = id
        self.text = text
        self.failAnswer = failAnswer
        self.isCritical = isCritical
    }
}

public enum MCHATRiskLevel: String, Sendable, Codable {
    case low = "Low Risk"
    case medium = "Medium Risk"
    case high = "High Risk"
}

public struct MCHATScoreResult: Sendable, Codable, Equatable {
    public let totalFailed: Int
    public let riskLevel: MCHATRiskLevel
    public let failedQuestionIds: [Int]
    public let clinicalRecommendation: String
    public let completionDate: Date

    public init(
        totalFailed: Int,
        riskLevel: MCHATRiskLevel,
        failedQuestionIds: [Int],
        clinicalRecommendation: String,
        completionDate: Date = Date()
    ) {
        self.totalFailed = totalFailed
        self.riskLevel = riskLevel
        self.failedQuestionIds = failedQuestionIds
        self.clinicalRecommendation = clinicalRecommendation
        self.completionDate = completionDate
    }
}

public enum MCHATSurveyEngine {
    public static let questions: [MCHATQuestion] = [
        MCHATQuestion(id: 1, text: "If you point at something across the room, does your child look at it?", failAnswer: false, isCritical: true),
        MCHATQuestion(id: 2, text: "Have you ever wondered if your child might be deaf?", failAnswer: true),
        MCHATQuestion(id: 3, text: "Does your child play pretend or make-believe (e.g. drink from an empty cup)?", failAnswer: false),
        MCHATQuestion(id: 4, text: "Does your child like climbing on things (furniture, playground equipment)?", failAnswer: false),
        MCHATQuestion(id: 5, text: "Does your child make unusual finger movements near his/her eyes?", failAnswer: true),
        MCHATQuestion(id: 6, text: "Does your child point with one finger to ask for something or to get help?", failAnswer: false),
        MCHATQuestion(id: 7, text: "Does your child point with one finger to show you something interesting?", failAnswer: false, isCritical: true),
        MCHATQuestion(id: 8, text: "Is your child interested in other children (watching, smiling, approaching)?", failAnswer: false),
        MCHATQuestion(id: 9, text: "Does your child show you things by bringing them over or holding them up?", failAnswer: false, isCritical: true),
        MCHATQuestion(id: 10, text: "Does your child respond when you call his/her name (looks up, talks, stops)?", failAnswer: false, isCritical: true),
        MCHATQuestion(id: 11, text: "When you smile at your child, does he/she smile back at you?", failAnswer: false),
        MCHATQuestion(id: 12, text: "Does your child get upset by everyday noises (vacuum, loud music)?", failAnswer: true),
        MCHATQuestion(id: 13, text: "Does your child walk independently?", failAnswer: false),
        MCHATQuestion(id: 14, text: "Does your child look you in the eye when talking, playing, or dressing?", failAnswer: false, isCritical: true),
        MCHATQuestion(id: 15, text: "Does your child try to copy what you do (clapping, waving, funny sounds)?", failAnswer: false, isCritical: true),
        MCHATQuestion(id: 16, text: "If you turn your head to look at something, does your child turn to look?", failAnswer: false),
        MCHATQuestion(id: 17, text: "Does your child try to get you to watch him/her (says 'Look' or 'Watch me')?", failAnswer: false),
        MCHATQuestion(id: 18, text: "Does your child understand when you tell him/her to do something simple?", failAnswer: false),
        MCHATQuestion(id: 19, text: "If something new happens, does your child look at your face to see your reaction?", failAnswer: false),
        MCHATQuestion(id: 20, text: "Does your child like movement activities (being swung, bounced on your knee)?", failAnswer: false)
    ]

    public static func evaluate(answers: [Int: Bool]) -> MCHATScoreResult {
        var failedIds: [Int] = []
        for q in questions {
            if let ans = answers[q.id], ans == q.failAnswer {
                failedIds.append(q.id)
            }
        }
        let total = failedIds.count
        let risk: MCHATRiskLevel
        let rec: String

        if total <= 2 {
            risk = .low
            rec = "Low developmental risk score. Continue monitoring milestones during routine pediatric well-child checks."
        } else if total <= 7 {
            risk = .medium
            rec = "Medium risk score. Clinical follow-up with developmental pediatrician or early intervention assessment recommended."
        } else {
            risk = .high
            rec = "High risk score. Prompt referral for comprehensive multidisciplinary developmental evaluation strongly recommended."
        }

        return MCHATScoreResult(
            totalFailed: total,
            riskLevel: risk,
            failedQuestionIds: failedIds,
            clinicalRecommendation: rec
        )
    }
}

// MARK: - ResearchKit Active Step Result Schema

public struct ORKActiveStepResult: Sendable, Codable, Equatable {
    public let identifier: String
    public let type: String
    public let sessionId: String
    public let durationSec: Double
    public let isValid: Bool
    public let validSampleRatio: Double
    public let qualityVerdict: String
    public let socialPreferenceScore: Double
    public let eyeDwellRatio: Double
    public let mouthDwellRatio: Double
    public let firstEyeLatencyMs: Double
    public let socialSmileIntensity: Double
    public let smileReactivityIndex: Double
    public let flatAffectScore: Double
    public let surpriseScoreMean: Double
    public let telemetrySampleCount: Int
    public let recordedAt: Date

    public init(
        identifier: String = "ORKActiveStepResult",
        type: String = "social_attention_gaze_result",
        sessionId: String,
        durationSec: Double,
        isValid: Bool,
        validSampleRatio: Double,
        qualityVerdict: String,
        socialPreferenceScore: Double,
        eyeDwellRatio: Double,
        mouthDwellRatio: Double,
        firstEyeLatencyMs: Double,
        socialSmileIntensity: Double,
        smileReactivityIndex: Double,
        flatAffectScore: Double,
        surpriseScoreMean: Double,
        telemetrySampleCount: Int = 0,
        recordedAt: Date = Date()
    ) {
        self.identifier = identifier
        self.type = type
        self.sessionId = sessionId
        self.durationSec = durationSec
        self.isValid = isValid
        self.validSampleRatio = validSampleRatio
        self.qualityVerdict = qualityVerdict
        self.socialPreferenceScore = socialPreferenceScore
        self.eyeDwellRatio = eyeDwellRatio
        self.mouthDwellRatio = mouthDwellRatio
        self.firstEyeLatencyMs = firstEyeLatencyMs
        self.socialSmileIntensity = socialSmileIntensity
        self.smileReactivityIndex = smileReactivityIndex
        self.flatAffectScore = flatAffectScore
        self.surpriseScoreMean = surpriseScoreMean
        self.telemetrySampleCount = telemetrySampleCount
        self.recordedAt = recordedAt
    }

    public init(summary: GazeSessionReportSummary, telemetrySampleCount: Int = 0) {
        self.init(
            sessionId: summary.sessionId,
            durationSec: summary.durationSec,
            isValid: summary.validRatio >= 0.60,
            validSampleRatio: summary.validRatio,
            qualityVerdict: summary.qualityVerdict.rawValue,
            socialPreferenceScore: summary.socialPreferenceScore,
            eyeDwellRatio: summary.eyeDwellRatio,
            mouthDwellRatio: summary.mouthDwellRatio,
            firstEyeLatencyMs: summary.firstEyeLatencyMs,
            socialSmileIntensity: summary.socialSmileIntensityMean,
            smileReactivityIndex: summary.smileReactivityIndex,
            flatAffectScore: summary.flatAffectScore,
            surpriseScoreMean: summary.surpriseScoreMean,
            telemetrySampleCount: telemetrySampleCount
        )
    }
}

// MARK: - Step Result and Task Result

public struct ORKStepResult: Sendable, Codable, Equatable {
    public let identifier: String
    public let stepType: String
    public let startDate: Date
    public let endDate: Date
    public let results: [String: AnyCodableValue]

    public init(
        identifier: String,
        stepType: String,
        startDate: Date = Date(),
        endDate: Date = Date(),
        results: [String: AnyCodableValue] = [:]
    ) {
        self.identifier = identifier
        self.stepType = stepType
        self.startDate = startDate
        self.endDate = endDate
        self.results = results
    }
}

public struct ORKTaskResult: Sendable, Codable, Equatable {
    public let identifier: String
    public let taskRunUUID: UUID
    public let participantId: String
    public let studyIdentifier: String
    public let irbProtocolId: String
    public let startDate: Date
    public let endDate: Date
    public var stepResults: [ORKStepResult]
    public var activeStepResult: ORKActiveStepResult?

    public init(
        identifier: String = "AsdResearchStudyTask",
        taskRunUUID: UUID = UUID(),
        participantId: String,
        studyIdentifier: String = "CURV-ASD-GAZE-2026",
        irbProtocolId: String = "IRB-ASD-2026-088",
        startDate: Date = Date(),
        endDate: Date = Date(),
        stepResults: [ORKStepResult] = [],
        activeStepResult: ORKActiveStepResult? = nil
    ) {
        self.identifier = identifier
        self.taskRunUUID = taskRunUUID
        self.participantId = participantId
        self.studyIdentifier = studyIdentifier
        self.irbProtocolId = irbProtocolId
        self.startDate = startDate
        self.endDate = endDate
        self.stepResults = stepResults
        self.activeStepResult = activeStepResult
    }
}

// MARK: - Research Study Enrollment Profile

public struct ResearchStudyEnrollment: Sendable, Codable, Equatable {
    public var participantId: String
    public var isConsented: Bool
    public var consentSignature: ORKConsentSignature?
    public var childAgeMonths: Int
    public var cloudSyncOptIn: Bool
    public var lastSyncTimestamp: Date?
    public var totalScreeningsSynced: Int

    public init(
        participantId: String = ResearchStudyEnrollment.generateParticipantId(),
        isConsented: Bool = false,
        consentSignature: ORKConsentSignature? = nil,
        childAgeMonths: Int = 24,
        cloudSyncOptIn: Bool = true,
        lastSyncTimestamp: Date? = nil,
        totalScreeningsSynced: Int = 0
    ) {
        self.participantId = participantId
        self.isConsented = isConsented
        self.consentSignature = consentSignature
        self.childAgeMonths = childAgeMonths
        self.cloudSyncOptIn = cloudSyncOptIn
        self.lastSyncTimestamp = lastSyncTimestamp
        self.totalScreeningsSynced = totalScreeningsSynced
    }

    public static func generateParticipantId() -> String {
        let hex = UUID().uuidString.prefix(6).uppercased()
        return "STUDY-ASD-\(hex)"
    }
}

// MARK: - Study Export Package

public struct StudyParticipantMetadata: Sendable, Codable, Equatable {
    public let participantId: String
    public let isConsented: Bool
    public let childAgeMonths: Int
    public let consentVersion: String

    public init(
        participantId: String,
        isConsented: Bool,
        childAgeMonths: Int,
        consentVersion: String
    ) {
        self.participantId = participantId
        self.isConsented = isConsented
        self.childAgeMonths = childAgeMonths
        self.consentVersion = consentVersion
    }
}

public struct ResearchKitStudyPackage: Sendable, Codable, Equatable {
    public let bundleType: String
    public let schemaVersion: String
    public let compiledAt: String
    public let studyIdentifier: String
    public let irbProtocolId: String
    public let participant: StudyParticipantMetadata
    public let taskResults: [ORKTaskResult]
    public let surveys: [MCHATScoreResult]
    public var payloadSha256: String?

    public init(
        bundleType: String = "ResearchKitStudyPackage",
        schemaVersion: String = "v1.4",
        compiledAt: String = ISO8601DateFormatter().string(from: Date()),
        studyIdentifier: String = "CURV-ASD-GAZE-2026",
        irbProtocolId: String = "IRB-ASD-2026-088",
        participant: StudyParticipantMetadata,
        taskResults: [ORKTaskResult] = [],
        surveys: [MCHATScoreResult] = [],
        payloadSha256: String? = nil
    ) {
        self.bundleType = bundleType
        self.schemaVersion = schemaVersion
        self.compiledAt = compiledAt
        self.studyIdentifier = studyIdentifier
        self.irbProtocolId = irbProtocolId
        self.participant = participant
        self.taskResults = taskResults
        self.surveys = surveys
        self.payloadSha256 = payloadSha256
    }
}

