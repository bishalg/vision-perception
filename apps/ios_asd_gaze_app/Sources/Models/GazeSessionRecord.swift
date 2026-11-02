import CurvAsdGazeKit
import Foundation
import os

/// High-level session record capturing gaze engagement metrics for caregiver review and export.
/// Conforms to our Zero Raw Video Recording guarantee: strictly mathematical features and temporal telemetry.
///
/// Schema v1.8 (SCH-5) = v1.7 plus an explicit scoring outcome:
/// - `scoringStatus` is `scored` or `unscorable`. The five score values
///   (`socialPreferenceScore`, `socialSmileIntensity`, `smileReactivityIndex`,
///   `flatAffectScore`, `surpriseScoreMean`) are all present when scored and all `nil`
///   (absent keys) when unscorable, which carries `unscorableReason` instead (CON-14).
/// - `bundleVersion` names the verified config bundle that scored the session (ING-2,
///   WP-A.5); absent when the build's bundled configuration scored it.
/// - `normCollection` is present only when the parent gave research consent and opted
///   into normative reference collection (`NormCollectionGate`); `consented` is then true.
///
/// Schema v1.7 (SIM-4, PLC-1/2, CNS-7, SCH-2):
/// - `captureSource` is required; simulated records are refused by every sink.
/// - `trackingQuality` (QS-1) and the per-clip tracking values in `perVideo[]` (QS-3) come
///   from the Rust gaze pipeline; absent when it did not run.
/// - `calibratedDistanceCm` is the in-session calibration pose distance (QS-5), absent when
///   no calibration ran in the session; never a constant.
/// - `eyeDwellRatio`, `mouthDwellRatio` and `firstEyeLatencyMs` are optional and `nil`
///   when not measured; nothing substitutes a default.
/// - `participantId` is always a pseudonymous token, never the child's name.
/// - Records decoded from v1.5 and earlier carry `legacyProvenance == true`.
public struct GazeSessionRecord: Identifiable, Codable, Sendable, Equatable {
    /// Schema written by this build.
    public static let currentSchemaVersion = "v1.8"

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

    /// SCH-5: `.unscorable` when on-device scoring produced no score values. Records
    /// before v1.8 always carried the numbers and decode as `.scored`.
    public let scoringStatus: ScoringStatus
    /// Why the session is unscorable; `nil` when scored.
    public let unscorableReason: UnscorableReason?

    // Gaze engagement metrics
    /// [-1.0, 1.0]: > 0 is social bias, < 0 is geometric pattern bias. `nil` when unscorable.
    public let socialPreferenceScore: Double?
    public let validFrameRatio: Double
    /// `nil` when no eye-or-mouth region hit occurred (PLC-2).
    public let eyeDwellRatio: Double?
    /// `nil` when no eye-or-mouth region hit occurred (PLC-2).
    public let mouthDwellRatio: Double?
    /// `nil` until a real first-fixation detector exists (PLC-1).
    public let firstEyeLatencyMs: Double?
    /// QS-5: measured calibration distance; `nil` when no calibration ran in the session.
    public let calibratedDistanceCm: Double?
    public let qualityVerdict: String

    // Facial affect engagement metrics (`nil` when unscorable)
    public let socialSmileIntensity: Double?
    public let smileReactivityIndex: Double?
    public let flatAffectScore: Double?
    public let surpriseScoreMean: Double?

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

    /// Session tracking-quality record (QS-1, v1.7), carried verbatim from the Rust pipeline;
    /// `nil` for records written before v1.7 or when the pipeline did not run.
    public let trackingQuality: JSONValue?

    /// Version of the verified config bundle that scored the session (`rawPayload.bundleVersion`,
    /// ING-2); `nil` when the bundled configuration scored it.
    public let bundleVersion: String?

