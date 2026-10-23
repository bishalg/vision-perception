import CurvAsdGazeKit
import Foundation
import os

/// High-level session record capturing gaze engagement metrics for caregiver review and export.
/// Conforms to our Zero Raw Video Recording guarantee: strictly mathematical features and temporal telemetry.
///
/// Schema v1.6 (SIM-4, PLC-1/2, CNS-7):
/// - `captureSource` is required; simulated records are refused by every sink.
/// - `eyeDwellRatio`, `mouthDwellRatio` and `firstEyeLatencyMs` are optional and `nil`
///   when not measured; nothing substitutes a default.
/// - `participantId` is always a pseudonymous token, never the child's name.
/// - Records decoded from v1.5 and earlier carry `legacyProvenance == true`.
public struct GazeSessionRecord: Identifiable, Codable, Sendable, Equatable {
    /// Schema written by this build.
    public static let currentSchemaVersion = "v1.6"

    public let id: String
    public let sessionId: String
    /// Pseudonymous participant token (`ResearchStudyEnrollment.participantId` when
    /// enrolled, else `child-<sessionId prefix 8>`). Never a name (CNS-7).
    public let participantId: String
    /// Capture provenance (SIM-4).
    public let captureSource: CaptureSource
    public let startedAt: Date
    public let endedAt: Date
    public let durationSec: Double

    // Gaze engagement metrics
    public let socialPreferenceScore: Double // [-1.0, 1.0]: > 0 is social bias, < 0 is geometric pattern bias
    public let validFrameRatio: Double
    /// `nil` when no eye-or-mouth region hit occurred (PLC-2).
    public let eyeDwellRatio: Double?
    /// `nil` when no eye-or-mouth region hit occurred (PLC-2).
    public let mouthDwellRatio: Double?
    /// `nil` until a real first-fixation detector exists (PLC-1).
    public let firstEyeLatencyMs: Double?
    public let calibratedDistanceCm: Double
    public let qualityVerdict: String

    // Facial affect engagement metrics
    public let socialSmileIntensity: Double
    public let smileReactivityIndex: Double
    public let flatAffectScore: Double
    public let surpriseScoreMean: Double

    // Metadata
    public let notes: String?
    public let schemaVersion: String
    public let zeroRawVideoEnforced: Bool
    /// `true` when the record predates v1.6 and its `captureSource` was inferred on
    /// decode rather than recorded (SIM-4). `nil` for records written with provenance.
    public let legacyProvenance: Bool?

    // Per-Stimulus Matrix (v1.5, additive; nil for legacy records)
    public let perVideoComposite: Double?
    public let perVideo: [PerVideoScoreReport]?

    /// Designated initialiser. Every metric and the capture source are explicit;
    /// test defaults live in the test target (`GazeSessionRecord.fixture`).
    public init(
        id: String = UUID().uuidString,
        sessionId: String,
        participantId: String,
        captureSource: CaptureSource,
        startedAt: Date,
        endedAt: Date,
        durationSec: Double,
        socialPreferenceScore: Double,
        validFrameRatio: Double,
        eyeDwellRatio: Double?,
        mouthDwellRatio: Double?,
        firstEyeLatencyMs: Double?,
        calibratedDistanceCm: Double,
        qualityVerdict: String,
        socialSmileIntensity: Double,
        smileReactivityIndex: Double,
        flatAffectScore: Double,
        surpriseScoreMean: Double,
        notes: String? = nil,
        schemaVersion: String = GazeSessionRecord.currentSchemaVersion,
        zeroRawVideoEnforced: Bool = true,
        legacyProvenance: Bool? = nil,
        perVideoComposite: Double? = nil,
        perVideo: [PerVideoScoreReport]? = nil
    ) {
        self.id = id
        self.sessionId = sessionId
        self.participantId = participantId
        self.captureSource = captureSource
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
        self.legacyProvenance = legacyProvenance
        self.perVideoComposite = perVideoComposite
        self.perVideo = perVideo
    }

