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
public enum AttentionFlagAlgorithm {
    public static let greenFloor = 0.65
    public static let yellowFloor = 0.35
    /// Share of scored clips below the quality gate that forces a recheck.
    public static let lowConfidenceShare = 0.50
    /// Share of scored rows beyond the reference band that forces red even when the composite is high.
    public static let atypicalShareForRed = 0.40

    public static func assess(_ score: PerVideoSessionScore) -> AttentionFlagAssessment {
        let scored = score.videos.filter(\.hasBaseline)
        guard !scored.isEmpty else {
            return AttentionFlagAssessment(
                flag: .yellow,
                headline: "Not enough scored clips",
                detail: "Run a full stimulus set before reading a flag. This is a screening signal, not a diagnosis."
            )
        }

        let lowShare = Double(scored.filter(\.lowConfidence).count) / Double(scored.count)
        if lowShare >= lowConfidenceShare {
            return AttentionFlagAssessment(
                flag: .yellow,
                headline: "Recheck — looking time was unreliable",
                detail: "Too many clips fell below the quality gate. Seat the child again and repeat. This is a screening signal, not a diagnosis."
            )
        }

        let rows = scored.flatMap(\.rows)
        let atypicalShare = rows.isEmpty
            ? 0
            : Double(rows.filter { $0.status == .atypical }.count) / Double(rows.count)

        if score.compositeScore >= greenFloor && atypicalShare < atypicalShareForRed {
            return AttentionFlagAssessment(
                flag: .green,
                headline: "Looking matches the age reference",
                detail: "Looking time sits in the expected range for this age. This is a screening signal, not a diagnosis."
            )
        }

        if score.compositeScore < yellowFloor || atypicalShare >= atypicalShareForRed {
            return AttentionFlagAssessment(
                flag: .red,
                headline: "Looking differs from the age reference",
                detail: "Share this result with your pediatrician at the next visit. This is a screening signal, not a diagnosis."
            )
        }

        return AttentionFlagAssessment(
            flag: .yellow,
            headline: "Some clips differ from the age reference",
            detail: "A repeat session is worth doing. This is a screening signal, not a diagnosis."
        )
    }
}