    /// Norm-collection consent (`rawPayload.normCollection`, ING-3); `nil` unless the parent
    /// consented to research and opted into normative collection for a live session.
    public let normCollection: NormCollectionConsent?

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
        socialPreferenceScore: Double?,
        validFrameRatio: Double,
        eyeDwellRatio: Double?,
        mouthDwellRatio: Double?,
        firstEyeLatencyMs: Double?,
        calibratedDistanceCm: Double?,
        qualityVerdict: String,
        socialSmileIntensity: Double?,
        smileReactivityIndex: Double?,
        flatAffectScore: Double?,
        surpriseScoreMean: Double?,
        unscorableReason: UnscorableReason? = nil,
        notes: String? = nil,
        schemaVersion: String = GazeSessionRecord.currentSchemaVersion,
        zeroRawVideoEnforced: Bool = true,
        legacyProvenance: Bool? = nil,
        perVideoComposite: Double? = nil,
        perVideo: [PerVideoScoreReport]? = nil,
        trackingQuality: JSONValue? = nil,
        bundleVersion: String? = nil,
        normCollection: NormCollectionConsent? = nil
    ) {
        self.id = id
        self.sessionId = sessionId
        self.participantId = participantId
        self.captureSource = captureSource
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.durationSec = durationSec
        // Scored only with all five values and no reason; otherwise none of them (SCH-5).
        let scores = unscorableReason == nil ? SessionScores(
            socialPreferenceScore: socialPreferenceScore,
            socialSmileIntensityMean: socialSmileIntensity,
            smileReactivityIndex: smileReactivityIndex,
            flatAffectScore: flatAffectScore,
            surpriseScoreMean: surpriseScoreMean
        ) : nil
        self.scoringStatus = scores == nil ? .unscorable : .scored
        self.unscorableReason = scores == nil ? (unscorableReason ?? .other) : nil
        self.socialPreferenceScore = scores?.socialPreferenceScore
        self.validFrameRatio = validFrameRatio
        self.eyeDwellRatio = eyeDwellRatio
        self.mouthDwellRatio = mouthDwellRatio
        self.firstEyeLatencyMs = firstEyeLatencyMs
        self.calibratedDistanceCm = calibratedDistanceCm
        self.qualityVerdict = qualityVerdict
        self.socialSmileIntensity = scores?.socialSmileIntensityMean
        self.smileReactivityIndex = scores?.smileReactivityIndex
        self.flatAffectScore = scores?.flatAffectScore
        self.surpriseScoreMean = scores?.surpriseScoreMean
        self.notes = notes
        self.schemaVersion = schemaVersion
        self.zeroRawVideoEnforced = zeroRawVideoEnforced
        self.legacyProvenance = legacyProvenance
        self.perVideoComposite = perVideoComposite
        self.perVideo = perVideo
        self.trackingQuality = trackingQuality
        self.bundleVersion = bundleVersion
        self.normCollection = normCollection
    }

    /// Builds a record from a finished session summary. `participantToken` must be
    /// a pseudonymous token (see `participantToken(enrollment:profileToken:)`), never a
    /// name; the label differs from the stored property on purpose so call sites that
    /// passed a name fail to compile (CNS-7).
    ///
    /// v1.7: `trackingQuality` and `calibratedDistanceCm` (QS-5) come from the summary, and
    /// each score report takes its clip's tracking values (QS-3) by clip id and play order.
    public init(
        summary: GazeSessionReportSummary,
        participantToken: String,
        notes: String? = nil,
        perVideoComposite: Double? = nil,
        perVideo: [PerVideoScoreReport]? = nil,
        bundleVersion: String? = nil,
        normCollection: NormCollectionConsent? = nil
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
            calibratedDistanceCm: summary.calibratedDistanceCm,
            qualityVerdict: summary.qualityVerdict.rawValue,
            socialSmileIntensity: summary.socialSmileIntensityMean,
            smileReactivityIndex: summary.smileReactivityIndex,
            flatAffectScore: summary.flatAffectScore,
            surpriseScoreMean: summary.surpriseScoreMean,
            unscorableReason: summary.scoringStatus == .unscorable ? (summary.unscorableReason ?? .other) : nil,
            notes: notes,
            perVideoComposite: perVideoComposite,
            perVideo: perVideo.map { reports in
                summary.clipTracking.map { PerVideoScoreReport.merging(reports, clipTracking: $0) } ?? reports
            },
            trackingQuality: summary.trackingQuality,
            bundleVersion: bundleVersion,
            normCollection: normCollection
        )
    }

    /// The pseudonymous participant token for a session (CNS-7, REC-11): the study
    /// participant id when the family is enrolled and consented, otherwise the child
    /// profile's stable token (`ChildProfileStore.participantToken()`), so every session
    /// of one child lands on one participant. Never derived from the session id.
    public static func participantToken(enrollment: ResearchStudyEnrollment?, profileToken: String) -> String {
        if let enrollment, enrollment.isConsented, !enrollment.participantId.isEmpty {
            return enrollment.participantId
        }
        return profileToken
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey {
        case id, sessionId, participantId, captureSource, startedAt, endedAt, durationSec
        case scoringStatus, unscorableReason, bundleVersion, normCollection
        case socialPreferenceScore, validFrameRatio, eyeDwellRatio, mouthDwellRatio, firstEyeLatencyMs
        case calibratedDistanceCm, qualityVerdict
        case socialSmileIntensity, smileReactivityIndex, flatAffectScore, surpriseScoreMean
        case notes, schemaVersion, zeroRawVideoEnforced, legacyProvenance
        case perVideoComposite, perVideo, trackingQuality
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
        // SCH-5: absent before v1.8, when every record carried the five numbers.
        let scoringStatus = try c.decodeIfPresent(ScoringStatus.self, forKey: .scoringStatus) ?? .scored
        let unscorableReason = try c.decodeIfPresent(UnscorableReason.self, forKey: .unscorableReason)

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
            socialPreferenceScore: try c.decodeIfPresent(Double.self, forKey: .socialPreferenceScore),
            validFrameRatio: try c.decode(Double.self, forKey: .validFrameRatio),
            eyeDwellRatio: try c.decodeIfPresent(Double.self, forKey: .eyeDwellRatio),
            mouthDwellRatio: try c.decodeIfPresent(Double.self, forKey: .mouthDwellRatio),
            firstEyeLatencyMs: try c.decodeIfPresent(Double.self, forKey: .firstEyeLatencyMs),
            // Required up to v1.6; optional from v1.7 (QS-5).
            calibratedDistanceCm: try c.decodeIfPresent(Double.self, forKey: .calibratedDistanceCm),
            qualityVerdict: try c.decode(String.self, forKey: .qualityVerdict),
            socialSmileIntensity: try c.decodeIfPresent(Double.self, forKey: .socialSmileIntensity),
            smileReactivityIndex: try c.decodeIfPresent(Double.self, forKey: .smileReactivityIndex),
            flatAffectScore: try c.decodeIfPresent(Double.self, forKey: .flatAffectScore),
            surpriseScoreMean: try c.decodeIfPresent(Double.self, forKey: .surpriseScoreMean),
            unscorableReason: scoringStatus == .unscorable ? (unscorableReason ?? .other) : nil,
            notes: try c.decodeIfPresent(String.self, forKey: .notes),
            schemaVersion: schemaVersion,
            zeroRawVideoEnforced: zeroRawVideoEnforced,
            legacyProvenance: legacyProvenance,
            perVideoComposite: try c.decodeIfPresent(Double.self, forKey: .perVideoComposite),
            perVideo: try c.decodeIfPresent([PerVideoScoreReport].self, forKey: .perVideo),
            trackingQuality: try c.decodeIfPresent(JSONValue.self, forKey: .trackingQuality),
            bundleVersion: try c.decodeIfPresent(String.self, forKey: .bundleVersion),
            normCollection: try c.decodeIfPresent(NormCollectionConsent.self, forKey: .normCollection)
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

// MARK: - Norm-collection consent (ING-3)

/// `rawPayload.normCollection`: the parent's normative-collection consent for one session.
/// Built only from `NormCollectionGate` metadata, so it exists only when research consent is
/// active, the parent opted in, and the capture is live TrueDepth; `consented` is then true.
/// There is no way to build one with `consented: false` or by default.
public struct NormCollectionConsent: Codable, Sendable, Equatable {
    public let consented: Bool
    public let consentVersion: String
    public let studyIdentifier: String
    public let childAgeMonths: Int
    public let collectedAtUtc: String

    /// `nil` when the gate is closed (`metadata == nil`) or the capture is simulated.
    public init?(metadata: NormCollectionMetadata?) {
        guard let metadata, !metadata.isSimulator else { return nil }
        consented = true
        consentVersion = metadata.consentVersion
        studyIdentifier = metadata.studyIdentifier
        childAgeMonths = metadata.childAgeMonths
        collectedAtUtc = metadata.collectedAtUtc
    }
}

// MARK: - FHIR R4 Serialization (FHIR-1, ADR-0003)

extension GazeSessionRecord {
    /// The FHIR R4 export unit for this session: a `Bundle` of `type: collection` built by
    /// `FHIRBundleBuilder`. A refused record (simulated or rejected, SIM-5/FHIR-2) yields a bundle
    /// without a gaze Observation; check `FHIRBundleBuilder.exportRefusalReason(for:)` first.
    public func toFHIRBundle(
        mchat: (MCHATScoreResult, [Int: Bool])? = nil,
        extraResources: [[String: AnyCodableValue]] = [],
        subject: FHIRSubject
    ) -> [String: AnyCodableValue] {
        FHIRBundleBuilder.bundle(for: self, mchat: mchat, extraResources: extraResources, subject: subject)
    }

    /// A single standalone `Observation` with a bare `Patient/<participantId>` subject.
    /// Returns `nil` for refused records (SIM-5, FHIR-2).
    @available(*, deprecated, message: "Export FHIRBundleBuilder.bundle(for:mchat:extraResources:subject:) (ADR-0003)")
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

