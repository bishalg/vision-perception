import AsdCore
import Foundation
import simd
import XCTest
@testable import CurvAsdGazeKit

/// WP-G.9 parity (PIPE-1, QS-1, QS-3, acceptance 12): a committed synthetic REP-1 log replayed
/// through `GazeProcessingActor` (ARKit snapshot -> FFI record -> Rust pipeline) gives exactly
/// the `trackingQuality`, per-clip tracking, target results and advice that `asd-cli gaze
/// replay` gives for the same log (canonical JSON compare: same keys, same numbers).
///
/// Fixture (`Fixtures/gaze/`): `blinks_and_loss_seed7_f32.jsonl` is
/// `asd-cli gaze synth --scenario blinks_and_loss --seed 7` with every matrix and vector number
/// rounded to `Float`, because ARKit delivers `simd_float4x4` and the actor sends what ARKit
/// gives; scalars (time, blink, light) stay `Double`. The expected file is
/// `asd-cli gaze replay Fixtures/gaze/blinks_and_loss_seed7_f32.jsonl` (regenerate both when
/// the pipeline maths changes on purpose, never to make this test pass).
///
/// The driver below plays the header's target schedule exactly as `asd_core::gaze::replay`
/// does: a target opens at the first frame at or after `startSec` and closes when the pipeline
/// reports it complete or the next one is due; consecutive calibration targets form one
/// calibration; a frame with `clip_media_time_sec` belongs to a recorded clip (by
/// `stimulus_id`), any other frame ends the clip.
final class GazePipelineParityTests: XCTestCase {
    static let fixtures: URL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("Fixtures/gaze", isDirectory: true)

    struct Header: Decodable {
        struct Target: Decodable {
            let kind: String
            let xNorm: Double
            let yNorm: Double
            let startSec: Double
            let endSec: Double
        }
        let deviceModel: String
        let orientation: String
        let targets: [Target]
    }

    struct Line: Decodable {
        let t_sec: Double
        let face_tracked: Bool
        let face_transform: [Double]?
        let look_at: [Double]?
        let left_eye_transform: [Double]?
        let right_eye_transform: [Double]?
        let eye_blink_left: Double?
        let eye_blink_right: Double?
        let ambient_intensity: Double?
        let ambient_color_temp: Double?
        let light_direction: [Double]?
        let clip_media_time_sec: Double?
        let stimulus_id: String?

        func matrix(_ m: [Double]?) -> simd_float4x4 {
            guard let m, m.count == 16 else { return matrix_identity_float4x4 }
            let f = m.map(Float.init)
            return simd_float4x4(
                simd_float4(f[0], f[1], f[2], f[3]),
                simd_float4(f[4], f[5], f[6], f[7]),
                simd_float4(f[8], f[9], f[10], f[11]),
                simd_float4(f[12], f[13], f[14], f[15])
            )
        }

        func vector(_ v: [Double]?) -> simd_float3? {
            guard let v, v.count == 3 else { return nil }
            return simd_float3(Float(v[0]), Float(v[1]), Float(v[2]))
        }

        var snapshot: FaceAnchorSnapshot {
            var shapes: [String: Double] = [:]
            shapes["eyeBlinkLeft"] = eye_blink_left
            shapes["eyeBlinkRight"] = eye_blink_right
            return FaceAnchorSnapshot(
                isTracked: face_tracked,
                transform: matrix(face_transform),
                lookAtPoint: vector(look_at) ?? simd_float3(0, 0, 0),
                leftEyeTransform: matrix(left_eye_transform),
                rightEyeTransform: matrix(right_eye_transform),
                blendShapes: shapes,
                ambientIntensity: ambient_intensity,
                ambientColorTemperature: ambient_color_temp,
                lightDirection: vector(light_direction)
            )
        }
    }

    static func load(_ name: String) throws -> (Header, [Line]) {
        let text = try String(contentsOf: fixtures.appendingPathComponent(name), encoding: .utf8)
        var lines = text.split(separator: "\n", omittingEmptySubsequences: true).makeIterator()
        let decoder = JSONDecoder()
        let header = try decoder.decode(Header.self, from: Data(try XCTUnwrap(lines.next()).utf8))
        var samples: [Line] = []
        while let line = lines.next() {
            samples.append(try decoder.decode(Line.self, from: Data(line.utf8)))
        }
        return (header, samples)
    }

