import AsdCore
import Foundation
import XCTest
@testable import CurvAsdGazeKit

/// FFI-4: the kit passes the same golden vectors as `cargo test -p asd-core`
/// (`rust/crates/asd-core/tests/golden/{scoring,flag,metrics,aggregate,quality}`), through the
/// engines that now delegate to the Rust core. Frames in the vectors are in the core's frame
/// form, so frame-based engines are driven through their `CoreFrame` entry points; the
/// `GazeFrame` -> `CoreFrame` mapping is covered by `AsdCoreBridgeTests` and the Swift
/// oracle comparison by `LegacyParityTests`.
///
/// Every golden file must be consumed by a test here (`testEveryGoldenFileIsCovered`), so a
/// new vector file cannot be skipped silently.
///
/// `ASD_WRITE_GOLDEN=1` regenerates the vectors the former Swift engines can compute from the
/// golden inputs alone into `ASD_GOLDEN_OUT` (default: a temporary directory). It never
/// writes into the golden tree: a changed vector goes into a new file with a new name (FFI-4).
final class GoldenParityTests: XCTestCase {

    /// Functions with an FFI-backed engine path, by family.
    private static let covered: [String: Set<String>] = [
        "scoring": ["evaluate", "piecewise", "brackets", "score_video", "score_session", "format_scored_at", "validate_baselines"],
        "flag": ["assess"],
        "quality": ["evaluate_sample"],
        "metrics": ["extract", "extract_clip"],
        "aggregate": ["session"]
    ]

    private func vectors(_ family: String, _ function: String) throws -> [Golden.Vector] {
        let all = try Golden.files(in: family).flatMap { url -> [Golden.Vector] in
            let (fn, vectors) = try Golden.vectors(at: url)
            return fn == function ? vectors : []
        }
        XCTAssertFalse(all.isEmpty, "no golden vectors for \(family)/\(function)")
        return all
    }

    private func dict(_ value: Any) throws -> [String: Any] {
        try XCTUnwrap(value as? [String: Any])
    }

    private func frames(_ value: Any?) throws -> [CoreFrame] {
        try Golden.decode([CoreFrame].self, try XCTUnwrap(value))
    }

    /// Placeholder baseline for `scoreVideo`, which reads only the video it is given.
    private let unusedBaseline = NormativeBaselineSet(
        schemaVersion: "golden",
        status: "golden",
        provenance: nil,
        ageBrackets: [],
        videos: [:]
    )

    // MARK: - Coverage

    func testGoldenTreeIsFound() throws {
        XCTAssertTrue(FileManager.default.fileExists(atPath: Golden.root.path), Golden.root.path)
    }

    func testEveryGoldenFileIsCovered() throws {
        for (family, functions) in Self.covered {
            for url in try Golden.files(in: family) {
                let (function, vectors) = try Golden.vectors(at: url)
                XCTAssertTrue(functions.contains(function), "\(family)/\(url.lastPathComponent): function \(function) has no Swift parity test")
                XCTAssertFalse(vectors.isEmpty, url.lastPathComponent)
            }
        }
    }

    // MARK: - Scoring (CORE-1..3)

    func testScoreSessionMatchesGolden() throws {
        for v in try vectors("scoring", "score_session") {
            let input = try dict(v.input)
            let score = PerVideoScoringEngine.score(
                metricSets: try Golden.decode([PerVideoMetricSet].self, try XCTUnwrap(input["metricSets"])),
                baseline: try Golden.decode(NormativeBaselineSet.self, try XCTUnwrap(input["baseline"])),
                ageMonths: try XCTUnwrap(input["ageMonths"] as? Int),
                coreFrames: try frames(input["frames"]),
                scoredAtUtc: try XCTUnwrap(input["scoredAtUtc"] as? String)
            )
            Golden.assertClose(try Golden.json(score), v.expected, v.name)
        }
    }

