// Former Swift body of `AttentionFlagAlgorithm` at dev@3718ea6, moved here by WP-A.11 (FFI-3).
// The production type now delegates to the Rust core; this copy is the parity oracle for
// one release (GoldenParityTests, LegacyParityTests). Do not change its behaviour.
@testable import CurvAsdGazeKit
import Foundation

enum LegacyAttentionFlagAlgorithm {
    static let greenFloor = 0.65
    static let yellowFloor = 0.35
    /// Share of scored clips below the quality gate that forces a recheck.
    static let lowConfidenceShare = 0.50
    /// Share of scored rows beyond the reference band that forces red even when the composite is high.
    static let atypicalShareForRed = 0.40

    static func assess(_ score: PerVideoSessionScore) -> AttentionFlagAssessment {
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
