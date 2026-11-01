import AsdCore
import Foundation
import simd
import XCTest
@testable import CurvAsdGazeKit

/// WP-G.9: the actor runs the Rust pipeline (TRK-1 states on frames, GAP-1/2 in the frames
/// file, QS-1/QS-3 in the summary, QS-5 distance, SIM-3 for synthetic frames) and the session
/// documents reach the score reports (1.7 `perVideo[]`).
final class GazePipelineIntegrationTests: XCTestCase {
    /// Face 48 cm in front of the glass; `lookY` -0.1 lands mid-screen (straight at the
    /// camera, 0, is above the active area and reads as off screen).
    static func face(lookY: Float = -0.1, blink: Double = 0.05) -> FaceAnchorSnapshot {
        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.0, -0.48, 1.0)
        return FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(0, lookY, 0.48),
            blendShapes: ["eyeBlinkLeft": blink, "eyeBlinkRight": blink],
            ambientIntensity: 800
        )
    }

    static let lost = FaceAnchorSnapshot(isTracked: false, transform: matrix_identity_float4x4, lookAtPoint: simd_float3(0, 0, 0))

    func testFramesCarryTheirTrackState() async {
        let actor = GazeProcessingActor()
        await actor.startSession()
        var t = 0.0
        func push(_ s: FaceAnchorSnapshot) async { _ = await actor.processFaceAnchor(snapshot: s, elapsedSec: t); t += 1.0 / 60.0 }
        await push(Self.face())
        await push(Self.lost)
        await push(Self.face(blink: 0.95))
        for _ in 0..<10 { await push(Self.face()) }
        await push(Self.face(lookY: 0))
        let frames = await actor.getVolatileFrames()
        XCTAssertEqual(frames.map(\.gaze.trackState).prefix(3), [.tracked, .lost, .blink])
        XCTAssertEqual(frames.last?.gaze.trackState, .offscreen)
        XCTAssertFalse(frames.last!.gaze.isValid, "off-screen frames are not valid (TRK-2)")
        // CNF-4: the stored confidence is the computed one, not a constant.
        XCTAssertEqual(frames[0].gaze.confidence, 1.0, accuracy: 1e-9)
        XCTAssertEqual(frames[1].gaze.confidence, 0)
        XCTAssertEqual(frames[1].qualityVerdict, .rejected, "a lost frame stays rejected (TRK-1, G.0)")
    }

    func testShortTrackLossIsGapFilledButNeverValid() async {
        let actor = GazeProcessingActor()
        await actor.startSession()
        var t = 0.0
        func push(_ s: FaceAnchorSnapshot) async { _ = await actor.processFaceAnchor(snapshot: s, elapsedSec: t); t += 1.0 / 60.0 }
        for _ in 0..<30 { await push(Self.face()) }
        for _ in 0..<3 { await push(Self.lost) }     // 50 ms < gapFill.maxMs (75 ms)
        for _ in 0..<30 { await push(Self.face()) }
        let (summary, frames) = await actor.finalizeSession()
        let filled = frames.enumerated().filter { $0.element.gaze.gapFilled == true }.map(\.offset)
        XCTAssertEqual(filled, [30, 31, 32])
        XCTAssertTrue(filled.allSatisfy { !frames[$0].gaze.isValid && frames[$0].gaze.trackState == .lost })
        // GAP-2: validRatio counts measured frames only.
        XCTAssertEqual(summary.validRatio, 60.0 / 63.0, accuracy: 1e-9)
        XCTAssertEqual(summary.validFrames, 60)
    }

    func testLongTrackLossIsNotFilled() async {
        let actor = GazeProcessingActor()
        await actor.startSession()
        var t = 0.0
        func push(_ s: FaceAnchorSnapshot) async { _ = await actor.processFaceAnchor(snapshot: s, elapsedSec: t); t += 1.0 / 60.0 }
        for _ in 0..<30 { await push(Self.face()) }
        for _ in 0..<12 { await push(Self.lost) }    // 200 ms
        for _ in 0..<30 { await push(Self.face()) }
        let (_, frames) = await actor.finalizeSession()
        XCTAssertFalse(frames.contains { $0.gaze.gapFilled == true })
    }

    func testFinalizeCarriesTrackingQualityAndClipTracking() async throws {
        let actor = GazeProcessingActor()
        await actor.startSession(battery: StimulusAOITracker.makeGeoPrefBattery(), stimulusId: "clip_a")
        var t = 0.0
        for _ in 0..<120 { _ = await actor.processFaceAnchor(snapshot: Self.face(), elapsedSec: t); t += 1.0 / 60.0 }
        await actor.switchStimulusClip(battery: StimulusAOITracker.makeFaceScanningBattery(), stimulusId: "clip_b")
        for _ in 0..<120 { _ = await actor.processFaceAnchor(snapshot: Self.face(), elapsedSec: t); t += 1.0 / 60.0 }
        let (summary, frames) = await actor.finalizeSession()

        XCTAssertEqual(frames.count, 240)
        let quality = try XCTUnwrap(summary.trackingQuality)
        XCTAssertEqual(quality["pipelineVersion"], .string("asd-gaze/1.0.0"))
        // No calibration ran in the pipeline and no accuracy was measured: honest verdict.
        XCTAssertEqual(quality["calibration"]?["quality"], .string("skipped"))
        XCTAssertEqual(quality["verdict"], .string("unusable"))
        XCTAssertNil(quality["estimatedAccuracyDeg"])
        XCTAssertEqual(quality["conditions"]?["ambientIntensityMedian"], .number(800))
        // QS-5: no in-session calibration, no distance (never a constant).
        XCTAssertNil(summary.calibratedDistanceCm)

        let clips = try XCTUnwrap(summary.clipTracking)
        XCTAssertEqual(clips.map(\.clipId), ["clip_a", "clip_b"])
        XCTAssertEqual(clips.map(\.order), [1, 2])
        XCTAssertEqual(try XCTUnwrap(clips[0].tracking.meanSampleConfidence), 1.0, accuracy: 1e-9)
        XCTAssertNil(clips[0].tracking.nearestCheckOffsetDeg, "no drift check followed the clip")
        // GeoPref boxes have built-in roles: their smallest side in degrees at 48 cm.
        XCTAssertNotNil(clips[0].tracking.aoiMinSizeDeg)
        // AOI hits come from the pipeline (GeoPref: the gaze at x = 0.5 is in both halves).
        XCTAssertFalse(frames[10].activeAOINames.isEmpty)
    }

    func testCalibrationProfileReachesThePipelineAsScreenAffine() async throws {
        let profile = GazeCalibrationProfile(a: 1, b: 0, c: 0.1, d: 0, e: 1, f: 0, sampleCount: 5, createdAt: Date())
        let model = try XCTUnwrap(GazePipelineDocuments.calibrationModel(profile))
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(model.utf8)) as? [String: Any])
        XCTAssertEqual(json["kind"] as? String, "screenAffine")
        XCTAssertEqual(json["x"] as? [Double], [1, 0, 0.1])
        XCTAssertNil(try GazePipelineDocuments.calibrationModel(.identity))

        // The fitted shift moves the stored point by 0.1 (no clamp, TRK-2).
        let plain = GazeProcessingActor()
        let shifted = GazeProcessingActor(calibrationProfile: profile)
        await plain.startSession()
        await shifted.startSession()
        for i in 0..<10 {
            _ = await plain.processFaceAnchor(snapshot: Self.face(), elapsedSec: Double(i) / 60)
            _ = await shifted.processFaceAnchor(snapshot: Self.face(), elapsedSec: Double(i) / 60)
        }
        let a = await plain.getVolatileFrames().last!.gaze
        let b = await shifted.getVolatileFrames().last!.gaze
        XCTAssertEqual(b.xNorm - a.xNorm, 0.1, accuracy: 1e-6)
        XCTAssertEqual(b.yNorm, a.yNorm, accuracy: 1e-9)
    }

    func testEndStimulusClipStopsBufferingUntilTheNextClip() async {
        let actor = GazeProcessingActor()
        await actor.startSession(stimulusId: "clip_a")
        _ = await actor.processFaceAnchor(snapshot: Self.face(), elapsedSec: 0)
        await actor.endStimulusClip()
        _ = await actor.processFaceAnchor(snapshot: Self.face(), elapsedSec: 0.1)
        let between = await actor.getRecordedFramesCount()
        XCTAssertEqual(between, 1, "drift-check frames are not clip frames")
        await actor.switchStimulusClip(battery: StimulusAOITracker(aois: []), stimulusId: "clip_b")
        _ = await actor.processFaceAnchor(snapshot: Self.face(), elapsedSec: 0.2)
        let after = await actor.getRecordedFramesCount()
        XCTAssertEqual(after, 2)
    }

    func testTargetAPIRunsAValidationWindow() async throws {
        let actor = GazeProcessingActor()
        await actor.startSession(beginsClip: false)
        try await actor.beginTarget(kind: .validation, at: GazeVec2(x: 0.5, y: 0.6))
        var t = 0.0
        var complete = false
        while !complete && t < 3 {
            _ = await actor.processFaceAnchor(snapshot: Self.face(), elapsedSec: t)
            t += 1.0 / 60.0
            complete = await actor.targetComplete()
        }
        XCTAssertTrue(complete)
        let ended = await actor.endTarget()
        let result = try XCTUnwrap(ended)
        XCTAssertEqual(result.kind, .validation)
        XCTAssertGreaterThan(result.validSamples, 0)
        let frames = await actor.getRecordedFramesCount()
        XCTAssertEqual(frames, 0, "target frames outside a clip are not buffered")
    }

    #if ASD_SIMULATION_ENABLED
    func testSimulatedSessionNeverRunsThePipeline() async throws {
        let actor = GazeProcessingActor()
        await actor.startSimulatedSession()
        for i in 0..<30 { _ = await actor.processSimulatedFrame(timeSec: Double(i) / 60, target: nil) }
        let (summary, frames) = await actor.finalizeSession()
        XCTAssertEqual(frames.count, 30)
        XCTAssertNil(summary.trackingQuality, "SIM-3: no trackingQuality for synthetic frames")
        XCTAssertNil(summary.clipTracking)
        XCTAssertTrue(frames.allSatisfy { $0.gaze.trackState == nil })
        do {
            try await actor.beginTarget(kind: .validation, at: GazeVec2(x: 0.5, y: 0.5))
            XCTFail("no pipeline in a simulated session")
        } catch {
            XCTAssertEqual(error as? GazeProcessingActor.TargetError, .noPipeline)
        }
    }
    #endif

    // MARK: - Documents

    func testScoreReportsTakeTheirClipsTrackingValues() {
        func report(_ id: String) -> PerVideoScoreReport {
            PerVideoScoreReport(stimulusId: id, title: id, category: "x", rows: [], subscore: 0,
                                validSampleRatio: 1, lowConfidence: false, hasBaseline: false, weight: 0)
        }
        let entries = [
            ClipTrackingEntry(clipId: "a", order: 1, tracking: ClipTracking(meanSampleConfidence: 0.9, aoiMinSizeDeg: 4)),
            ClipTrackingEntry(clipId: "b", order: 2, tracking: ClipTracking(meanSampleConfidence: 0.5)),
            ClipTrackingEntry(clipId: "b", order: 3, tracking: ClipTracking(meanSampleConfidence: 0.4))
        ]
        let merged = PerVideoScoreReport.merging([report("a"), report("b"), report("c")], clipTracking: entries)
        XCTAssertEqual(merged[0].meanSampleConfidence, 0.9)
        XCTAssertEqual(merged[0].aoiMinSizeDeg, 4)
        XCTAssertNil(merged[0].nearestCheckOffsetDeg)
        XCTAssertEqual(merged[1].meanSampleConfidence, 0.5, "a replayed clip takes the play whose order matches its position")
        XCTAssertNil(merged[2].meanSampleConfidence, "no entry, no value (CON-14)")
        XCTAssertEqual(merged[2], report("c"))
    }

    func testClipTrackingEntryReadsThePipelineJSON() throws {
        let json = #"[{"clipId":"a","order":1,"meanSampleConfidence":0.8,"nearestCheckOffsetDeg":1.25}]"#
        let entries = try JSONDecoder().decode([ClipTrackingEntry].self, from: Data(json.utf8))
        XCTAssertEqual(entries, [ClipTrackingEntry(clipId: "a", order: 1, tracking: ClipTracking(meanSampleConfidence: 0.8, nearestCheckOffsetDeg: 1.25))])
        let back = try JSONSerialization.jsonObject(with: JSONEncoder().encode(entries)) as? [[String: Any]]
        XCTAssertEqual(Set(back?.first?.keys.map { $0 } ?? []), ["clipId", "order", "meanSampleConfidence", "nearestCheckOffsetDeg"])
    }

    func testJSONValueRoundTripsADocument() throws {
        let json = #"{"a":[1,2.5,true,null,"x"],"b":{"c":0.022358037409427538}}"#
        let value = try JSONValue(json: json)
        XCTAssertEqual(try JSONValue(json: value.jsonString()), value)
        XCTAssertEqual(value["b"]?["c"], .number(0.022358037409427538))
    }

    func testSummaryWithTrackingQualityRoundTripsThroughCodable() throws {
        let summary = GazeSessionReportSummary.aggregate(
            frames: [],
            sessionId: "S",
            captureSource: .trueDepth,
            subjectAgeSignal: nil,
            trackingQuality: try JSONValue(json: #"{"verdict":"fair"}"#),
            clipTracking: [ClipTrackingEntry(clipId: "a", order: 1, tracking: ClipTracking())]
        )
        let decoded = try JSONDecoder().decode(GazeSessionReportSummary.self, from: JSONEncoder().encode(summary))
        XCTAssertEqual(decoded, summary)
    }
}