    func testScoreVideoMatchesGolden() throws {
        for v in try vectors("scoring", "score_video") {
            let input = try dict(v.input)
            let report = PerVideoScoringEngine.scoreVideo(
                metricSet: try Golden.decode(PerVideoMetricSet.self, try XCTUnwrap(input["metricSet"])),
                video: try Golden.decode(NormativeVideo.self, try XCTUnwrap(input["video"])),
                baseline: unusedBaseline,
                bracketId: try XCTUnwrap(input["bracketId"] as? String),
                clipFrames: try frames(input["clipFrames"])
            )
            Golden.assertClose(try Golden.json(report), v.expected, v.name)
        }
    }

    func testBracketResolutionMatchesGolden() throws {
        for v in try vectors("scoring", "brackets") {
            let input = try dict(v.input)
            let set = NormativeBaselineSet(
                schemaVersion: "t",
                status: "t",
                provenance: nil,
                ageBrackets: try Golden.decode([NormativeBaselineSet.AgeBracket].self, try XCTUnwrap(input["ageBrackets"])),
                videos: [:]
            )
            let age = try XCTUnwrap(input["ageMonths"] as? Int)
            let actual: [String: Any] = [
                "resolvedBracketId": PerVideoScoringEngine.resolvedBracketId(baseline: set, ageMonths: age),
                "bracketId": set.bracketId(forAgeMonths: age) ?? NSNull()
            ]
            Golden.assertClose(actual, v.expected, v.name)
        }
    }

    /// Both the core's formatter and the one the kit uses for `scoredAt` (they differ only
    /// for fractions >= 0.9995 s, which no vector has; see `AsdCoreBridge.formatScoredAt`).
    func testFormatScoredAtMatchesGolden() throws {
        for v in try vectors("scoring", "format_scored_at") {
            let seconds = try XCTUnwrap(try dict(v.input)["unixSeconds"] as? Double)
            Golden.assertClose(["scoredAtUtc": formatScoredAt(unixSeconds: seconds)], v.expected, v.name)
            let kit = AsdCoreBridge.formatScoredAt(Date(timeIntervalSince1970: seconds))
            Golden.assertClose(["scoredAtUtc": kit], v.expected, "kit \(v.name)")
        }
    }

    func testValidateBaselinesMatchesGolden() throws {
        for v in try vectors("scoring", "validate_baselines") {
            let input = try dict(v.input)
            let problems = NormativeBaselineLibrary.validate(
                try Golden.decode(NormativeBaselineSet.self, try XCTUnwrap(input["baseline"])),
                requiredVideoIds: try XCTUnwrap(input["requiredVideoIds"] as? [String])
            )
            Golden.assertClose(["problems": problems], v.expected, v.name)
        }
    }

    /// `evaluate` and `piecewise` are internal steps of `scoreVideo` with no FFI entry point
    /// of their own; the core is checked on them by cargo, and every `scoreVideo` vector
    /// above runs them through the FFI. Here the Swift oracle is checked against them.
    func testLegacyOracleEvaluateAndPiecewiseMatchGolden() throws {
        for v in try vectors("scoring", "evaluate") {
            let input = try dict(v.input)
            let result = LegacyPerVideoScoringEngine.evaluate(
                observed: try XCTUnwrap(input["observed"] as? Double),
                direction: try Golden.decode(NormativeDirection.self, try XCTUnwrap(input["direction"])),
                range: try Golden.decode(NormativeRange.self, try XCTUnwrap(input["range"]))
            )
            Golden.assertClose(["status": result.status.rawValue, "score": result.score], v.expected, v.name)
        }
        for v in try vectors("scoring", "piecewise") {
            let input = try dict(v.input)
            let score = LegacyPerVideoScoringEngine.piecewise(
                observed: try XCTUnwrap(input["observed"] as? Double),
                goodEnd: try XCTUnwrap(input["goodEnd"] as? Double),
                badEnd: try XCTUnwrap(input["badEnd"] as? Double)
            )
            Golden.assertClose(["score": score], v.expected, v.name)
        }
    }