    static func kind(_ s: String) throws -> GazeTargetKind {
        switch s {
        case "calibration": return .calibration
        case "validation": return .validation
        case "driftCheck": return .driftCheck
        default: throw NSError(domain: "kind \(s)", code: 1)
        }
    }

    struct Replayed {
        var summary: String
        var clips: String
        var targets: [GazeTargetResultRecord]
        var advice: GazeRecalAdviceRecord
        var frames: [GazeFrame]
        var recordedFrames: Int
    }

    /// The `asd_core::gaze::replay` rules, driven through the actor.
    static func replay(_ header: Header, _ samples: [Line]) async throws -> Replayed {
        XCTAssertEqual(header.orientation, "portrait")
        let actor = GazeProcessingActor(deviceModelIdentifier: header.deviceModel, orientation: .portrait)
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let targets = header.targets
        var results: [GazeTargetResultRecord] = []
        var open = false
        var calibrating = false
        var next = 0
        var currentClip: String?
        var recorded = 0

        func end() async {
            guard open else { return }
            open = false
            if let r = await actor.endTarget() { results.append(r) }
        }
        func finishCalibration() async throws {
            if calibrating {
                calibrating = false
                _ = try await actor.finishCalibration()
            }
        }
        func begin(_ index: Int) async throws {
            let t = targets[index]
            let at = GazeVec2(x: t.xNorm, y: t.yNorm)
            let k = try kind(t.kind)
            if k == .calibration {
                if !calibrating {
                    var layout: [GazeVec2] = []
                    for x in targets[index...].prefix(while: { $0.kind == "calibration" }) {
                        let p = GazeVec2(x: x.xNorm, y: x.yNorm)
                        if layout.last != p { layout.append(p) }
                    }
                    try await actor.startCalibration(layout: layout, reason: nil)
                    calibrating = true
                }
                // The fixture has no CAL-3 re-shows: the pending target is always this one.
                guard case let .pending(_, target, attempt) = await actor.calibrationStatus() else {
                    return XCTFail("calibration target \(index) is not pending")
                }
                XCTAssertEqual(attempt, 1)
                XCTAssertEqual(target, at)
            } else {
                try await finishCalibration()
            }
            try await actor.beginTarget(kind: k, at: at)
            open = true
        }

        for raw in samples {
            let t = raw.t_sec
            if open && next < targets.count && t >= targets[next].startSec {
                await end()
            }
            while !open && next < targets.count && t >= targets[next].startSec {
                let index = next
                next += 1
                try await begin(index)
                if targets[index].endSec < t { await end() }
            }
            let recording = raw.clip_media_time_sec != nil
            recorded += recording ? 1 : 0
            let clipId = raw.stimulus_id ?? ""
            if !recording {
                await actor.endStimulusClip()
            } else {
                let isOpen = await actor.isClipOpen
                if currentClip != clipId || !isOpen {
                    await actor.switchStimulusClip(battery: StimulusAOITracker(aois: []), stimulusId: clipId)
                    currentClip = clipId
                }
                await actor.updateClipMediaTime(raw.clip_media_time_sec ?? 0)
            }
            _ = await actor.processFaceAnchor(snapshot: raw.snapshot, elapsedSec: t)
            if open, await actor.targetComplete() { await end() }
        }
        await end()
        await actor.endStimulusClip()
        try await finishCalibration()
        let maybeAdvice = await actor.recalibrationAdvice()
        let advice = try XCTUnwrap(maybeAdvice)
        let (summary, frames) = await actor.finalizeSession()
        let quality = try XCTUnwrap(summary.trackingQuality, "live session carries trackingQuality")
        let clips = try XCTUnwrap(summary.clipTracking)
        let encoder = JSONEncoder()
        return Replayed(
            summary: try quality.jsonString(),
            clips: String(decoding: try encoder.encode(clips), as: UTF8.self),
            targets: results,
            advice: advice,
            frames: frames,
            recordedFrames: recorded
        )
    }

