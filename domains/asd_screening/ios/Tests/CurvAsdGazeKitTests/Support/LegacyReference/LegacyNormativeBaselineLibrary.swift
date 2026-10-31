// Former Swift body of `NormativeBaselineLibrary.validate` at dev@3718ea6, moved here by WP-A.11 (FFI-3).
// The production type now delegates to the Rust core; this copy is the parity oracle for
// one release (GoldenParityTests, LegacyParityTests). Do not change its behaviour.
@testable import CurvAsdGazeKit
import Foundation

enum LegacyNormativeBaselineLibrary {
    /// Validates coverage: every catalog clip id passed in has a profile,
    /// and every expectation resolves for every declared bracket. Returns human-readable problems.
    static func validate(_ set: NormativeBaselineSet, requiredVideoIds: [String]) -> [String] {
        var problems: [String] = []
        let bracketIds = set.ageBrackets.map { $0.id }
        for videoId in requiredVideoIds where set.videos[videoId] == nil {
            problems.append("missing video profile: \(videoId)")
        }
        for (videoId, video) in set.videos {
            if video.segments.isEmpty {
                problems.append("\(videoId): no segments")
            }
            for segment in video.segments {
                for expectation in segment.expectations {
                    for bracketId in bracketIds where expectation.range(forBracketId: bracketId) == nil {
                        problems.append("\(videoId)/\(segment.id)/\(expectation.biomarker): no range for bracket \(bracketId)")
                    }
                }
            }
        }
        return problems
    }
}
