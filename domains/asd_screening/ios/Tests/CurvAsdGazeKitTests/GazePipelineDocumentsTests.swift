import AsdCore
import XCTest
@testable import CurvAsdGazeKit

/// CFG-3 (WP-A.13, acceptance 24): the delivered `ScoringProfile` reaches the gaze pipeline
/// through Rust (`gazeConfigJsonForProfile`); the kit passes the profile JSON through and
/// assembles no gate or params JSON itself.
final class GazePipelineDocumentsTests: XCTestCase {
    private static let fourPointLayout: [[Double]] = [[0.5, 0.5], [0.12, 0.12], [0.88, 0.12], [0.12, 0.88]]

    /// The bundled profile with `trackingQuality.calibration.layout5` replaced (Swift edits
    /// the test document only; production profiles come signed from the backend).
    private static func profile(layout5: [[Double]]) throws -> String {
        var root = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(AsdCoreBridge.defaultScoringProfileJSON.utf8)) as? [String: Any]
        )
        var tracking = try XCTUnwrap(root["trackingQuality"] as? [String: Any])
        var calibration = try XCTUnwrap(tracking["calibration"] as? [String: Any])
        calibration["layout5"] = layout5
        tracking["calibration"] = calibration
        root["trackingQuality"] = tracking
        return String(decoding: try JSONSerialization.data(withJSONObject: root), as: UTF8.self)
    }

    /// The bundled profile with some `qualityGate` fields replaced (test document only).
    private static func profile(gate overrides: [String: Double]) throws -> String {
        var root = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(AsdCoreBridge.defaultScoringProfileJSON.utf8)) as? [String: Any]
        )
        var gate = try XCTUnwrap(root["qualityGate"] as? [String: Any])
        for (key, value) in overrides {
            XCTAssertNotNil(gate[key], "unknown gate field \(key)")
            gate[key] = value
        }
        root["qualityGate"] = gate
        return String(decoding: try JSONSerialization.data(withJSONObject: root), as: UTF8.self)
    }

    /// A clean tracked frame at `distanceCm`, as the pipeline would hand it to the writer.
    private static func processedSample(distanceCm: Double) -> ProcessedSampleRecord {
        ProcessedSampleRecord(
            tSec: 0.1,
            trackState: .tracked,
            rayStatus: nil,
            rawPointNorm: GazeVec2(x: 0.5, y: 0.5),
            calibratedPointNorm: GazeVec2(x: 0.5, y: 0.5),
            filteredPointNorm: GazeVec2(x: 0.5, y: 0.5),
            eyePosMm: GazeVec3(x: 0, y: 0, z: distanceCm * 10),
            distanceCm: distanceCm,
            headYawRad: 0,
            headPitchRad: 0,
            headRollRad: 0,
            headSpeedDegPerSec: 0,
            eyeSpeedDegPerSec: 0,
            leftEyeOpenness: 0.95,
            rightEyeOpenness: 0.95,
            ambientIntensity: nil,
            confidence: 0.95,
            valid: true,
            gapFilled: false,
            activeAoiNames: [],
            stimulusId: nil,
            clipMediaTimeSec: nil
        )
    }

    /// The actor's synthetic-frame gate is not a second input: it is the gate of the config
    /// Rust builds from the one profile, the same one the pipeline and writer use.
    func testNonDefaultProfileGateReachesPipelineWriterAndSyntheticGate() async throws {
        let profile = try Self.profile(gate: ["minDistanceCm": 20, "maxDistanceCm": 100])
        let actor = GazeProcessingActor(
            scoringProfileJSON: profile, deviceModelIdentifier: "iPhone17,3", orientation: .portrait)
        let defaultActor = GazeProcessingActor(
            scoringProfileJSON: AsdCoreBridge.defaultScoringProfileJSON,
            deviceModelIdentifier: "iPhone17,3", orientation: .portrait)

        // Synthetic-frame gate.
        XCTAssertEqual(actor.qualityGate.minDistanceCm, 20)
        XCTAssertEqual(actor.qualityGate.maxDistanceCm, 100)
        XCTAssertEqual(defaultActor.qualityGate.maxDistanceCm, GazeQualityGate().maxDistanceCm)

        // Pipeline config.
        let maybeConfig = await actor.pipelineConfigJSON()
        let config = try XCTUnwrap(maybeConfig)
        let configGate = try XCTUnwrap(try JSONValue(json: config)["qualityGate"])
        XCTAssertEqual(configGate["minDistanceCm"], .number(20))
        XCTAssertEqual(configGate["maxDistanceCm"], .number(100))

        // Frames-file writer: 80 cm passes the delivered gate, not the default one.
        let sample = Self.processedSample(distanceCm: 80)
        let maybeWritten = await actor.writeTelemetrySample(sample)
        let written = try XCTUnwrap(maybeWritten)
        XCTAssertEqual(written.failureReasons, [])
        XCTAssertEqual(written.qualityVerdict, "optimal")
        let maybeDefaultWritten = await defaultActor.writeTelemetrySample(sample)
        let defaultWritten = try XCTUnwrap(maybeDefaultWritten)
        XCTAssertNotEqual(defaultWritten.failureReasons, [], "the default gate rejects 80 cm")
        XCTAssertNotEqual(defaultWritten.qualityVerdict, "optimal")

        // Starting a session rebuilds the pipeline from the same profile.
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let maybeRestarted = await actor.pipelineConfigJSON()
        let restarted = try XCTUnwrap(try JSONValue(json: try XCTUnwrap(maybeRestarted))["qualityGate"])
        XCTAssertEqual(restarted, configGate)
    }

    @MainActor
    func testControllerGateIsTheActorsProfileGate() throws {
        let profile = try Self.profile(gate: ["maxDistanceCm": 90])
        let controller = GazeSessionController(scoringProfileJSON: profile)
        XCTAssertEqual(controller.qualityGate.maxDistanceCm, 90)
        XCTAssertEqual(controller.processingActor.qualityGate.maxDistanceCm, 90)
    }

    /// A 1.0.0 profile: no `trackingQuality`.
    private static func profile100() throws -> String {
        var root = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(AsdCoreBridge.defaultScoringProfileJSON.utf8)) as? [String: Any]
        )
        root["schemaVersion"] = "1.0.0"
        root.removeValue(forKey: "trackingQuality")
        return String(decoding: try JSONSerialization.data(withJSONObject: root), as: UTF8.self)
    }

    func testOneZeroZeroProfileGivesTheRustDefaultConfigByteForByte() throws {
        for model in ["iPhone15,2", "iPhone17,3", "iPad99,9"] {
            let config = try GazePipelineDocuments.config(
                modelIdentifier: model, orientation: .portrait, scoringProfileJSON: try Self.profile100())
            XCTAssertEqual(config, try gazeDefaultConfigJson(modelIdentifier: model, orientation: .portrait), model)
        }
    }

    func testBundledProfileGivesTheRustDefaultConfig() throws {
        let config = try GazePipelineDocuments.config(
            modelIdentifier: "iPhone17,3", orientation: .landscapeLeft,
            scoringProfileJSON: AsdCoreBridge.defaultScoringProfileJSON)
        XCTAssertEqual(config, try gazeDefaultConfigJson(modelIdentifier: "iPhone17,3", orientation: .landscapeLeft))
    }

    func testMalformedProfileIsAnInvalidJsonError() {
        XCTAssertThrowsError(try GazePipelineDocuments.config(
            modelIdentifier: "iPhone17,3", orientation: .portrait, scoringProfileJSON: "{\"qualityGate\": 1}")) { error in
            guard case AsdError.InvalidJson = error else {
                return XCTFail("expected InvalidJson, got \(error)")
            }
        }
    }

    func testWriterGateIsTheConfigGateVerbatim() throws {
        let config = try GazePipelineDocuments.config(
            modelIdentifier: "iPhone17,3", orientation: .portrait,
            scoringProfileJSON: AsdCoreBridge.defaultScoringProfileJSON)
        let gate = try XCTUnwrap(GazePipelineDocuments.qualityGateJSON(ofConfig: config))
        XCTAssertEqual(try JSONValue(json: gate), try JSONValue(json: config)["qualityGate"])
    }

    func testFourPointLayout5GivesAFourTargetCalibrationSchedule() async throws {
        let actor = GazeProcessingActor(scoringProfileJSON: try Self.profile(layout5: Self.fourPointLayout))
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let maybeSchedule = await actor.targetSchedule()
        let schedule = try XCTUnwrap(maybeSchedule)
        XCTAssertEqual(schedule.calibrationLayout.count, 4)
        XCTAssertEqual(schedule.calibrationLayout.map { [$0.x, $0.y] }, Self.fourPointLayout)
    }

    /// Acceptance 24: a scripted session with the four-point profile shows four calibration
    /// targets and its `trackingQuality.calibration` records four layout points.
    func testScriptedSessionWithFourPointProfileRecordsFourCalibrationTargets() async throws {
        let (model, looks) = try GazeTargetFlowTests.looks()
        let actor = GazeProcessingActor(
            scoringProfileJSON: try Self.profile(layout5: Self.fourPointLayout),
            deviceModelIdentifier: model,
            orientation: .portrait
        )
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let maybeSchedule = await actor.targetSchedule()
        let schedule = try XCTUnwrap(maybeSchedule)
        let viewer = GazeTargetFlowTests.SyntheticViewer(pipeline: actor, looks: looks)
        let log = GazeTargetFlowTests.Log()
        let outcome = try await GazeTargetFlowTests.runner(actor, viewer, schedule, shown: { await log.add($0) })
            .runCalibration()
        XCTAssertEqual(outcome.targets.count, 4)
        let shown = await log.shown
        XCTAssertEqual(shown.filter { $0.kind == .calibration }.count, 4)

        await viewer.play(clip: "clip_0", frames: 60)
        let (session, _) = await actor.finalizeSession()
        let quality = try XCTUnwrap(session.trackingQuality)
        XCTAssertEqual(quality["calibration"]?["layoutPoints"], .number(4))
    }
}
