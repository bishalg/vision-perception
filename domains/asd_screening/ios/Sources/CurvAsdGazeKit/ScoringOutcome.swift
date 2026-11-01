import Foundation

/// Whether on-device scoring produced a session's scores (session schema 1.8, SCH-5).
///
/// `unscorable`: scoring did not run or was refused, and none of the five score fields
/// (`socialPreferenceScore`, `socialSmileIntensity`, `smileReactivityIndex`,
/// `flatAffectScore`, `surpriseScoreMean`) carries a value (CON-14: never a placeholder).
public enum ScoringStatus: String, Codable, Sendable, Equatable {
    case scored
    case unscorable
}

/// Why a session is `unscorable` (`asd-contract::session_v1_8::UnscorableReason`).
public enum UnscorableReason: String, Codable, Sendable, Equatable, CaseIterable {
    /// The core refused the session aggregation (e.g. a non-finite input).
    case aggregationFailed = "aggregation_failed"
    /// Too few valid gaze samples to compute a score.
    case insufficientValidGaze = "insufficient_valid_gaze"
    /// Tracking quality verdict `unusable`.
    case trackingUnusable = "tracking_unusable"
    /// Calibration failed and no score may be computed without it.
    case calibrationFailed = "calibration_failed"
    /// No clip of the protocol could be scored.
    case noScorableClips = "no_scorable_clips"
    case other
}

/// The five session score values, present together or not at all.
public struct SessionScores: Codable, Sendable, Equatable {
    public let socialPreferenceScore: Double
    public let socialSmileIntensityMean: Double
    public let smileReactivityIndex: Double
    public let flatAffectScore: Double
    public let surpriseScoreMean: Double

    public init(
        socialPreferenceScore: Double,
        socialSmileIntensityMean: Double,
        smileReactivityIndex: Double,
        flatAffectScore: Double,
        surpriseScoreMean: Double
    ) {
        self.socialPreferenceScore = socialPreferenceScore
        self.socialSmileIntensityMean = socialSmileIntensityMean
        self.smileReactivityIndex = smileReactivityIndex
        self.flatAffectScore = flatAffectScore
        self.surpriseScoreMean = surpriseScoreMean
    }

    /// All five when every value is present and finite, else `nil`.
    public init?(
        socialPreferenceScore: Double?,
        socialSmileIntensityMean: Double?,
        smileReactivityIndex: Double?,
        flatAffectScore: Double?,
        surpriseScoreMean: Double?
    ) {
        guard let p = socialPreferenceScore, let s = socialSmileIntensityMean,
              let r = smileReactivityIndex, let f = flatAffectScore, let u = surpriseScoreMean,
              [p, s, r, f, u].allSatisfy(\.isFinite) else { return nil }
        self.init(
            socialPreferenceScore: p,
            socialSmileIntensityMean: s,
            smileReactivityIndex: r,
            flatAffectScore: f,
            surpriseScoreMean: u
        )
    }
}
