import XCTest
import CoreGraphics
import simd
@testable import CurvAsdGazeKit

/// CONC-1..4, CAM-2, PLC-5: one ordered ingress into the processing actor.
///
/// Every event (frames, calibration, AOI battery, clip clock, interruptions,
/// finalize) goes through `ARSessionBridge`, whose submit methods are synchronous
/// and thread-safe, so the order in which callers submit is the order in which
/// the actor sees them. These tests are deterministic and run under TSAN.
final class ConcurrencyOrderingTests: XCTestCase {
    // MARK: - Fixtures

    /// Tracked face at 48 cm looking left of centre: lands only in GeoPref `Social_Actor`.
    private static let socialSnapshot: FaceAnchorSnapshot = {
        var transform = matrix_identity_float4x4
        transform.columns.3 = simd_float4(0.0, 0.02, -0.48, 1.0)
        return FaceAnchorSnapshot(
            isTracked: true,
            transform: transform,
            lookAtPoint: simd_float3(-0.03, -0.05, 0.48),
            blendShapes: ["eyeBlinkLeft": 0.05, "eyeBlinkRight": 0.05]
        )
    }()

    private static let origin: TimeInterval = 1_000.0

    private func makeStartedBridge(
        sink: @escaping @Sendable (GazeUIState) -> Void = { _ in }
    ) -> (GazeProcessingActor, ARSessionBridge) {
        let actor = GazeProcessingActor()
        let bridge = ARSessionBridge(processingActor: actor, uiSink: sink)
        bridge.start(
            battery: .makeGeoPrefBattery(),
            profile: .identity,
            captureSource: .trueDepth,
            stimulusId: "clip-A",
            originTimestamp: Self.origin
        )
        return (actor, bridge)
    }

    // MARK: - CONC-4: stop cuts the stream exactly where it was requested

    func testThousandSnapshotsStopAtFiveHundredKeepsExactlyFiveHundred() async {
        let (actor, bridge) = makeStartedBridge()
        var pending: ARSessionBridge.PendingFinalize?
        for i in 0..<1000 {
            if i == 500 {
                pending = bridge.requestFinalize(at: Self.origin + Double(i) / 60.0)
            }
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
        }
        guard let pending else { return XCTFail("finalize was never requested") }
        let (summary, frames) = await pending.value

        XCTAssertEqual(frames.count, 500, "Frames before stop are kept, frames after are dropped")
        XCTAssertEqual(summary.totalFrames, 500)
        XCTAssertEqual(summary.captureSource, .trueDepth)
        // Arrival order is preserved: sample times strictly increase, first is t = 0.
        XCTAssertEqual(frames.first?.gaze.timestampMs ?? -1, 0.0, accuracy: 1e-6)
        XCTAssertEqual(frames.last?.gaze.timestampMs ?? -1, 499.0 / 60.0 * 1000.0, accuracy: 1e-6)
        for (a, b) in zip(frames, frames.dropFirst()) {
            XCTAssertLessThan(a.gaze.timestampMs, b.gaze.timestampMs)
        }
        // The stimulus attribution from `start` reaches the actor before the first frame.
        XCTAssertEqual(frames.first?.stimulusId, "clip-A")

        await bridge.drain()
        let recording = await actor.isRecording
        let buffered = await actor.getRecordedFramesCount()
        XCTAssertFalse(recording)
        XCTAssertEqual(buffered, 0, "Frames after stop must never be buffered")
    }

    func testStopRequestedFromProducerThreadIsDeterministic() async {
        let (_, bridge) = makeStartedBridge()
        // The AR delegate queue is a background serial queue; reproduce that.
        let pending: ARSessionBridge.PendingFinalize = await withCheckedContinuation { cont in
            DispatchQueue(label: "test.ar.delegate").async {
                var ticket: ARSessionBridge.PendingFinalize?
                for i in 0..<1000 {
                    if i == 500 { ticket = bridge.requestFinalize(at: Self.origin + 500.0 / 60.0) }
                    bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
                }
                cont.resume(returning: ticket!)
            }
        }
        let (summary, frames) = await pending.value
        XCTAssertEqual(frames.count, 500)
        XCTAssertEqual(summary.totalFrames, 500)
    }

