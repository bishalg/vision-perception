import Foundation

/// Caregiver-facing traffic light for one scored gaze session.
///
/// Green, yellow, and red summarize how looking time sits against the age reference.
/// The flag is a screening signal. It is not a diagnosis.
public enum AttentionFlag: String, Codable, Sendable, Equatable, CaseIterable {
    case green
    case yellow
    case red

    public var title: String {
        switch self {
        case .green: return "Green"
        case .yellow: return "Yellow"
        case .red: return "Red"
        }
    }
}

public struct AttentionFlagAssessment: Codable, Sendable, Equatable {
    public let flag: AttentionFlag
    public let headline: String
    public let detail: String
    /// Always true. Exports keep the screening-only boundary attached to the flag.
    public let isScreeningSignalOnly: Bool

    public init(flag: AttentionFlag, headline: String, detail: String) {
        self.flag = flag
        self.headline = headline
        self.detail = detail
        self.isScreeningSignalOnly = true
    }
}

/// Rolls a per-stimulus session score into one green / yellow / red flag.
///
/// Unreliable looking time stays yellow, so a thin sample cannot paint green or red.
/// A strong composite with few atypical rows is green. A weak composite, or a large
/// share of atypical rows, is red. Everything between is yellow.
///
/// The rule and its wording live in the Rust core (`asd-core::flag`, CORE-4), read with the
/// bundled default `ScoringProfile`; this type keeps the public Swift API (FFI-3). The
/// constants below mirror that profile's `flag` thresholds (checked by a test).
public enum AttentionFlagAlgorithm {
    public static let greenFloor = 0.65
    public static let yellowFloor = 0.35
    /// Share of scored clips below the quality gate that forces a recheck.
    public static let lowConfidenceShare = 0.50
    /// Share of scored rows beyond the reference band that forces red even when the composite is high.
    public static let atypicalShareForRed = 0.40

    public static func assess(_ score: PerVideoSessionScore) -> AttentionFlagAssessment {
        do {
            return try AsdCoreBridge.assessFlag(score)
        } catch {
            AsdCoreBridge.report("assess_flag", error)
            // Never green or red without the core's verdict.
            return AttentionFlagAssessment(
                flag: .yellow,
                headline: "Flag not available",
                detail: "This result could not be read. Run the session again. This is a screening signal, not a diagnosis."
            )
        }
    }
}