    /// Builds a record from a finished session summary. `participantToken` must be
    /// a pseudonymous token (see `participantToken(enrollment:sessionId:)`), never a
    /// name; the label differs from the stored property on purpose so call sites that
    /// passed a name fail to compile (CNS-7).
    public init(
        summary: GazeSessionReportSummary,
        participantToken: String,
        calibratedDistanceCm: Double = 35.0,
        notes: String? = nil,
        perVideoComposite: Double? = nil,
        perVideo: [PerVideoScoreReport]? = nil
    ) {
        let now = Date()
        let start = now.addingTimeInterval(-summary.durationSec)
        self.init(
            sessionId: summary.sessionId,
            participantId: participantToken,
            captureSource: summary.captureSource,
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
            notes: notes,
            perVideoComposite: perVideoComposite,
            perVideo: perVideo
        )
    }

    /// The pseudonymous participant token for a session (CNS-7): the study
    /// participant id when the family is enrolled and consented, otherwise
    /// `child-<first 8 characters of the session id>`.
    public static func participantToken(enrollment: ResearchStudyEnrollment?, sessionId: String) -> String {
        if let enrollment, enrollment.isConsented, !enrollment.participantId.isEmpty {
            return enrollment.participantId
        }
        return "child-\(sessionId.prefix(8))"
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey {
        case id, sessionId, participantId, captureSource, startedAt, endedAt, durationSec
        case socialPreferenceScore, validFrameRatio, eyeDwellRatio, mouthDwellRatio, firstEyeLatencyMs
        case calibratedDistanceCm, qualityVerdict
        case socialSmileIntensity, smileReactivityIndex, flatAffectScore, surpriseScoreMean
        case notes, schemaVersion, zeroRawVideoEnforced, legacyProvenance
        case perVideoComposite, perVideo
    }

    /// Decodes v1.6 records strictly and older records with an inferred provenance
    /// (SIM-4): a pre-v1.6 record without `captureSource` decodes as `.trueDepth`
    /// only when it asserted `zeroRawVideoEnforced == true`; otherwise its provenance
    /// is unknown and it is quarantined as `.simulated`. Either way it is marked
    /// `legacyProvenance = true`. A v1.6+ record without `captureSource` fails.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let schemaVersion = try c.decode(String.self, forKey: .schemaVersion)
        let zeroRawVideoEnforced = try c.decodeIfPresent(Bool.self, forKey: .zeroRawVideoEnforced) ?? false

        let captureSource: CaptureSource
        let legacyProvenance: Bool?
        if let recorded = try c.decodeIfPresent(CaptureSource.self, forKey: .captureSource) {
            captureSource = recorded
            legacyProvenance = try c.decodeIfPresent(Bool.self, forKey: .legacyProvenance)
        } else if Self.predatesProvenance(schemaVersion) {
            captureSource = zeroRawVideoEnforced ? .trueDepth : .simulated
            legacyProvenance = true
        } else {
            throw DecodingError.keyNotFound(
                CodingKeys.captureSource,
                DecodingError.Context(
                    codingPath: c.codingPath,
                    debugDescription: "schema \(schemaVersion) requires captureSource"
                )
            )
        }

        self.init(
            id: try c.decode(String.self, forKey: .id),
            sessionId: try c.decode(String.self, forKey: .sessionId),
            participantId: try c.decode(String.self, forKey: .participantId),
            captureSource: captureSource,
            startedAt: try c.decode(Date.self, forKey: .startedAt),
            endedAt: try c.decode(Date.self, forKey: .endedAt),
            durationSec: try c.decode(Double.self, forKey: .durationSec),
            socialPreferenceScore: try c.decode(Double.self, forKey: .socialPreferenceScore),
            validFrameRatio: try c.decode(Double.self, forKey: .validFrameRatio),
            eyeDwellRatio: try c.decodeIfPresent(Double.self, forKey: .eyeDwellRatio),
            mouthDwellRatio: try c.decodeIfPresent(Double.self, forKey: .mouthDwellRatio),
            firstEyeLatencyMs: try c.decodeIfPresent(Double.self, forKey: .firstEyeLatencyMs),
            calibratedDistanceCm: try c.decode(Double.self, forKey: .calibratedDistanceCm),
            qualityVerdict: try c.decode(String.self, forKey: .qualityVerdict),
            socialSmileIntensity: try c.decode(Double.self, forKey: .socialSmileIntensity),
            smileReactivityIndex: try c.decode(Double.self, forKey: .smileReactivityIndex),
            flatAffectScore: try c.decode(Double.self, forKey: .flatAffectScore),
            surpriseScoreMean: try c.decode(Double.self, forKey: .surpriseScoreMean),
            notes: try c.decodeIfPresent(String.self, forKey: .notes),
            schemaVersion: schemaVersion,
            zeroRawVideoEnforced: zeroRawVideoEnforced,
            legacyProvenance: legacyProvenance,
            perVideoComposite: try c.decodeIfPresent(Double.self, forKey: .perVideoComposite),
            perVideo: try c.decodeIfPresent([PerVideoScoreReport].self, forKey: .perVideo)
        )
    }