    /// Canonical form for comparison: parsed and re-serialised with sorted keys, so two
    /// documents compare equal exactly when their keys and numbers are equal.
    static func canonical(_ value: Any) throws -> String {
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed])
        return String(decoding: data, as: UTF8.self)
    }

    static func canonical(json: String) throws -> String {
        try canonical(JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed]))
    }

    func testReplayThroughTheActorEqualsTheRustReplay() async throws {
        let (header, samples) = try Self.load("blinks_and_loss_seed7_f32.jsonl")
        let expected = try Golden.object(at: Self.fixtures.appendingPathComponent("blinks_and_loss_seed7_f32.replay.json"))
        let got = try await Self.replay(header, samples)

        XCTAssertEqual(got.recordedFrames, expected["recordedFrames"] as? Int)
        XCTAssertEqual(
            try Self.canonical(json: got.summary),
            try Self.canonical(try XCTUnwrap(expected["summary"])),
            "trackingQuality through the actor must equal the Rust replay"
        )
        XCTAssertEqual(
            try Self.canonical(json: got.clips),
            try Self.canonical(try XCTUnwrap(expected["clips"])),
            "per-clip ClipTracking must equal the Rust replay"
        )
        let expectedTargets = try XCTUnwrap(expected["targets"] as? [[String: Any]])
        XCTAssertEqual(got.targets.count, expectedTargets.count)
        for (record, json) in zip(got.targets, expectedTargets) {
            XCTAssertEqual(record.outcome == .hit ? "hit" : (record.outcome == .missed ? "missed" : "needsRetry"), json["outcome"] as? String)
            XCTAssertEqual(record.validSamples, json["validSamples"] as? UInt32)
            // NSJSONSerialization may parse a 17-digit number one ulp off; the FFI value is exact.
            if let e = json["accuracyDeg"] as? Double {
                XCTAssertEqual(try XCTUnwrap(record.accuracyDeg), e, accuracy: 1e-15)
            } else {
                XCTAssertNil(record.accuracyDeg)
            }
        }
        let expectedAdvice = try XCTUnwrap(expected["advice"] as? [String: Any])
        XCTAssertEqual(got.advice.action == .none ? "none" : "other", expectedAdvice["action"] as? String)
        XCTAssertEqual(got.advice.limitReached, expectedAdvice["limitReached"] as? Bool)

        // The frame buffer holds exactly the recorded (clip) frames, each with its track state.
        XCTAssertEqual(got.frames.count, got.recordedFrames)
        let states = Set(got.frames.compactMap(\.gaze.trackState))
        XCTAssertTrue(states.isSuperset(of: [.tracked, .blink, .lost]), "\(states)")
        XCTAssertTrue(got.frames.allSatisfy { $0.gaze.trackState != nil })
        // GAP-2: filled frames are never valid.
        XCTAssertTrue(got.frames.filter { $0.gaze.gapFilled == true }.allSatisfy { !$0.gaze.isValid })
    }

    /// The config the actor builds for the fixture's model is the geometry the log was made
    /// with (GEO-2 table lookup), so the comparison above runs on the same screen.
    func testActorConfigUsesTheTableGeometryOfTheLog() throws {
        let text = try String(contentsOf: Self.fixtures.appendingPathComponent("blinks_and_loss_seed7_f32.jsonl"), encoding: .utf8)
        let first = try XCTUnwrap(text.split(separator: "\n").first)
        let header = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(first.utf8)) as? [String: Any])
        let config = try JSONSerialization.jsonObject(with: Data(GazePipelineDocuments.config(
            modelIdentifier: try XCTUnwrap(header["deviceModel"] as? String),
            orientation: .portrait,
            gate: GazeQualityGate()
        ).utf8)) as? [String: Any]
        XCTAssertEqual(config?["geometrySource"] as? String, "table")
        XCTAssertEqual(
            try Self.canonical(try XCTUnwrap(config?["geometry"])),
            try Self.canonical(try XCTUnwrap(header["geometry"]))
        )
    }
}
