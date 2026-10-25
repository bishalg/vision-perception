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
        consentVersion: String = ResearchStudyConfiguration.current?.consentVersion
            ?? ResearchStudyConfiguration.unconfiguredStudyIdentifier
    ) {
        self.signatureId = signatureId
        self.guardianFullName = guardianFullName
        self.signedDate = signedDate
        self.signatureToken = signatureToken
        self.consentVersion = consentVersion
    }
}

/// The consent document of the configured study (CNS-9). It has no defaults:
/// every study fact comes from `ResearchStudyConfiguration`, so a build without
/// a study cannot show a consent document at all (ADR-0002 decision 5).
public struct ORKConsentDocument: Sendable, Codable {
    public let title: String
    public let studyIdentifier: String
    public let irbProtocolId: String
    public let institution: String
    public let principalInvestigator: String
    public let consentVersion: String
    public let sections: [ORKConsentSection]
    public var signatures: [ORKConsentSignature]

    public init(configuration: ResearchStudyConfiguration, signatures: [ORKConsentSignature] = []) {
        self.title = "Optional research participation: \(configuration.institution)"
        self.studyIdentifier = configuration.studyIdentifier
        self.irbProtocolId = configuration.irbProtocolId
        self.institution = configuration.institution
        self.principalInvestigator = configuration.principalInvestigator
        self.consentVersion = configuration.consentVersion
        self.sections = Self.sections(for: configuration)
        self.signatures = signatures
    }