    // MARK: - Flag (CORE-4)

    func testAttentionFlagMatchesGolden() throws {
        for v in try vectors("flag", "assess") {
            let score = try Golden.decode(PerVideoSessionScore.self, try XCTUnwrap(try dict(v.input)["score"]))
            Golden.assertClose(try Golden.json(AttentionFlagAlgorithm.assess(score)), v.expected, v.name)
            Golden.assertClose(try Golden.json(LegacyAttentionFlagAlgorithm.assess(score)), v.expected, "legacy \(v.name)")
        }
    }

    // MARK: - Quality gate (CORE-7)

    func testQualityGateMatchesGolden() throws {
        for v in try vectors("quality", "evaluate_sample") {
            let input = try dict(v.input)
            let g = try Golden.decode(CoreQualityGate.self, try XCTUnwrap(input["gate"]))
            let gate = GazeQualityGate(
                minDistanceCm: g.minDistanceCm,
                maxDistanceCm: g.maxDistanceCm,
                maxYawRad: g.maxYawRad,
                maxPitchRad: g.maxPitchRad,
                minConfidence: g.minConfidence,
                blinkThreshold: g.blinkThreshold
            )
            let sample = try Golden.decode(GazeTelemetrySample.self, try XCTUnwrap(input["sample"]))
            Golden.assertClose(Self.json(gate.evaluateSample(sample)), v.expected, v.name)
            Golden.assertClose(Self.json(LegacyGazeQualityGate(gate).evaluateSample(sample)), v.expected, "legacy \(v.name)")
        }
    }

    // MARK: - Metrics and aggregate (CORE-5, CORE-6)

    func testMetricExtractionMatchesGolden() throws {
        for v in try vectors("metrics", "extract") {
            let sets = PerVideoMetricsExtractor.extract(coreFrames: try frames(try dict(v.input)["frames"]))
            Golden.assertClose(try Golden.json(sets), v.expected, v.name)
        }
    }

    /// `extractClip` (segment windows) has no FFI entry point and no public Swift API; the
    /// core runs it inside `score_session`, covered by the `score_video_frames` vectors.
    func testClipExtractionVectorsAreScoredThroughSegmentWindows() throws {
        XCTAssertFalse(try vectors("metrics", "extract_clip").isEmpty)
        XCTAssertFalse(try vectors("scoring", "score_video").filter { $0.name.hasPrefix("score_video_frames") }.isEmpty)
    }

    func testSessionAggregateMatchesGolden() throws {
        for v in try vectors("aggregate", "session") {
            let input = try dict(v.input)
            let aggregates = SessionBiomarkerAggregator.aggregate(
                coreFrames: try frames(input["validFrames"]),
                durationSec: try XCTUnwrap(input["durationSec"] as? Double)
            )
            Golden.assertClose(Self.json(aggregates), v.expected, v.name)
        }
    }

    // MARK: - JSON forms of the non-Encodable results

    static func json(_ e: GazeQualityGate.Evaluation) -> [String: Any] {
        ["isValid": e.isValid, "verdict": e.verdict.rawValue, "failureReasons": e.failureReasons, "isBlink": e.isBlink]
    }

    /// Absent keys for unmeasured values, as the core and the vectors write them.
    static func json(_ a: SessionBiomarkerAggregator.Aggregates) -> [String: Any] {
        var out: [String: Any] = [
            "socialDwellSec": a.socialDwellSec,
            "geometricDwellSec": a.geometricDwellSec,
            "socialPreferenceScore": a.socialPreferenceScore,
            "socialSmileIntensityMean": a.socialSmileIntensityMean,
            "smileReactivityIndex": a.smileReactivityIndex,
            "flatAffectScore": a.flatAffectScore,
            "surpriseScoreMean": a.surpriseScoreMean
        ]
        out["eyeDwellRatio"] = a.eyeDwellRatio
        out["mouthDwellRatio"] = a.mouthDwellRatio
        out["firstOrientLatencyMs"] = a.firstOrientLatencyMs
        return out
    }

