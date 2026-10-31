import Foundation

// MARK: - Score Models

public enum ExpectationStatus: String, Codable, Sendable {
    case onTarget
    case monitor
    case atypical
}

/// One scored expected-vs-recorded cell of the per-stimulus matrix.
public struct PerVideoScoreRow: Codable, Sendable, Equatable {
    public let segmentId: String
    public let segmentDescription: String
    public let biomarker: String
    public let targetAOI: String?
    /// Composed metrics lookup key (`biomarker` or `biomarker::AOI`).
    public let metricKey: String
    public let direction: NormativeDirection
    /// Recorded value; latency keys missing from the metric set are imputed at clip duration (`imputed`).
    public let observed: Double
    public let expected: Double?
    public let acceptableLow: Double?
    public let acceptableHigh: Double?
    public let status: ExpectationStatus
    /// Continuous goodness in [0, 1]; 1 = meets expectation.
    public let score: Double
    /// True when `observed` was imputed because the clip never produced the metric.
    public let imputed: Bool
    public let citation: String
    public let source: String

    public init(
        segmentId: String,
        segmentDescription: String,
        biomarker: String,
        targetAOI: String?,
        metricKey: String,
        direction: NormativeDirection,
        observed: Double,
        expected: Double?,
        acceptableLow: Double?,
        acceptableHigh: Double?,
        status: ExpectationStatus,
        score: Double,
        imputed: Bool,
        citation: String,
        source: String
    ) {
        self.segmentId = segmentId
        self.segmentDescription = segmentDescription
        self.biomarker = biomarker
        self.targetAOI = targetAOI
        self.metricKey = metricKey
        self.direction = direction
        self.observed = observed
        self.expected = expected
        self.acceptableLow = acceptableLow
        self.acceptableHigh = acceptableHigh
        self.status = status
        self.score = score
        self.imputed = imputed
        self.citation = citation
        self.source = source
    }
}

/// Expected-vs-recorded result for one stimulus clip.
public struct PerVideoScoreReport: Codable, Sendable, Equatable {
    public let stimulusId: String
    public let title: String
    public let category: String
    public let rows: [PerVideoScoreRow]
    /// Mean of row scores in [0, 1]; 0 when there is no baseline profile.
    public let subscore: Double
    public let validSampleRatio: Double
    /// True when clip valid-sample ratio fell below the quality gate — treat scores as indicative only.
    public let lowConfidence: Bool
    /// False when no baseline profile exists for the clip (untagged frames or unknown id).
    public let hasBaseline: Bool
    public let weight: Double

    public init(
        stimulusId: String,
        title: String,
        category: String,
        rows: [PerVideoScoreRow],
        subscore: Double,
        validSampleRatio: Double,
        lowConfidence: Bool,
        hasBaseline: Bool,
        weight: Double
    ) {
        self.stimulusId = stimulusId
        self.title = title
        self.category = category
        self.rows = rows
        self.subscore = subscore
        self.validSampleRatio = validSampleRatio
        self.lowConfidence = lowConfidence
        self.hasBaseline = hasBaseline
        self.weight = weight
    }
}

/// Session-level per-stimulus matrix with the weighted composite score.
public struct PerVideoSessionScore: Codable, Sendable, Equatable {
    public let ageBracketId: String
    public let ageMonths: Int
    /// Weighted mean of per-video subscores over clips with baseline profiles, in [0, 1].
    public let compositeScore: Double
    public let videos: [PerVideoScoreReport]
    public let baselineStatus: String
    public let baselineSchemaVersion: String
    public let scoredAtUtc: String
}

// MARK: - Engine

/// Compares per-video metrics against age-conditional normative expectations and produces
/// the expected-vs-recorded matrix plus a weighted composite score.
///
/// Status bands: `onTarget` meets the expected range, `monitor` falls in the tolerance zone,
/// `atypical` is beyond it. Continuous scores are piecewise-linear in [0, 1] with 0 at one
/// band-width beyond the tolerance edge. Composite weights come from the baseline set and are
/// normalized over the clips that actually ran.
///
/// The maths lives in the Rust core (`asd-core::scoring`, CORE-1..3); this type keeps the
/// public Swift API and delegates (FFI-3).
public enum PerVideoScoringEngine {

