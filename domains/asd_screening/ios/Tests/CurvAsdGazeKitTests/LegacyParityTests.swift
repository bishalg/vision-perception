import Foundation
import XCTest
@testable import CurvAsdGazeKit

/// FFI-3: the engines that now delegate to the Rust core give the same results as their
/// former Swift bodies (`Support/LegacyReference`) on Swift-built frames: real
/// `FacialAffectMetrics` blend shapes, bundled-catalog AOI names (so catalog roles count),
/// loop wraps, duplicate timestamps, rejected and invalid samples. Floats to 1e-9, strings,
/// enums, key sets and orders exactly.
final class LegacyParityTests: XCTestCase {
    private static let tolerance = 1e-9

    private let baseline = NormativeBaselineLibrary.loadBundled()!

    /// SplitMix64: deterministic across platforms and runs.
    private struct Rng {
        var state: UInt64
        mutating func next() -> UInt64 {
            state &+= 0x9E37_79B9_7F4A_7C15
            var z = state
            z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
            z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
            return z ^ (z >> 31)
        }
        mutating func unit() -> Double { Double(next() >> 11) / Double(1 << 53) }
        mutating func chance(_ p: Double) -> Bool { unit() < p }
        mutating func pick<T>(_ items: [T]) -> T { items[Int(next() % UInt64(items.count))] }
    }

    private static let aoiNames = [
        "Actor_Eyes", "Actor_Face", "Animal_Puppies", "Cartoon_Character", "Center_Fixation_Target",
        "Eye_Region", "Face_Center", "Face_Bounding_Box", "Gaze_Target_Left", "Gaze_Target_Right",
        "Geometric_Pattern", "Gift_Box", "Mechanical_Motion", "Mouth_Region", "Social_Actor",
        "Teacher_Face", "Teacher_Mouth", "Unknown_Region"
    ]

    /// One session: clips in playlist order (some repeated, some untagged), 60 Hz-ish frames.
    private func session(seed: UInt64, frameCount: Int = 600) -> [GazeFrame] {
        var rng = Rng(state: seed)
        let clipIds: [String?] = baseline.videos.keys.sorted().map { Optional($0) } + [nil, "stim_unknown_clip"]
        var frames: [GazeFrame] = []
        var sessionTime = 0.0
        var clip: String? = rng.pick(clipIds)
        var media = 0.0
        var x = 0.5
        var y = 0.5
        for _ in 0..<frameCount {
            if rng.chance(0.01) {
                clip = rng.pick(clipIds)
                media = 0
            }
            // Mostly 1/60 s steps; sometimes a duplicate timestamp or a loop wrap.
            if rng.chance(0.02) {
                media = 0
            } else if !rng.chance(0.02) {
                media += 1.0 / 60.0
            }
            sessionTime += 1.0 / 60.0
            // Fixation jitter with occasional saccades.
            if rng.chance(0.05) {
                x = rng.unit()
                y = rng.unit()
            } else {
                x = min(1, max(0, x + (rng.unit() - 0.5) * 0.004))
                y = min(1, max(0, y + (rng.unit() - 0.5) * 0.004))
            }
            var names: [String] = []
            for _ in 0..<Int(rng.next() % 3) { names.append(rng.pick(Self.aoiNames)) }
            let valid = !rng.chance(0.1)
            let verdict: GazeQualityGate.Verdict = rng.pick([.optimal, .optimal, .acceptable, .degraded, .rejected])
            let affect = FacialAffectMetrics(
                mouthSmileLeft: rng.unit(), mouthSmileRight: rng.unit(),
                cheekSquintLeft: rng.unit() * 0.5, cheekSquintRight: rng.unit() * 0.5,
                browInnerUp: rng.unit() * 0.3, browOuterUpLeft: rng.unit() * 0.3, browOuterUpRight: rng.unit() * 0.3,
                eyeWideLeft: rng.unit() * 0.2, eyeWideRight: rng.unit() * 0.2, jawOpen: rng.unit() * 0.4,
                browDownLeft: rng.unit() * 0.1, browDownRight: rng.unit() * 0.1,
                mouthFrownLeft: rng.unit() * 0.1, mouthFrownRight: rng.unit() * 0.1
            )
            let mediaTime: Double? = rng.chance(0.05) ? nil : media
            let sample = GazeTelemetrySample(
                timestampMs: sessionTime * 1000,
                videoTimeSec: sessionTime,
                xNorm: x,
                yNorm: y,
                confidence: valid ? 0.95 : 0.1,
                isValid: valid,
                stimulusId: clip,
                clipMediaTimeSec: mediaTime
            )
            frames.append(GazeFrame(
                gaze: sample,
                affect: affect,
                activeAOINames: names,
                qualityVerdict: verdict,
                stimulusId: clip,
                clipMediaTimeSec: mediaTime
            ))
        }
        return frames
    }