    // MARK: - CONC-1, CONC-3: concurrent control updates while frames stream (TSAN)

    func testConcurrentCalibrationAOIAndClockUpdatesWhileProcessing() async {
        let (actor, bridge) = makeStartedBridge()
        let profile = GazeCalibrationProfile(
            a: 1.05, b: 0.0, c: 0.01, d: 0.0, e: 0.97, f: -0.01,
            sampleCount: 5, createdAt: Date()
        )

        // 10 s synthetic session at 60 Hz on a producer thread, while 30 Hz control
        // updates (calibration, AOI battery switches, clip clock) arrive from many
        // other threads at once.
        await withCheckedContinuation { (cont: CheckedContinuation<Void, Never>) in
            let group = DispatchGroup()
            group.enter()
            DispatchQueue(label: "test.ar.delegate").async {
                for i in 0..<600 {
                    bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
                }
                group.leave()
            }
            group.enter()
            DispatchQueue.global(qos: .userInitiated).async {
                DispatchQueue.concurrentPerform(iterations: 300) { i in
                    switch i % 4 {
                    case 0:
                        bridge.updateCalibration(profile)
                    case 1:
                        bridge.switchStimulusClip(
                            battery: i % 8 == 1 ? .makeFaceScanningBattery() : .makeGeoPrefBattery(),
                            stimulusId: "clip-\(i)",
                            at: Self.origin + Double(i) / 30.0
                        )
                    case 2:
                        bridge.updateClipMediaTime(Double(i) / 30.0)
                    default:
                        // Direct actor access from arbitrary threads is also race-free.
                        Task { await actor.updateClipMediaTime(Double(i) / 30.0) }
                    }
                }
                group.leave()
            }
            group.notify(queue: .global()) { cont.resume() }
        }

        let (summary, frames) = await bridge.requestFinalize(at: Self.origin + 10.0).value
        XCTAssertEqual(frames.count, 600, "Every frame submitted before stop is kept")
        XCTAssertEqual(summary.totalFrames, 600)
        XCTAssertEqual(summary.durationSec, 599.0 / 60.0, accuracy: 1e-6)
        let finalProfile = await actor.calibrationProfile
        XCTAssertEqual(finalProfile.a, 1.05, accuracy: 1e-9)
    }