    /// Clips below this valid-sample ratio are marked low-confidence (matches the 0.60 vault gate).
    /// Mirrors `ScoringProfile.lowConfidenceValidRatio` of the Rust core (checked by a test).
    public static let lowConfidenceValidRatio: Double = 0.60

    public static func score(
        metricSets: [PerVideoMetricSet],
        baseline: NormativeBaselineSet,
        ageMonths: Int,
        frames: [GazeFrame] = [],
        scoredAt: Date = Date()
    ) -> PerVideoSessionScore {
        score(
            metricSets: metricSets,
            baseline: baseline,
            ageMonths: ageMonths,
            coreFrames: frames.map { CoreFrame($0) },
            scoredAtUtc: AsdCoreBridge.formatScoredAt(scoredAt)
        )
    }

    static func score(
        metricSets: [PerVideoMetricSet],
        baseline: NormativeBaselineSet,
        ageMonths: Int,
        coreFrames: [CoreFrame],
        scoredAtUtc: String
    ) -> PerVideoSessionScore {
        do {
            return try AsdCoreBridge.scoreSession(
                metricSets: metricSets,
                baseline: baseline,
                ageMonths: ageMonths,
                frames: coreFrames,
                scoredAtUtc: scoredAtUtc
            )
        } catch {
            AsdCoreBridge.report("score_session", error)
            // No scored clip: the flag reads "not enough scored clips" instead of a guess.
            return PerVideoSessionScore(
                ageBracketId: "default",
                ageMonths: ageMonths,
                compositeScore: 0,
                videos: [],
                baselineStatus: baseline.status,
                baselineSchemaVersion: baseline.schemaVersion,
                scoredAtUtc: scoredAtUtc
            )
        }
    }

    /// Bracket id for an age, clamped to the nearest declared bracket for out-of-range ages.
    public static func resolvedBracketId(baseline: NormativeBaselineSet, ageMonths: Int) -> String {
        // The core resolves the bracket while scoring; score nothing against the brackets alone.
        let bracketsOnly = NormativeBaselineSet(
            schemaVersion: baseline.schemaVersion,
            status: baseline.status,
            provenance: nil,
            ageBrackets: baseline.ageBrackets,
            videos: [:]
        )
        return score(
            metricSets: [],
            baseline: bracketsOnly,
            ageMonths: ageMonths,
            coreFrames: [],
            scoredAtUtc: ""
        ).ageBracketId
    }

    public static func scoreVideo(
        metricSet: PerVideoMetricSet,
        video: NormativeVideo,
        baseline: NormativeBaselineSet,
        bracketId: String,
        clipFrames: [GazeFrame] = []
    ) -> PerVideoScoreReport {
        scoreVideo(
            metricSet: metricSet,
            video: video,
            baseline: baseline,
            bracketId: bracketId,
            clipFrames: clipFrames.map { CoreFrame($0) }
        )
    }

    /// One clip scored by the core as a one-clip session: the baseline holds only `video`
    /// and a single bracket `bracketId` that age 0 resolves to, and every clip frame is
    /// attributed to the clip, so the session's only report is the clip's report.
    static func scoreVideo(
        metricSet: PerVideoMetricSet,
        video: NormativeVideo,
        baseline: NormativeBaselineSet,
        bracketId: String,
        clipFrames: [CoreFrame]
    ) -> PerVideoScoreReport {
        let oneClip = NormativeBaselineSet(
            schemaVersion: baseline.schemaVersion,
            status: baseline.status,
            provenance: nil,
            ageBrackets: [.init(id: bracketId, minMonths: 0, maxMonths: 0)],
            videos: [metricSet.stimulusId: video]
        )
        let session = score(
            metricSets: [metricSet],
            baseline: oneClip,
            ageMonths: 0,
            coreFrames: clipFrames.map { $0.attributed(to: metricSet.stimulusId) },
            scoredAtUtc: ""
        )
        if let report = session.videos.first {
            return report
        }
        // Only reached when the core refused the documents (already logged).
        return PerVideoScoreReport(
            stimulusId: metricSet.stimulusId,
            title: video.title,
            category: video.category,
            rows: [],
            subscore: 0,
            validSampleRatio: metricSet.validSampleRatio,
            lowConfidence: true,
            hasBaseline: true,
            weight: video.weight
        )
    }
}