    private static let seeds: [UInt64] = [1, 7, 42, 2026, 0xA5D_C0DE]

    private func assertJSONClose<T: Encodable>(_ actual: T, _ expected: T, _ label: String, file: StaticString = #filePath, line: UInt = #line) throws {
        Golden.assertClose(try Golden.json(actual), try Golden.json(expected), label, file: file, line: line)
    }

    // MARK: - Metrics, scoring, flag

    func testExtractionMatchesSwiftOracle() throws {
        for seed in Self.seeds {
            let frames = session(seed: seed)
            let rust = PerVideoMetricsExtractor.extract(from: frames)
            let swift = LegacyPerVideoMetricsExtractor.extract(from: frames)
            XCTAssertEqual(rust.map(\.stimulusId), swift.map(\.stimulusId), "group order, seed \(seed)")
            try assertJSONClose(rust, swift, "extract seed \(seed)")
        }
    }

    func testSessionScoreAndFlagMatchSwiftOracle() throws {
        let scoredAt = Date(timeIntervalSince1970: 1_792_800_000.75)
        for seed in Self.seeds {
            let frames = session(seed: seed)
            let sets = LegacyPerVideoMetricsExtractor.extract(from: frames)
            for age in [6, 12, 20, 30, 40, 55, 72] {
                let rust = PerVideoScoringEngine.score(metricSets: sets, baseline: baseline, ageMonths: age, frames: frames, scoredAt: scoredAt)
                let swift = LegacyPerVideoScoringEngine.score(metricSets: sets, baseline: baseline, ageMonths: age, frames: frames, scoredAt: scoredAt)
                try assertJSONClose(rust, swift, "score seed \(seed) age \(age)")
                XCTAssertEqual(AttentionFlagAlgorithm.assess(rust), LegacyAttentionFlagAlgorithm.assess(swift), "flag seed \(seed) age \(age)")
            }
            // Without frames (the older call shape): whole-clip metric sets only.
            let rust = PerVideoScoringEngine.score(metricSets: sets, baseline: baseline, ageMonths: 30, scoredAt: scoredAt)
            let swift = LegacyPerVideoScoringEngine.score(metricSets: sets, baseline: baseline, ageMonths: 30, scoredAt: scoredAt)
            try assertJSONClose(rust, swift, "score without frames seed \(seed)")
        }
    }

    func testScoreVideoMatchesSwiftOracle() throws {
        let frames = session(seed: 99, frameCount: 2400)
        let byClip = Dictionary(grouping: frames) { $0.stimulusId ?? PerVideoMetricsExtractor.untaggedClipId }
        for (id, video) in baseline.videos.sorted(by: { $0.key < $1.key }) {
            let clipFrames = byClip[id] ?? []
            let set = LegacyPerVideoMetricsExtractor.extract(from: clipFrames).first
                ?? PerVideoMetricSet(stimulusId: id, clipFrameCount: 0, validFrameCount: 0, validSampleRatio: 0,
                                     observedDurationSec: 0, aoiDwellRatios: [:], metrics: [:])
            for bracket in baseline.ageBrackets.map(\.id) + ["default"] {
                let rust = PerVideoScoringEngine.scoreVideo(metricSet: set, video: video, baseline: baseline, bracketId: bracket, clipFrames: clipFrames)
                let swift = LegacyPerVideoScoringEngine.scoreVideo(metricSet: set, video: video, baseline: baseline, bracketId: bracket, clipFrames: clipFrames)
                try assertJSONClose(rust, swift, "scoreVideo \(id) \(bracket)")
            }
        }
    }

    func testBracketResolutionMatchesSwiftOracle() {
        for age in -5...80 {
            XCTAssertEqual(
                PerVideoScoringEngine.resolvedBracketId(baseline: baseline, ageMonths: age),
                LegacyPerVideoScoringEngine.resolvedBracketId(baseline: baseline, ageMonths: age),
                "age \(age)"
            )
        }
        let empty = NormativeBaselineSet(schemaVersion: "t", status: "t", provenance: nil, ageBrackets: [], videos: [:])
        XCTAssertEqual(PerVideoScoringEngine.resolvedBracketId(baseline: empty, ageMonths: 30), "default")
    }