    @MainActor
    func testControllerCalibrationUpdatesDuringSimulatedSessionAreRaceFree() async {
        #if ASD_SIMULATION_ENABLED
        let controller = GazeSessionController(calibrationProfile: .identity)
        controller.startSimulatedSession(battery: .makeGeoPrefBattery(), stimulusId: "clip-S")
        let actor = controller.processingActor
        let clockUpdates = Task.detached {
            for i in 0..<300 {
                await actor.updateClipMediaTime(Double(i) / 30.0)
            }
        }
        for i in 0..<600 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
            if i % 2 == 0 {
                controller.updateClipMediaTime(Double(i) / 60.0)
            }
            if i % 120 == 0 {
                controller.switchStimulusClip(battery: .makeGeoPrefBattery(), stimulusId: "clip-\(i)")
            }
        }
        await clockUpdates.value
        let summary = await controller.stopSession()
        XCTAssertEqual(summary.totalFrames, 600)
        XCTAssertEqual(controller.recordedFrames.count, 600)
        XCTAssertEqual(summary.captureSource, .simulated)
        #endif
    }

    @MainActor
    func testControllerFramesAfterStopAreDropped() async {
        #if ASD_SIMULATION_ENABLED
        let controller = GazeSessionController(calibrationProfile: .identity)
        controller.startSimulatedSession()
        for i in 0..<500 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        let summary = await controller.stopSession()
        for i in 500..<1000 {
            controller.tickSimulatedFrame(timeSec: Double(i) / 60.0)
        }
        await controller.flushPipeline()
        XCTAssertEqual(summary.totalFrames, 500)
        XCTAssertEqual(controller.recordedFrames.count, 500)
        let buffered = await controller.processingActor.getRecordedFramesCount()
        XCTAssertEqual(buffered, 0)
        XCTAssertFalse(controller.isRecording)
        #endif
    }

    // MARK: - PLC-5: durations and dwell from timestamps, dwell capped at 100 ms

    func testThirtyFpsDurationAndDwellComeFromTimestamps() async throws {
        let (_, bridge) = makeStartedBridge()
        for i in 0..<90 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 30.0)
        }
        let (summary, frames) = await bridge.requestFinalize(at: Self.origin + 3.0).value

        XCTAssertEqual(summary.totalFrames, 90)
        // 60 fps assumption would give 1.5 s; the timestamps span 89/30 s.
        XCTAssertEqual(summary.durationSec, 89.0 / 30.0, accuracy: 1e-6)
        XCTAssertEqual(summary.socialDwellSec, 89.0 / 30.0, accuracy: 1e-6)
        XCTAssertEqual(summary.geometricDwellSec, 0.0, accuracy: 1e-9)
        // aoiDwellMs holds the measured inter-frame interval, not 16.67.
        XCTAssertEqual(frames[0].aoiDwellMs, 0.0, accuracy: 1e-9)
        XCTAssertEqual(frames[1].aoiDwellMs, 1000.0 / 30.0, accuracy: 1e-6)
        XCTAssertEqual(frames[89].aoiDwellMs, 1000.0 / 30.0, accuracy: 1e-6)
    }

    func testDwellContributionCappedAtOneHundredMilliseconds() async {
        let (_, bridge) = makeStartedBridge()
        // 0.0 ... 0.5 s at 30 fps (16 frames), a 2 s gap, then 15 more frames.
        var timestamps: [Double] = (0..<16).map { Double($0) / 30.0 }
        let resume = timestamps.last! + 2.0
        timestamps += (0..<15).map { resume + Double($0) / 30.0 }
        for t in timestamps {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + t)
        }
        let (summary, frames) = await bridge.requestFinalize(at: Self.origin + timestamps.last!).value

        XCTAssertEqual(frames.count, 31)
        XCTAssertEqual(summary.durationSec, timestamps.last! - timestamps.first!, accuracy: 1e-6)
        // 15 + 14 regular intervals of 1/30 s, plus the gap capped at 0.1 s.
        let expectedDwell = 29.0 / 30.0 + 0.1
        XCTAssertEqual(summary.socialDwellSec, expectedDwell, accuracy: 1e-6)
        XCTAssertEqual(frames[16].aoiDwellMs, 2000.0, accuracy: 1e-6, "aoiDwellMs records the measured interval, uncapped")
    }

    // MARK: - CAM-2: interruptions and failures

    func testInterruptionsAreCountedTimedAndCapVerdict() async {
        let collector = UIStateCollector()
        let (_, bridge) = makeStartedBridge(sink: { collector.append($0) })

        for i in 0..<60 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
        }
        bridge.interruptionBegan(at: Self.origin + 1.0)
        // Frames delivered while interrupted are not appended.
        for i in 0..<30 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + 1.0 + Double(i) / 60.0)
        }
        bridge.interruptionEnded(at: Self.origin + 3.0)
        for i in 0..<60 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + 3.0 + Double(i) / 60.0)
        }
        // Second interruption still open when the session stops.
        bridge.interruptionBegan(at: Self.origin + 5.0)
        let (summary, frames) = await bridge.requestFinalize(at: Self.origin + 6.0).value

        XCTAssertEqual(frames.count, 120)
        XCTAssertEqual(summary.interruptionCount, 2)
        XCTAssertEqual(summary.interruptedSec, 3.0, accuracy: 1e-6)
        XCTAssertGreaterThan(summary.validRatio, 0.9, "All kept frames are valid ...")
        XCTAssertEqual(summary.qualityVerdict, .degraded, "... but an interrupted session is at best degraded")
        XCTAssertTrue(
            collector.stages.contains(.unavailable(reason: .interrupted)),
            "The UI learns about the interruption"
        )
    }

    func testSessionFailureStopsAppendingAndIsReported() async {
        let collector = UIStateCollector()
        let (_, bridge) = makeStartedBridge(sink: { collector.append($0) })
        for i in 0..<30 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
        }
        bridge.sessionFailed(at: Self.origin + 0.5)
        for i in 30..<60 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
        }
        let (summary, frames) = await bridge.requestFinalize(at: Self.origin + 2.0).value
        XCTAssertEqual(frames.count, 30)
        XCTAssertEqual(summary.interruptionCount, 1)
        XCTAssertEqual(summary.interruptedSec, 1.5, accuracy: 1e-6)
        XCTAssertNotEqual(summary.qualityVerdict, .optimal)
        XCTAssertNotEqual(summary.qualityVerdict, .acceptable)
        XCTAssertEqual(collector.stages.last, .unavailable(reason: .sessionFailed))
    }

    func testUninterruptedSessionHasZeroInterruptions() async {
        let (_, bridge) = makeStartedBridge()
        for i in 0..<60 {
            bridge.submit(Self.socialSnapshot, timestamp: Self.origin + Double(i) / 60.0)
        }
        let (summary, _) = await bridge.requestFinalize(at: Self.origin + 1.0).value
        XCTAssertEqual(summary.interruptionCount, 0)
        XCTAssertEqual(summary.interruptedSec, 0)
        XCTAssertEqual(summary.qualityVerdict, .optimal)
    }

    // MARK: - Honest empty sessions

    func testFinalizeWithoutStartIsQuarantinedNotLive() async {
        let actor = GazeProcessingActor()
        let (summary, frames) = await actor.finalizeSession()
        XCTAssertTrue(frames.isEmpty)
        // Nothing was captured: never claim TrueDepth. Unknown provenance is
        // quarantined as `.simulated`, the same rule the app applies to records.
        XCTAssertEqual(summary.captureSource, .simulated, "Nothing was captured: never claim TrueDepth")
        XCTAssertEqual(summary.qualityVerdict, .rejected)
    }

    @MainActor
    func testControllerStopWithoutStartIsQuarantinedAndNotPersisted() async throws {
        let controller = GazeSessionController(calibrationProfile: .identity)
        let summary = await controller.stopSession()
        XCTAssertEqual(summary.captureSource, .simulated, "Never-started stop must not claim live data")
        XCTAssertEqual(summary.totalFrames, 0)
        XCTAssertEqual(summary.qualityVerdict, .rejected)

        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let vault = SessionVaultManager(vaultDirectory: tempDir, keyData: Data(repeating: 3, count: 32))
        do {
            _ = try await controller.stopAndPersistBatchAsync(participantToken: "child-TEST0000", vault: vault)
            XCTFail("An empty, never-started session must not be written to the vault")
        } catch GazeSessionError.nothingCaptured {
            // expected
        }
        let written = (try? FileManager.default.contentsOfDirectory(atPath: tempDir.path)) ?? []
        XCTAssertTrue(written.isEmpty)
    }

    @MainActor
    func testSecondStopDoesNotReplayPreviousSession() async {
        #if ASD_SIMULATION_ENABLED
        let controller = GazeSessionController(calibrationProfile: .identity)
        controller.startSimulatedSession()
        for i in 0..<10 { controller.tickSimulatedFrame(timeSec: Double(i) / 60.0) }
        let first = await controller.stopSession()
        let second = await controller.stopSession()
        XCTAssertEqual(first.totalFrames, 10)
        XCTAssertEqual(second.totalFrames, 0)
        XCTAssertEqual(second.captureSource, .simulated, "No session running: quarantined, not live")
        #endif
    }

    // MARK: - CONC-2

    func testBridgeIsSendable() {
        let bridge = ARSessionBridge(processingActor: GazeProcessingActor())
        let sendable: any Sendable = bridge
        XCTAssertNotNil(sendable)
    }
}

/// Thread-safe collector for UI states posted by the pipeline.
private final class UIStateCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var _stages: [TrackingStage] = []

    func append(_ state: GazeUIState) {
        lock.lock()
        _stages.append(state.trackingStage)
        lock.unlock()
    }

    var stages: [TrackingStage] {
        lock.lock()
        defer { lock.unlock() }
        return _stages
    }
}