    public static func sections(for configuration: ResearchStudyConfiguration) -> [ORKConsentSection] {
        [
            ORKConsentSection(
                type: .overview,
                title: "Purpose",
                summary: "\(configuration.institution) is collecting de-identified observation data. Taking part is optional.",
                content: "Study \(configuration.studyIdentifier) is run by \(configuration.institution) (lead investigator: \(configuration.principalInvestigator)) under ethics approval \(configuration.irbProtocolId). It looks at how young children watch short social and non-social video clips. This app is not a diagnostic tool, and taking part does not give you a diagnosis or medical advice."
            ),
            ORKConsentSection(
                type: .cameraPrivacy,
                title: "No video is stored",
                summary: "No photos, videos or camera frames are stored or uploaded.",
                content: "The front camera is processed on this device while a clip plays. Only numbers, such as where on the screen your child looked and for how long, are kept. Camera frames are discarded as soon as they are processed."
            ),
            ORKConsentSection(
                type: .dataGathering,
                title: "What is shared",
                summary: "De-identified gaze summaries, diary observations and questionnaire answers.",
                content: "If you agree, the app sends timestamps, gaze summaries, diary observations (without your free-text notes) and M-CHAT-R/F answers to \(configuration.institution) over an encrypted connection. Records carry a random participant code, never your child's name."
            ),
            ORKConsentSection(
                type: .studySurvey,
                title: "M-CHAT-R/F questionnaire",
                summary: "20 standard questions answered by a parent or guardian.",
                content: "You may answer the Modified Checklist for Autism in Toddlers, Revised with Follow-Up. Your answers are shared with the study only while your consent is active."
            ),
            ORKConsentSection(
                type: .withdrawing,
                title: "Withdrawing",
                summary: "You can withdraw at any time in Settings.",
                content: "Taking part is voluntary. Withdrawing stops all uploads and deletes anything still waiting to be sent from this device. It does not affect your care plans, medications or diary in this app. Consent version \(configuration.consentVersion)."
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

    /// Standard LOINC code (LOINC panel 80628-1 item code: 80629-9 to 80648-9).
    public var loincCode: String {
        StandardCodingSystems.LOINC.loincCode(forMCHATQuestionId: id) ?? "80628-1"
    }

    public init(id: Int, text: String, failAnswer: Bool, isCritical: Bool = false) {
        self.id = id
        self.text = text
        self.failAnswer = failAnswer
        self.isCritical = isCritical
    }
}

/// M-CHAT-R/F result category. Raw values are persisted (Codable) and MUST NOT
/// change; user-facing text uses `displayName` (CLM-4).
public enum MCHATRiskLevel: String, Sendable, Codable {
    case low = "Low Risk" // claims-ok: persisted raw value, never shown (CLM-4)
    case medium = "Medium Risk" // claims-ok: persisted raw value, never shown (CLM-4)
    case high = "High Risk" // claims-ok: persisted raw value, never shown (CLM-4)

    /// Parent-facing wording: names the instrument, the category and the next step,
    /// e.g. "M-CHAT-R/F: medium (follow-up interview needed; discuss with your pediatrician)".
    public var displayName: String {
        switch self {
        case .low:
            return "M-CHAT-R/F: low (no follow-up needed now; discuss any concerns with your pediatrician)"
        case .medium:
            return "M-CHAT-R/F: medium (follow-up interview needed; discuss with your pediatrician)"
        case .high:
            return "M-CHAT-R/F: high (follow-up needed; discuss with your pediatrician soon)"
        }
    }
}

public struct MCHATScoreResult: Sendable, Codable, Equatable {
    public let totalFailed: Int
    public let riskLevel: MCHATRiskLevel
    public let failedQuestionIds: [Int]
    public let clinicalRecommendation: String
    public let completionDate: Date

    /// Standard LOINC coding for M-CHAT-R score and risk interpretation.
    public var loincScoreCoding: ClinicalCoding { StandardCodingSystems.LOINC.mchatRTotalScore }
    public var loincRiskCoding: ClinicalCoding { StandardCodingSystems.LOINC.mchatRRiskLevel }

    /// M-CHAT-R/F protocol: medium scores (3–7) require the structured
    /// Follow-Up interview before any referral decision.
    public var requiresFollowUpInterview: Bool { riskLevel == .medium }

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

// MARK: - FHIR R4 Serialization (STAGE 2)

extension MCHATScoreResult {
    /// Converts this M-CHAT-R/F result into an HL7 FHIR Release 4 `QuestionnaireResponse` resource dictionary.
    public func toFHIRQuestionnaireResponse(answers: [Int: Bool] = [:], participantId: String) -> [String: AnyCodableValue] {
        FHIRInteroperabilityMapper.makeFHIRQuestionnaireResponse(from: self, answers: answers, participantId: participantId)
    }

    /// Converts this M-CHAT-R/F result into OMOP CDM `OBSERVATION` tabular rows.
    public func toOMOPObservations(personId: String, answers: [Int: Bool]) -> [OMOPObservationRecord] {
        let dateStr = ISO8601DateFormatter().string(from: completionDate).prefix(10)
        var records: [OMOPObservationRecord] = []
        for qId in 1...20 {
            let loincCode = StandardCodingSystems.LOINC.loincCode(forMCHATQuestionId: qId) ?? "80628-1"
            let ansBool = answers[qId]
            let valueConceptId = ansBool == true ? 4188539 : (ansBool == false ? 4188540 : nil)
            records.append(OMOPObservationRecord(
                personId: personId,
                observationConceptId: Int(loincCode) ?? 806281,
                observationDate: String(dateStr),
                valueAsString: ansBool.map { $0 ? "Yes" : "No" },
                valueAsConceptId: valueConceptId,
                qualifierConceptId: nil,
                observationSourceValue: "MCHAT-R-Q\(qId)"
            ))
        }
        return records
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
            rec = "Medium risk (3–7). Complete the official M-CHAT-R Follow-Up interview before referral; do not treat this score as a diagnosis."
        } else {
            risk = .high
            rec = "High risk (8–20). Seek prompt clinical evaluation with a pediatrician or developmental specialist. This score is a screening result, not a diagnosis."
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
    /// `nil` when not measured (PLC-2); omitted from the encoded JSON.
    public let eyeDwellRatio: Double?
    /// `nil` when not measured (PLC-2); omitted from the encoded JSON.
    public let mouthDwellRatio: Double?
    /// `nil` until a first-fixation detector exists (PLC-1); omitted from the encoded JSON.
    public let firstEyeLatencyMs: Double?
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
        eyeDwellRatio: Double?,
        mouthDwellRatio: Double?,
        firstEyeLatencyMs: Double?,
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

    /// Regulatory eCOA / Digital Biomarker classification.
    public var outcomeType: ClinicalOutcomeAssessmentType { .digitalBiomarker }

    /// Standard LOINC panel coding.
    public var loincPanelCoding: ClinicalCoding { StandardCodingSystems.LOINC.gazeTrackingPanel }

    /// Standard SNOMED CT procedure coding.
    public var snomedProcedureCoding: ClinicalCoding { StandardCodingSystems.SNOMED.eyeTrackingProcedure }
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
        configuration: ResearchStudyConfiguration? = ResearchStudyConfiguration.current,
        startDate: Date = Date(),
        endDate: Date = Date(),
        stepResults: [ORKStepResult] = [],
        activeStepResult: ORKActiveStepResult? = nil
    ) {
        self.identifier = identifier
        self.taskRunUUID = taskRunUUID
        self.participantId = participantId
        self.studyIdentifier = configuration?.studyIdentifier ?? ResearchStudyConfiguration.unconfiguredStudyIdentifier
        self.irbProtocolId = configuration?.irbProtocolId ?? ResearchStudyConfiguration.unconfiguredStudyIdentifier
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
    /// Opt-in to contribute de-identified per-video metric sets toward building our own
    /// age-bracketed normative references. Optional for backward-compatible decoding of
    /// pre-existing enrollment payloads (nil = not opted in).
    public var normCollectionOptIn: Bool?

    public init(
        participantId: String = ResearchStudyEnrollment.generateParticipantId(),
        isConsented: Bool = false,
        consentSignature: ORKConsentSignature? = nil,
        childAgeMonths: Int = 24,
        cloudSyncOptIn: Bool = true,
        lastSyncTimestamp: Date? = nil,
        totalScreeningsSynced: Int = 0,
        normCollectionOptIn: Bool? = nil
    ) {
        self.participantId = participantId
        self.isConsented = isConsented
        self.consentSignature = consentSignature
        self.childAgeMonths = childAgeMonths
        self.cloudSyncOptIn = cloudSyncOptIn
        self.lastSyncTimestamp = lastSyncTimestamp
        self.totalScreeningsSynced = totalScreeningsSynced
        self.normCollectionOptIn = normCollectionOptIn
    }

    public static func generateParticipantId() -> String {
        let hex = UUID().uuidString.prefix(6).uppercased()
        return "STUDY-ASD-\(hex)"
    }
}

// MARK: - Normative Data Collection Gate

/// Shared gate deciding whether a session contributes to normative reference collection.
/// Requires research consent, explicit norm-collection opt-in, and live TrueDepth capture
/// (simulated frames must never enter a normative distribution, SIM-5).
public enum NormCollectionGate {
    public static func isActive(
        enrollment: ResearchStudyEnrollment?,
        captureSource: CaptureSource?
    ) -> Bool {
        guard let enrollment = enrollment else { return false }
        return enrollment.isConsented
            && enrollment.normCollectionOptIn == true
            && captureSource == .trueDepth
    }

    /// Builds the export payload for an opted-in session; nil when the gate is closed.
    public static func collectionMetadata(
        enrollment: ResearchStudyEnrollment?,
        childAgeMonths: Int,
        captureSource: CaptureSource?,
        configuration: ResearchStudyConfiguration? = ResearchStudyConfiguration.current
    ) -> NormCollectionMetadata? {
        guard isActive(enrollment: enrollment, captureSource: captureSource),
              let enrollment = enrollment else { return nil }
        return NormCollectionMetadata(
            consentVersion: enrollment.consentSignature?.consentVersion ?? "unsigned",
            studyIdentifier: configuration?.studyIdentifier ?? ResearchStudyConfiguration.unconfiguredStudyIdentifier,
            participantId: enrollment.participantId,
            childAgeMonths: childAgeMonths,
            deviceModel: deviceModel(),
            isSimulator: false,
            collectedAtUtc: ISO8601DateFormatter().string(from: Date())
        )
    }

    private static func deviceModel() -> String {
        var systemInfo = utsname()
        uname(&systemInfo)
        let mirror = Mirror(reflecting: systemInfo.machine)
        return mirror.children.reduce(into: "") { result, element in
            guard let value = element.value as? Int8, value != 0 else { return }
            result.append(Character(UnicodeScalar(UInt8(value))))
        }
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

/// De-identified diary row. Free-text notes stay on device; only whether notes exist is exported.
public struct DiaryObservationRecord: Sendable, Codable, Equatable {
    public let loggedAt: String
    public let emotion: String
    public let recognition: String
    public let intensity: Double
    public let durationMinutes: Double
    public let tags: [String]
    public let source: String
    public let notesPresent: Bool

    public init(
        loggedAt: String,
        emotion: String,
        recognition: String,
        intensity: Double,
        durationMinutes: Double,
        tags: [String],
        source: String,
        notesPresent: Bool
    ) {
        self.loggedAt = loggedAt
        self.emotion = emotion
        self.recognition = recognition
        self.intensity = intensity
        self.durationMinutes = durationMinutes
        self.tags = tags
        self.source = source
        self.notesPresent = notesPresent
    }

    public init(entry: MoodDiaryEntry) {
        self.init(
            loggedAt: ISO8601DateFormatter().string(from: entry.loggedAt),
            emotion: entry.emotion.rawValue,
            recognition: (entry.recognition ?? .notObserved).rawValue,
            intensity: entry.intensity,
            durationMinutes: entry.durationMinutes,
            tags: entry.tags.map(\.rawValue),
            source: entry.source.rawValue,
            notesPresent: !entry.notes.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        )
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
    public let diaryObservations: [DiaryObservationRecord]
    public var payloadSha256: String?

    public init(
        bundleType: String = "ResearchKitStudyPackage",
        schemaVersion: String = "v1.5",
        compiledAt: String = ISO8601DateFormatter().string(from: Date()),
        configuration: ResearchStudyConfiguration? = ResearchStudyConfiguration.current,
        participant: StudyParticipantMetadata,
        taskResults: [ORKTaskResult] = [],
        surveys: [MCHATScoreResult] = [],
        diaryObservations: [DiaryObservationRecord] = [],
        payloadSha256: String? = nil
    ) {
        self.bundleType = bundleType
        self.schemaVersion = schemaVersion
        self.compiledAt = compiledAt
        self.studyIdentifier = configuration?.studyIdentifier ?? ResearchStudyConfiguration.unconfiguredStudyIdentifier
        self.irbProtocolId = configuration?.irbProtocolId ?? ResearchStudyConfiguration.unconfiguredStudyIdentifier
        self.participant = participant
        self.taskResults = taskResults
        self.surveys = surveys
        self.diaryObservations = diaryObservations
        self.payloadSha256 = payloadSha256
    }

    private enum CodingKeys: String, CodingKey {
        case bundleType, schemaVersion, compiledAt, studyIdentifier, irbProtocolId
        case participant, taskResults, surveys, diaryObservations, payloadSha256
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        bundleType = try container.decode(String.self, forKey: .bundleType)
        schemaVersion = try container.decode(String.self, forKey: .schemaVersion)
        compiledAt = try container.decode(String.self, forKey: .compiledAt)
        studyIdentifier = try container.decode(String.self, forKey: .studyIdentifier)
        irbProtocolId = try container.decode(String.self, forKey: .irbProtocolId)
        participant = try container.decode(StudyParticipantMetadata.self, forKey: .participant)
        taskResults = try container.decodeIfPresent([ORKTaskResult].self, forKey: .taskResults) ?? []
        surveys = try container.decodeIfPresent([MCHATScoreResult].self, forKey: .surveys) ?? []
        diaryObservations = try container.decodeIfPresent([DiaryObservationRecord].self, forKey: .diaryObservations) ?? []
        payloadSha256 = try container.decodeIfPresent(String.self, forKey: .payloadSha256)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(bundleType, forKey: .bundleType)
        try container.encode(schemaVersion, forKey: .schemaVersion)
        try container.encode(compiledAt, forKey: .compiledAt)
        try container.encode(studyIdentifier, forKey: .studyIdentifier)
        try container.encode(irbProtocolId, forKey: .irbProtocolId)
        try container.encode(participant, forKey: .participant)
        try container.encode(taskResults, forKey: .taskResults)
        try container.encode(surveys, forKey: .surveys)
        try container.encode(diaryObservations, forKey: .diaryObservations)
        try container.encodeIfPresent(payloadSha256, forKey: .payloadSha256)
    }
}

