import AsdCore
import Foundation
import os

// The kit's only door into the Rust core (FFI-3, ADR-0006 §4).
//
// Documents cross the boundary as JSON in the Swift `Codable` wire format; this file holds
// no scoring logic, only encoding, the FFI call and decoding. If the core refuses a
// document (it never should for documents this kit encoded), the caller gets a
// conservative fallback and the failure is logged, because the public engine API does not
// throw.

/// One frame in the form `asd-core` reads (`rust/crates/asd-core/src/types/frame.rs`).
///
/// `FacialAffectMetrics.socialSmileIntensity`, `expressivityIndex` and `surpriseScore` are
/// computed properties in Swift, so `JSONEncoder` of a `GazeFrame` does not carry them; this
/// form sends them explicitly together with the gaze fields scoring reads.
struct CoreFrame: Codable, Equatable, Sendable {
    struct Gaze: Codable, Equatable, Sendable {
        let valid: Bool
        let xNorm: Double
        let yNorm: Double
        let videoTimeSec: Double
        let clipMediaTimeSec: Double?

        enum CodingKeys: String, CodingKey {
            case valid
            case xNorm = "x_norm"
            case yNorm = "y_norm"
            case videoTimeSec = "video_time_sec"
            case clipMediaTimeSec = "clip_media_time_sec"
        }
    }

    struct Affect: Codable, Equatable, Sendable {
        let socialSmileIntensity: Double
        let expressivityIndex: Double
        let surpriseScore: Double
    }

    let gaze: Gaze
    let affect: Affect
    let activeAOINames: [String]
    let qualityVerdict: GazeQualityGate.Verdict
    private(set) var stimulusId: String?

    /// The same frame attributed to another clip.
    func attributed(to stimulusId: String) -> CoreFrame {
        var copy = self
        copy.stimulusId = stimulusId
        return copy
    }

    init(_ frame: GazeFrame) {
        gaze = Gaze(
            valid: frame.gaze.isValid,
            xNorm: frame.gaze.xNorm,
            yNorm: frame.gaze.yNorm,
            videoTimeSec: frame.gaze.videoTimeSec,
            clipMediaTimeSec: frame.gaze.clipMediaTimeSec
        )
        affect = Affect(
            socialSmileIntensity: frame.affect.socialSmileIntensity,
            expressivityIndex: frame.affect.expressivityIndex,
            surpriseScore: frame.affect.surpriseScore
        )
        activeAOINames = frame.activeAOINames
        qualityVerdict = frame.qualityVerdict
        stimulusId = frame.stimulusId
    }
}

/// `ScoringProfile.qualityGate` (camelCase), the gate document `evaluate_quality_json` reads.
struct CoreQualityGate: Codable, Equatable, Sendable {
    let minDistanceCm: Double
    let maxDistanceCm: Double
    let maxYawRad: Double
    let maxPitchRad: Double
    let minConfidence: Double
    let blinkThreshold: Double
}

/// The parts of the bundled default `ScoringProfile` the kit mirrors as public constants.
struct CoreScoringProfile: Decodable, Sendable {
    struct Flag: Decodable, Sendable {
        let greenFloor: Double
        let yellowFloor: Double
        let lowConfidenceShare: Double
        let atypicalShareForRed: Double
    }

    let flag: Flag
    let lowConfidenceValidRatio: Double
    let qualityGate: CoreQualityGate
}

enum AsdCoreBridge {
    static let logger = Logger(subsystem: "com.visionperception.asd", category: "AsdCore")

    /// The bundled default `ScoringProfile` JSON (CON-5); `assess_flag_json` needs a full one.
    static let defaultScoringProfileJSON: String = defaultScoringProfileJson()

    static var coreVersion: String { AsdCore.coreVersion() }

    static func encode<T: Encodable>(_ value: T) throws -> String {
        let data = try JSONEncoder().encode(value)
        return String(decoding: data, as: UTF8.self)
    }

    static func decode<T: Decodable>(_ type: T.Type, from json: String) throws -> T {
        try JSONDecoder().decode(type, from: Data(json.utf8))
    }

    /// Logs a refused call. Only the operation and the error kind, never document contents.
    static func report(_ operation: StaticString, _ error: Error) {
        let kind: String
        switch error {
        case let asd as AsdError:
            switch asd {
            case .InvalidJson: kind = "invalidJson"
            case .InvalidArgument: kind = "invalidArgument"
            case .NotRegistered: kind = "notRegistered"
            case .UploadRefused: kind = "uploadRefused"
            case .InvalidUrl: kind = "invalidUrl"
            }
        case is EncodingError: kind = "encoding (non-finite number?)"
        case is DecodingError: kind = "decoding"
        default: kind = String(describing: type(of: error))
        }
        logger.fault("asd-core \(operation, privacy: .public) failed: \(kind, privacy: .public)")
    }

    // MARK: Calls

    static func scoreSession(
        metricSets: [PerVideoMetricSet],
        baseline: NormativeBaselineSet,
        ageMonths: Int,
        frames: [CoreFrame],
        scoredAtUtc: String
    ) throws -> PerVideoSessionScore {
        let json = try scoreSessionJson(
            metricSetsJson: encode(metricSets),
            baselineJson: encode(baseline),
            ageMonths: Int64(ageMonths),
            framesJson: encode(frames),
            scoredAtRfc3339: scoredAtUtc
        )
        return try decode(PerVideoSessionScore.self, from: json)
    }

    static func assessFlag(_ score: PerVideoSessionScore, profileJSON: String = defaultScoringProfileJSON) throws -> AttentionFlagAssessment {
        let json = try assessFlagJson(scoreJson: encode(score), profileJson: profileJSON)
        return try decode(AttentionFlagAssessment.self, from: json)
    }

    static func extractMetrics(_ frames: [CoreFrame]) throws -> [PerVideoMetricSet] {
        try decode([PerVideoMetricSet].self, from: extractMetricsJson(framesJson: encode(frames)))
    }

    static func aggregateSession(_ frames: [CoreFrame], durationSec: Double) throws -> SessionBiomarkerAggregator.Aggregates {
        let json = try aggregateSessionJson(framesJson: encode(frames), durationSec: durationSec)
        return try decode(SessionBiomarkerAggregator.Aggregates.self, from: json)
    }

    static func evaluateQuality(_ sample: GazeTelemetrySample, gateJSON: String) throws -> GazeQualityGate.Evaluation {
        let json = try evaluateQualityJson(sampleJson: encode(sample), gateJson: gateJSON)
        return try decode(GazeQualityGate.Evaluation.self, from: json)
    }

    static func validateBaselines(_ set: NormativeBaselineSet, requiredVideoIds: [String]) throws -> [String] {
        try validateBaselinesJson(setJson: encode(set), requiredIds: requiredVideoIds)
    }

    /// `scoredAtUtc` as the app has always written it. The core takes the string verbatim.
    /// Not `AsdCore.formatScoredAt`: that truncates to the second, while
    /// `ISO8601DateFormatter` first rounds to the millisecond, so the two differ when the
    /// fraction is >= 0.9995 s (reported as an asd-ffi gap in WP-A.11).
    static func formatScoredAt(_ date: Date) -> String {
        ISO8601DateFormatter().string(from: date)
    }

    static func defaultProfile() throws -> CoreScoringProfile {
        try decode(CoreScoringProfile.self, from: defaultScoringProfileJSON)
    }
}