    /// Includes fractions >= 0.9995 s, where the core's `format_scored_at` would round
    /// differently; the kit keeps the platform formatter for `scoredAtUtc`.
    func testScoredAtFormatMatchesSwiftOracle() {
        for seconds in [0, 1, 59.999, 1_792_800_000, 1_792_800_000.5, 1_792_800_000.999_5, 1_792_800_000.999_9, 4_102_444_800, -1.5] as [Double] {
            let date = Date(timeIntervalSince1970: seconds)
            XCTAssertEqual(AsdCoreBridge.formatScoredAt(date), ISO8601DateFormatter().string(from: date), "\(seconds)")
        }
    }

    // MARK: - Aggregate

    func testSessionAggregateMatchesSwiftOracle() {
        for seed in Self.seeds {
            let valid = session(seed: seed).filter { $0.gaze.isValid && $0.qualityVerdict != .rejected }
            for subset in [valid, Array(valid.prefix(1)), [], valid.filter { !$0.activeAOINames.isEmpty }] {
                let rust = SessionBiomarkerAggregator.aggregate(validFrames: subset, durationSec: 10)
                let swift = LegacySessionBiomarkerAggregator.aggregate(validFrames: subset, durationSec: 10)
                Golden.assertClose(GoldenParityTests.json(rust), GoldenParityTests.json(swift), "aggregate seed \(seed) n=\(subset.count)")
            }
        }
    }

    // MARK: - Quality gate

    func testQualityGateMatchesSwiftOracle() {
        var rng = Rng(state: 31)
        let gates = [
            GazeQualityGate(),
            GazeQualityGate(minDistanceCm: 30.5, maxDistanceCm: 70.9, maxYawRad: 0.7, maxPitchRad: 0.4, minConfidence: 0.333, blinkThreshold: 0.2)
        ]
        // Boundary values of the default gate plus random readings.
        var samples: [GazeTelemetrySample] = [35.0, 65.0, 34.999, 65.001].map {
            GazeTelemetrySample(timestampMs: 0, xNorm: 0.5, yNorm: 0.5, confidence: 0.5, leftEyeOpenness: 0.45, rightEyeOpenness: 0.45, distanceToScreenCm: $0)
        }
        for i in 0..<2000 {
            samples.append(GazeTelemetrySample(
                timestampMs: Double(i) * 16.7,
                xNorm: rng.unit(),
                yNorm: rng.unit(),
                confidence: rng.unit(),
                leftEyeOpenness: rng.unit(),
                rightEyeOpenness: rng.unit(),
                headPitchRad: (rng.unit() - 0.5) * 1.6,
                headYawRad: (rng.unit() - 0.5) * 1.6,
                headRollRad: (rng.unit() - 0.5),
                distanceToScreenCm: 20 + rng.unit() * 60,
                isValid: rng.chance(0.9)
            ))
        }
        for (g, gate) in gates.enumerated() {
            let legacy = LegacyGazeQualityGate(gate)
            for (i, sample) in samples.enumerated() {
                XCTAssertEqual(gate.evaluateSample(sample), legacy.evaluateSample(sample), "gate \(g) sample \(i)")
            }
        }
    }

    // MARK: - Baseline validation

    func testBaselineValidationMatchesSwiftOracleUpToOrder() throws {
        let catalogIds = baseline.videos.keys.sorted()
        XCTAssertEqual(NormativeBaselineLibrary.validate(baseline, requiredVideoIds: catalogIds), [])
        // Drop ranges and segments to produce problems.
        var json = try XCTUnwrap(try Golden.json(baseline) as? [String: Any])
        var videos = try XCTUnwrap(json["videos"] as? [String: Any])
        var first = try XCTUnwrap(videos[catalogIds[0]] as? [String: Any])
        first["segments"] = []
        videos[catalogIds[0]] = first
        videos.removeValue(forKey: catalogIds[1])
        json["videos"] = videos
        var brackets = try XCTUnwrap(json["ageBrackets"] as? [[String: Any]])
        brackets.append(["id": "61-72", "minMonths": 61, "maxMonths": 72])
        json["ageBrackets"] = brackets
        let broken = try Golden.decode(NormativeBaselineSet.self, json)
        let required = catalogIds + ["stim_missing"]
        let rust = NormativeBaselineLibrary.validate(broken, requiredVideoIds: required)
        let swift = LegacyNormativeBaselineLibrary.validate(broken, requiredVideoIds: required)
        XCTAssertFalse(rust.isEmpty)
        XCTAssertEqual(rust.sorted(), swift.sorted())
        XCTAssertEqual(Array(rust.prefix(2)), ["missing video profile: \(catalogIds[1])", "missing video profile: stim_missing"])
    }
}