    /// `true` for schema versions older than v1.6 ("v1.5", "v1.4", ...). Unparseable
    /// versions are treated as current, so they must carry `captureSource`.
    static func predatesProvenance(_ schemaVersion: String) -> Bool {
        let digits = schemaVersion.drop { !$0.isNumber }
        let parts = digits.split(separator: ".").map { Int($0) }
        guard let major = parts.first ?? nil else { return false }
        let minor = parts.count > 1 ? (parts[1] ?? 0) : 0
        return major < 1 || (major == 1 && minor < 6)
    }
}

// MARK: - FHIR R4 Serialization (STAGE 2)

extension GazeSessionRecord {
    /// Converts this gaze session record into an HL7 FHIR Release 4 `Observation` resource dictionary.
    /// Includes USCDI v5 `basedOn` linkage for ServiceRequest traceability.
    /// Returns `nil` for simulated records (SIM-5).
    public func toFHIRObservation(basedOn serviceRequestId: String? = nil) -> [String: AnyCodableValue]? {
        guard var obs = FHIRInteroperabilityMapper.makeFHIRObservation(from: self, participantId: participantId) else {
            return nil
        }
        if let serviceRequestId = serviceRequestId {
            obs["basedOn"] = .array([
                .dictionary(["reference": .string("ServiceRequest/\(serviceRequestId)")])
            ])
        }
        return obs
    }

    /// Converts this gaze session record into OMOP CDM `MEASUREMENT` tabular rows
    /// (empty for simulated records, SIM-5).
    public func toOMOPMeasurements(personId: String) -> [OMOPMeasurementRecord] {
        FHIRInteroperabilityMapper.makeOMOPMeasurements(from: self, personId: personId)
    }
}

// MARK: - Session History Store (data collection)

/// On-device history of completed gaze screenings, persisted as Codable JSON.
/// Feeds the dashboard trend badges, weekly summaries, and the doctor report.
/// De-identified: records carry the study participant token, never raw video.
/// Simulated sessions are never stored (SIM-5), including ones persisted by
/// earlier builds, which are dropped on load.
@MainActor
public final class GazeSessionHistoryStore: ObservableObject {
    public static let shared = GazeSessionHistoryStore()

    /// Cap so the history file stays small on-device.
    public static let maxStoredRecords = 100

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "GazeSessionHistoryStore")

    @Published public private(set) var records: [GazeSessionRecord] = []

    private let storageKey = "org.curv.asd.gaze_session_history"
    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: storageKey),
           let decoded = try? JSONDecoder().decode([GazeSessionRecord].self, from: data) {
            self.records = decoded.filter { $0.captureSource != .simulated }
        }
    }

    /// Records a completed session. Replaces any prior entry with the same
    /// session id (re-exports and retries must not double-count).
    public func record(_ record: GazeSessionRecord) {
        guard record.captureSource != .simulated else {
            Self.logger.info("Ignoring simulated session \(record.sessionId, privacy: .public) for history")
            return
        }
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
