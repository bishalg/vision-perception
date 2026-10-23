import Foundation

// MARK: - Normative Baseline Model

/// Age-conditional expected range for one biomarker expectation.
/// `higherBetter` uses `acceptableLow`; `lowerBetter` uses `acceptableHigh`; `band` uses both.
public struct NormativeRange: Codable, Sendable, Equatable {
    public let expected: Double?
    public let acceptableLow: Double?
    public let acceptableHigh: Double?

    public init(expected: Double? = nil, acceptableLow: Double? = nil, acceptableHigh: Double? = nil) {
        self.expected = expected
        self.acceptableLow = acceptableLow
        self.acceptableHigh = acceptableHigh
    }
}

/// Direction of "better" for an expectation; drives status and deviation scoring.
public enum NormativeDirection: String, Codable, Sendable {
    case higherBetter = "higherBetter"
    case lowerBetter = "lowerBetter"
    case band = "band"
}

/// One expected-response row inside a stimulus segment: which metric, which AOI,
/// what range per age bracket, and where the number came from.
public struct NormativeExpectation: Codable, Sendable {
    public let biomarker: String
    public let direction: NormativeDirection
    public let targetAOI: String?
    /// Keyed by age-bracket id ("18-23") or `"default"`; bracket id wins over default.
    public let byAge: [String: NormativeRange]
    public let citation: String
    /// "literature" or "interpolated" — surfaced in UI and exports so provisional numbers are never passed off as measured norms.
    public let source: String
    public let note: String?

    /// Range for a bracket id, falling back to `default`.
    public func range(forBracketId bracketId: String) -> NormativeRange? {
        byAge[bracketId] ?? byAge["default"]
    }
}

/// A time-coded chunk of a stimulus video: what is shown, when, and what response is expected.
public struct NormativeSegment: Codable, Sendable {
    public let id: String
    public let description: String
    public let startSec: Double
    public let endSec: Double
    public let expectations: [NormativeExpectation]
}

/// Per-video normative profile: presentation metadata, composite weight, and segment expectations.
public struct NormativeVideo: Codable, Sendable {
    public let title: String
    public let category: String
    /// Relative contribution to the session composite when this clip ran (normalized over present clips).
    public let weight: Double
    public let segments: [NormativeSegment]
}

/// Machine-readable, versioned baseline set bundled with the app.
public struct NormativeBaselineSet: Codable, Sendable {
    public struct AgeBracket: Codable, Sendable {
        public let id: String
        public let minMonths: Int
        public let maxMonths: Int
    }

    public let schemaVersion: String
    public let status: String
    public let provenance: String?
    public let ageBrackets: [AgeBracket]
    public let videos: [String: NormativeVideo]

    /// Bracket id containing the given age in months; nil when outside all brackets.
    public func bracketId(forAgeMonths ageMonths: Int) -> String? {
        ageBrackets.first { ageMonths >= $0.minMonths && ageMonths <= $0.maxMonths }?.id
    }

    public func video(_ stimulusId: String) -> NormativeVideo? {
        videos[stimulusId]
    }
}

// MARK: - Library

/// Loads and resolves normative baseline sets.
///
/// The bundled set is `provisional-literature-seeded`: literature-anchored values with explicit
/// citations, plus clearly-marked developmental interpolations. A candidate set produced by the
/// norm-aggregation pipeline (`tools/asd_norm_aggregator.py`) can be loaded over it after review.
public enum NormativeBaselineLibrary {

    public static let bundledResourceName = "normative_baselines"

    /// Loads the bundled JSON shipped as an SPM package resource.
    public static func loadBundled() -> NormativeBaselineSet? {
        if let url = Bundle.module.url(forResource: bundledResourceName, withExtension: "json") {
            return load(from: url)
        }
        // Fallback for build setups where package resources land in the main bundle.
        if let url = Bundle.main.url(forResource: bundledResourceName, withExtension: "json") {
            return load(from: url)
        }
        return nil
    }

    public static func load(from url: URL) -> NormativeBaselineSet? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return decode(data)
    }

    public static func decode(_ data: Data) -> NormativeBaselineSet? {
        let decoder = JSONDecoder()
        return try? decoder.decode(NormativeBaselineSet.self, from: data)
    }

    /// Validates coverage: every catalog clip id passed in has a profile,
    /// and every expectation resolves for every declared bracket. Returns human-readable problems.
    public static func validate(_ set: NormativeBaselineSet, requiredVideoIds: [String]) -> [String] {
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
