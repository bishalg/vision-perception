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
