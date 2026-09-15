import CurvAsdGazeKit
import Foundation

/// High-level session record capturing clinical gaze metrics for pediatrician review and research export.
/// Conforms to our Zero Raw Video Recording guarantee: strictly mathematical features and temporal telemetry.
public struct GazeSessionRecord: Identifiable, Codable, Sendable, Equatable {
    public let id: String
    public let sessionId: String
    public let participantId: String
    public let startedAt: Date
    public let endedAt: Date
    public let durationSec: Double

    // Core GeoPref Biomarkers
    public let socialPreferenceScore: Double // [-1.0, 1.0]: > 0 is social bias, < 0 is geometric pattern bias
    public let validFrameRatio: Double
    public let eyeDwellRatio: Double
    public let mouthDwellRatio: Double
    public let firstEyeLatencyMs: Double
    public let calibratedDistanceCm: Double
    public let qualityVerdict: String

    // Social Affect Biomarkers
    public let socialSmileIntensity: Double
    public let smileReactivityIndex: Double
    public let flatAffectScore: Double
    public let surpriseScoreMean: Double

    // Clinical Metadata
    public let notes: String?
    public let schemaVersion: String
    public let zeroRawVideoEnforced: Bool

    public init(
        id: String = UUID().uuidString,
        sessionId: String,
        participantId: String,
        startedAt: Date,
        endedAt: Date,
        durationSec: Double,
        socialPreferenceScore: Double,
        validFrameRatio: Double,
        eyeDwellRatio: Double,
        mouthDwellRatio: Double,
        firstEyeLatencyMs: Double = 480.0,
        calibratedDistanceCm: Double = 35.0,
        qualityVerdict: String = "optimal",
        socialSmileIntensity: Double = 0.45,
        smileReactivityIndex: Double = 0.38,
        flatAffectScore: Double = 0.12,
        surpriseScoreMean: Double = 0.24,
        notes: String? = nil,
        schemaVersion: String = "v1.4",
        zeroRawVideoEnforced: Bool = true
    ) {
        self.id = id
        self.sessionId = sessionId
        self.participantId = participantId
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.durationSec = durationSec
        self.socialPreferenceScore = socialPreferenceScore
        self.validFrameRatio = validFrameRatio
        self.eyeDwellRatio = eyeDwellRatio
        self.mouthDwellRatio = mouthDwellRatio
        self.firstEyeLatencyMs = firstEyeLatencyMs
        self.calibratedDistanceCm = calibratedDistanceCm
        self.qualityVerdict = qualityVerdict
        self.socialSmileIntensity = socialSmileIntensity
        self.smileReactivityIndex = smileReactivityIndex
        self.flatAffectScore = flatAffectScore
        self.surpriseScoreMean = surpriseScoreMean
        self.notes = notes
        self.schemaVersion = schemaVersion
        self.zeroRawVideoEnforced = zeroRawVideoEnforced
    }

    public init(
        summary: GazeSessionReportSummary,
        participantId: String,
        calibratedDistanceCm: Double = 35.0,
        notes: String? = nil
    ) {
        let now = Date()
        let start = now.addingTimeInterval(-summary.durationSec)
        self.init(
            sessionId: summary.sessionId,
            participantId: participantId,
            startedAt: start,
            endedAt: now,
            durationSec: summary.durationSec,
            socialPreferenceScore: summary.socialPreferenceScore,
            validFrameRatio: summary.validRatio,
            eyeDwellRatio: summary.eyeDwellRatio,
            mouthDwellRatio: summary.mouthDwellRatio,
            firstEyeLatencyMs: summary.firstEyeLatencyMs,
            calibratedDistanceCm: calibratedDistanceCm,
            qualityVerdict: summary.qualityVerdict.rawValue,
            socialSmileIntensity: summary.socialSmileIntensityMean,
            smileReactivityIndex: summary.smileReactivityIndex,
            flatAffectScore: summary.flatAffectScore,
            surpriseScoreMean: summary.surpriseScoreMean,
            notes: notes
        )
    }
}

// MARK: - FHIR R4 Serialization (STAGE 2)

extension GazeSessionRecord {
    /// Converts this gaze session record into an HL7 FHIR Release 4 `Observation` resource dictionary.
    /// Includes USCDI v5 `basedOn` linkage for ServiceRequest traceability.
    public func toFHIRObservation(basedOn serviceRequestId: String? = nil) -> [String: AnyCodableValue] {
        var obs = FHIRInteroperabilityMapper.makeFHIRObservation(from: self, participantId: participantId)
        if let serviceRequestId = serviceRequestId {
            obs["basedOn"] = .array([
                .dictionary(["reference": .string("ServiceRequest/\(serviceRequestId)")])
            ])
        }
        return obs
    }

    /// Converts this gaze session record into OMOP CDM `MEASUREMENT` tabular rows.
    public func toOMOPMeasurements(personId: String) -> [OMOPMeasurementRecord] {
        FHIRInteroperabilityMapper.makeOMOPMeasurements(from: self, personId: personId)
    }
}

// MARK: - Session History Store (data collection)

/// On-device history of completed gaze screenings, persisted as Codable JSON.
/// Feeds the dashboard trend badges, weekly summaries, and the doctor report.
/// De-identified: records carry the study participant token, never raw video.
@MainActor
public final class GazeSessionHistoryStore: ObservableObject {
    public static let shared = GazeSessionHistoryStore()

    /// Cap so the history file stays small on-device.
    public static let maxStoredRecords = 100

    @Published public private(set) var records: [GazeSessionRecord] = []

    private let storageKey = "org.curv.asd.gaze_session_history"
    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: storageKey),
           let decoded = try? JSONDecoder().decode([GazeSessionRecord].self, from: data) {
            self.records = decoded
        }
    }

    /// Records a completed session. Replaces any prior entry with the same
    /// session id (re-exports and retries must not double-count).
    public func record(_ record: GazeSessionRecord) {
        records.removeAll { $0.sessionId == record.sessionId }
        records.append(record)
        records.sort { $0.endedAt < $1.endedAt }
        if records.count > Self.maxStoredRecords {
            records.removeFirst(records.count - Self.maxStoredRecords)
        }
        persist()
    }

    public func fetchAll() -> [GazeSessionRecord] { records }

    public var lastRecord: GazeSessionRecord? { records.last }

    /// Sessions finished in the trailing 7 days (dashboard weekly badges).
    public func sessionsThisWeek(now: Date = Date()) -> [GazeSessionRecord] {
        let cutoff = now.addingTimeInterval(-7 * 24 * 3600)
        return records.filter { $0.endedAt >= cutoff }
    }

    public func clear() {
        records = []
        defaults.removeObject(forKey: storageKey)
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(records) {
            defaults.set(data, forKey: storageKey)
        }
    }
}