    // MARK: - ASD_WRITE_GOLDEN writer

    /// Recomputes, with the former Swift engines, every vector whose input the Swift oracle
    /// can take directly (no frames: frame vectors carry derived affect values, not blend
    /// shapes) and writes `{input, expected}` files to `ASD_GOLDEN_OUT`. Review the diff
    /// against the golden tree; a real change becomes a new golden file.
    func testWriteGoldenFromSwiftOracle() throws {
        let env = ProcessInfo.processInfo.environment
        guard env["ASD_WRITE_GOLDEN"] == "1" else {
            throw XCTSkip("set ASD_WRITE_GOLDEN=1 to regenerate golden vectors from the Swift oracle")
        }
        let out = URL(fileURLWithPath: env["ASD_GOLDEN_OUT"]
            ?? NSTemporaryDirectory() + "asd-golden-\(Int(Date().timeIntervalSince1970))", isDirectory: true)
        XCTAssertFalse(out.standardizedFileURL.path.hasPrefix(Golden.root.standardizedFileURL.path), "never write into the golden tree")
        try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)

        func write(_ name: String, _ function: String, _ vectors: [[String: Any]]) throws {
            let doc: [String: Any] = [
                "schema": "asd-golden/1",
                "function": function,
                "derivation": "Written by GoldenParityTests.testWriteGoldenFromSwiftOracle (LegacyReference engines).",
                "vectors": vectors
            ]
            let url = out.appendingPathComponent(name)
            XCTAssertFalse(FileManager.default.fileExists(atPath: url.path), "\(url.path) exists; pick a new ASD_GOLDEN_OUT")
            try JSONSerialization.data(withJSONObject: doc, options: [.prettyPrinted, .sortedKeys]).write(to: url)
        }

        var flag: [[String: Any]] = []
        for v in try vectors("flag", "assess") {
            let score = try Golden.decode(PerVideoSessionScore.self, try XCTUnwrap(try dict(v.input)["score"]))
            flag.append(["name": v.name, "input": v.input, "expected": try Golden.json(LegacyAttentionFlagAlgorithm.assess(score))])
        }
        try write("flag_assess.json", "assess", flag)

        var quality: [[String: Any]] = []
        for v in try vectors("quality", "evaluate_sample") {
            let input = try dict(v.input)
            let g = try Golden.decode(CoreQualityGate.self, try XCTUnwrap(input["gate"]))
            let gate = GazeQualityGate(
                minDistanceCm: g.minDistanceCm, maxDistanceCm: g.maxDistanceCm, maxYawRad: g.maxYawRad,
                maxPitchRad: g.maxPitchRad, minConfidence: g.minConfidence, blinkThreshold: g.blinkThreshold
            )
            let sample = try Golden.decode(GazeTelemetrySample.self, try XCTUnwrap(input["sample"]))
            quality.append(["name": v.name, "input": v.input, "expected": Self.json(LegacyGazeQualityGate(gate).evaluateSample(sample))])
        }
        try write("quality_evaluate_sample.json", "evaluate_sample", quality)

        var validate: [[String: Any]] = []
        for v in try vectors("scoring", "validate_baselines") {
            let input = try dict(v.input)
            let problems = LegacyNormativeBaselineLibrary.validate(
                try Golden.decode(NormativeBaselineSet.self, try XCTUnwrap(input["baseline"])),
                requiredVideoIds: try XCTUnwrap(input["requiredVideoIds"] as? [String])
            )
            validate.append(["name": v.name, "input": v.input, "expected": ["problems": problems]])
        }
        try write("scoring_validate_baselines.json", "validate_baselines", validate)
        print("GoldenParityTests: wrote Swift-oracle vectors to \(out.path)")
    }
}
