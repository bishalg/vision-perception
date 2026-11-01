import AsdCore
import Foundation
import simd
import XCTest
@testable import CurvAsdGazeKit

/// WP-G.10: the screen sequence around the Rust pipeline's target API (CAL-7 .. CAL-9, VAL-1,
/// DRF-1). The flows only sequence; outcomes, quality and advice come from the pipeline.
final class GazeTargetFlowTests: XCTestCase {
    // MARK: - Schedule

    func testScheduleIsTheSpecLayoutAndTimingFromThePipelineConfig() async throws {
        let actor = GazeProcessingActor()
        await actor.startSession(beginsClip: false)
        let maybeSchedule = await actor.targetSchedule()
        let schedule = try XCTUnwrap(maybeSchedule)
        func pts(_ p: [GazeVec2]) -> [[Double]] { p.map { [$0.x, $0.y] } }
        XCTAssertEqual(pts(schedule.calibrationLayout), [[0.5, 0.5], [0.12, 0.12], [0.88, 0.12], [0.12, 0.88], [0.88, 0.88]], "CAL-7")
        XCTAssertEqual(schedule.calibrationLayout9.count, 9)
        XCTAssertEqual(pts(schedule.validationLayout), [[0.3, 0.3], [0.7, 0.3], [0.3, 0.7], [0.7, 0.7]], "VAL-1")
        XCTAssertTrue(Set(schedule.validationLayout).isDisjoint(with: Set(schedule.calibrationLayout)), "VAL-1: not calibration positions")
        XCTAssertEqual(schedule.calibration, .init(settleSec: 0.4, collectSec: 1.2), "CAL-8")
        XCTAssertEqual(schedule.validation, .init(settleSec: 0.4, collectSec: 1.0), "VAL-1")
        XCTAssertEqual(schedule.driftCheck, .init(settleSec: 0.3, collectSec: 0.9), "DRF-1")
        XCTAssertEqual(schedule.maxTargetWaitSec, 4.0)
        XCTAssertEqual(schedule.maxRetriesPerTarget, 1)
        XCTAssertGreaterThan(schedule.watchdogFrames(for: .calibration, frameRate: 60), Int(5.6 * 60))
    }

    #if ASD_SIMULATION_ENABLED
    func testSimulatedSessionHasNoScheduleAndTheRunnerReportsNoPipeline() async throws {
        let actor = GazeProcessingActor()
        await actor.startSimulatedSession()
        let schedule = await actor.targetSchedule()
        XCTAssertNil(schedule)
        let validation = await actor.validationSummary()
        XCTAssertNil(validation)
    }
    #endif

    // MARK: - Calibration flow

    static let none = GazeRecalAdviceRecord(action: .none, reasons: [], limitReached: false)
    static func recal(_ reasons: [GazeRecalReason], limit: Bool = false) -> GazeRecalAdviceRecord {
        GazeRecalAdviceRecord(action: .recalibrate, reasons: reasons, limitReached: limit)
    }

    func testGoodCalibrationGoesToValidationThenReady() {
        var flow = GazeCalibrationFlow()
        XCTAssertEqual(flow.phase, .calibrating(reason: nil))
        flow.calibrationFinished(.good, advice: Self.none)
        XCTAssertEqual(flow.phase, .validating, "VAL-1: validation after every calibration")
        flow.validationFinished(advice: Self.none, accuracyDeg: 1.2)
        XCTAssertEqual(flow.phase, .ready(continuedAnyway: false))
        XCTAssertEqual(flow.attempts, 1)
    }

    func testFailedCalibrationCannotProceedWithoutRetryOrContinueAnyway() {
        var flow = GazeCalibrationFlow()
        flow.calibrationFinished(.failed, advice: Self.recal([.calibrationQuality]))
        XCTAssertEqual(flow.phase, .calibrationResult(.failed, canRetry: true))
        // Validation results or a second finish never move a result screen on.
        flow.validationFinished(advice: Self.none, accuracyDeg: 1)
        flow.calibrationFinished(.good, advice: nil)
        XCTAssertFalse(flow.isReady)

        var retried = flow
        retried.retry()
        XCTAssertEqual(retried.phase, .calibrating(reason: .calibrationQuality), "the re-calibration carries its reason")
        XCTAssertEqual(retried.attempts, 2)

        flow.continueAnyway()
        XCTAssertEqual(flow.phase, .ready(continuedAnyway: true), "no model to validate after a failed calibration")
        XCTAssertTrue(flow.continuedAnyway)
    }

    func testPoorCalibrationOffersTryAgainAndContinueGoesThroughValidation() {
        var flow = GazeCalibrationFlow()
        flow.calibrationFinished(.poor, advice: Self.recal([.calibrationQuality]))
        XCTAssertEqual(flow.phase, .calibrationResult(.poor, canRetry: true), "acceptance 13: poor offers try again")
        flow.continueAnyway()
        XCTAssertEqual(flow.phase, .validating)
        // The accepted calibration-quality reason is not asked again after validation.
        flow.validationFinished(advice: Self.recal([.calibrationQuality]), accuracyDeg: 2)
        XCTAssertEqual(flow.phase, .ready(continuedAnyway: true))
    }

    func testSpentLimitHidesTryAgain() {
        var flow = GazeCalibrationFlow()
        flow.calibrationFinished(.failed, advice: Self.recal([.calibrationQuality], limit: true))
        XCTAssertFalse(flow.canRetry)
        flow.retry()
        XCTAssertEqual(flow.phase, .calibrationResult(.failed, canRetry: false), "retry is refused at the limit")
    }

    func testValidationAdviceAsksTheOperator() {
        var flow = GazeCalibrationFlow()
        flow.calibrationFinished(.fair, advice: Self.none)
        let advice = Self.recal([.validationAccuracy])
        flow.validationFinished(advice: advice, accuracyDeg: 6.2)
        XCTAssertEqual(flow.phase, .validationResult(advice, accuracyDeg: 6.2), "VAL-4")
        var retried = flow
        retried.retry()
        XCTAssertEqual(retried.phase, .calibrating(reason: .validationAccuracy))
        flow.continueAnyway()
        XCTAssertEqual(flow.phase, .ready(continuedAnyway: true))

        var warned = GazeCalibrationFlow()
        warned.calibrationFinished(.good, advice: nil)
        let warn = GazeRecalAdviceRecord(action: .warn, reasons: [.validationAccuracy], limitReached: false)
        warned.validationFinished(advice: warn, accuracyDeg: 4.0)
        XCTAssertEqual(warned.phase, .validationResult(warn, accuracyDeg: 4.0))
        warned.continueAnyway()
        XCTAssertEqual(warned.phase, .ready(continuedAnyway: false), "a warning is not going on against advice")
    }

    func testUnavailableCalibrationGoesOnUncalibrated() {
        var flow = GazeCalibrationFlow()
        flow.calibrationUnavailable()
        XCTAssertEqual(flow.phase, .ready(continuedAnyway: true))
    }

    // MARK: - Battery flow

    func testACheckTargetFollowsEveryClipIncludingTheLast() {
        var flow = GazeBatteryFlow(clipCount: 3)
        var seen: [GazeBatteryFlow.Step] = [flow.step]
        while flow.step != .finished {
            switch flow.step {
            case .clip: flow.clipEnded()
            case .driftCheck: flow.driftCheckFinished(advice: Self.none)
            default: XCTFail("unexpected \(flow.step)"); return
            }
            seen.append(flow.step)
        }
        XCTAssertEqual(seen, [
            .clip(0), .driftCheck(afterClip: 0),
            .clip(1), .driftCheck(afterClip: 1),
            .clip(2), .driftCheck(afterClip: 2),
            .finished
        ], "DRF-1: one check between every pair of clips and after the last")
    }

    func testRecalibrateAdviceMidClipIsDeferredToTheBoundary() {
        var flow = GazeBatteryFlow(clipCount: 2)
        flow.noteAdvice(Self.recal([.poseShift]))
        XCTAssertEqual(flow.step, .clip(0), "CAL-9: never acted on mid-clip")
        XCTAssertEqual(flow.deferredAdvice?.reasons, [.poseShift])
        flow.clipEnded()
        XCTAssertEqual(flow.step, .driftCheck(afterClip: 0), "the check still runs first")
        // The head came back by the check, but the clip's advice is still offered.
        flow.driftCheckFinished(advice: Self.none)
        XCTAssertEqual(flow.step, .recalibrationPrompt(afterClip: 0, reasons: [.poseShift]))
        XCTAssertNil(flow.deferredAdvice)
        flow.acceptRecalibration()
        XCTAssertEqual(flow.step, .recalibrating(afterClip: 0, reason: .poseShift))
        flow.recalibrationFinished()
        XCTAssertEqual(flow.step, .clip(1))
        XCTAssertEqual(flow.recalibrations, [.poseShift])
    }

    func testDriftAdviceAfterTheCheckPromptsAndCanBeDeclined() {
        var flow = GazeBatteryFlow(clipCount: 2)
        flow.clipEnded()
        flow.driftCheckFinished(advice: Self.recal([.drift]))
        XCTAssertEqual(flow.step, .recalibrationPrompt(afterClip: 0, reasons: [.drift]))
        flow.declineRecalibration()
        XCTAssertEqual(flow.step, .clip(1))
        XCTAssertTrue(flow.recalibrations.isEmpty)
    }

    func testNoPromptAfterTheLastClipOrAtTheLimit() {
        var last = GazeBatteryFlow(clipCount: 1)
        last.clipEnded()
        last.driftCheckFinished(advice: Self.recal([.drift]))
        XCTAssertEqual(last.step, .finished, "the battery is over; nothing left to re-calibrate for")

        var limited = GazeBatteryFlow(clipCount: 3)
        limited.noteAdvice(Self.recal([.poseShift], limit: true))
        XCTAssertNil(limited.deferredAdvice, "advice past the limit is not held")
        limited.noteAdvice(Self.recal([.poseShift]))
        limited.clipEnded()
        limited.driftCheckFinished(advice: Self.recal([.drift], limit: true))
        XCTAssertEqual(limited.step, .clip(1), "the current advice says the limit is spent")
    }

    func testFlowIgnoresEventsOutOfOrder() {
        var flow = GazeBatteryFlow(clipCount: 2)
        flow.driftCheckFinished(advice: Self.recal([.drift]))
        flow.acceptRecalibration()
        flow.recalibrationFinished()
        XCTAssertEqual(flow.step, .clip(0))
        flow.clipEnded()
        flow.clipEnded()
        XCTAssertEqual(flow.step, .driftCheck(afterClip: 0))
        flow.noteAdvice(Self.recal([.drift]))
        XCTAssertNil(flow.deferredAdvice, "advice is only held while a clip plays")
        XCTAssertEqual(GazeBatteryFlow(clipCount: 0).step, .finished)
    }

    // MARK: - Runner through the Rust pipeline

    /// A synthetic child who looks at whatever target is on screen. The look-at for each
    /// position is taken from the committed REP-1 log (`asd-cli gaze synth`), so the frames are
    /// the generator's, not geometry computed here.
    actor SyntheticViewer {
        let pipeline: GazeProcessingActor
        let looks: [(GazeVec2, FaceAnchorSnapshot)]
        private(set) var aim = GazeVec2(x: 0.5, y: 0.5)
        private(set) var t = 0.0
        /// Positions the child never looks at (face lost while they are shown).
        var ignored: [GazeVec2] = []

        init(pipeline: GazeProcessingActor, looks: [(GazeVec2, FaceAnchorSnapshot)]) {
            self.pipeline = pipeline
            self.looks = looks
        }

        func look(at p: GazeVec2) { aim = p }
        func ignore(_ p: GazeVec2) { ignored.append(p) }

        func push() async {
            let snapshot: FaceAnchorSnapshot
            if ignored.contains(aim) {
                snapshot = FaceAnchorSnapshot(isTracked: false, transform: matrix_identity_float4x4, lookAtPoint: simd_float3(0, 0, 0))
            } else {
                snapshot = looks.min { hypot($0.0.x - aim.x, $0.0.y - aim.y) < hypot($1.0.x - aim.x, $1.0.y - aim.y) }!.1
            }
            _ = await pipeline.processFaceAnchor(snapshot: snapshot, elapsedSec: t)
            t += 1.0 / 60.0
        }

        func play(clip: String, frames: Int) async {
            aim = GazeVec2(x: 0.5, y: 0.5)
            await pipeline.switchStimulusClip(battery: StimulusAOITracker(aois: []), stimulusId: clip)
            for i in 0..<frames {
                await pipeline.updateClipMediaTime(Double(i) / 60.0)
                await push()
            }
        }
    }

    /// One steady, open-eyed frame from the middle of each target window of the log.
    static func looks() throws -> (String, [(GazeVec2, FaceAnchorSnapshot)]) {
        let (header, lines) = try GazePipelineParityTests.load("blinks_and_loss_seed7_f32.jsonl")
        var looks: [(GazeVec2, FaceAnchorSnapshot)] = []
        for target in header.targets {
            let p = GazeVec2(x: target.xNorm, y: target.yNorm)
            guard !looks.contains(where: { $0.0 == p }) else { continue }
            let window = lines.filter {
                $0.face_tracked && $0.t_sec >= target.startSec + 0.5 && $0.t_sec <= target.endSec - 0.1
                    && ($0.eye_blink_left ?? 0) < 0.3 && ($0.eye_blink_right ?? 0) < 0.3
            }
            guard !window.isEmpty else { continue }
            looks.append((p, window[window.count / 2].snapshot))
        }
        return (header.deviceModel, looks)
    }

    static func runner(_ actor: GazeProcessingActor, _ viewer: SyntheticViewer, _ schedule: GazeTargetSchedule, shown: (@Sendable (GazeTargetPresentation) async -> Void)? = nil) -> GazeTargetRunner<GazeProcessingActor> {
        GazeTargetRunner(
            session: actor,
            schedule: schedule,
            onTargetShown: { p in
                await viewer.look(at: p.target)
                await shown?(p)
            },
            nextFrame: { await viewer.push() }
        )
    }

    actor Log {
        var shown: [GazeTargetPresentation] = []
        func add(_ p: GazeTargetPresentation) { shown.append(p) }
    }

    /// Acceptance for the main goal: a session that calibrates through the pipeline, validates,
    /// plays clips with drift checks and ends has a real calibration and accuracy, so its
    /// verdict is not `unusable` (it was `unusable` for every live session before WP-G.10).
    func testGoodSyntheticSessionEndsWithAUsableVerdict() async throws {
        let (model, looks) = try Self.looks()
        let actor = GazeProcessingActor(deviceModelIdentifier: model, orientation: .portrait)
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let maybeSchedule = await actor.targetSchedule()
        let schedule = try XCTUnwrap(maybeSchedule)
        let viewer = SyntheticViewer(pipeline: actor, looks: looks)
        let log = Log()
        let runner = Self.runner(actor, viewer, schedule, shown: { await log.add($0) })

        var calibration = GazeCalibrationFlow()
        let outcome = try await runner.runCalibration()
        XCTAssertTrue(outcome.applied)
        XCTAssertEqual(outcome.targets.count, 5)
        XCTAssertTrue(outcome.targets.allSatisfy { $0.outcome == .hit })
        calibration.calibrationFinished(outcome.quality, advice: await actor.recalibrationAdvice())
        XCTAssertEqual(calibration.phase, .validating, "quality \(outcome.quality)")

        let validation = try await runner.runValidation()
        XCTAssertEqual(validation.count, 4)
        let summary = await actor.validationSummary()
        XCTAssertEqual(summary?.complete, true)
        calibration.validationFinished(advice: await actor.recalibrationAdvice(), accuracyDeg: summary?.accuracyDeg)
        XCTAssertEqual(calibration.phase, .ready(continuedAnyway: false))

        var battery = GazeBatteryFlow(clipCount: 2)
        while battery.step != .finished {
            switch battery.step {
            case let .clip(i):
                await viewer.play(clip: "clip_\(i)", frames: 180)
                battery.noteAdvice(await actor.recalibrationAdvice())
                await actor.endStimulusClip()
                battery.clipEnded()
            case .driftCheck:
                let check = try await runner.runDriftCheck()
                XCTAssertEqual(check?.kind, .driftCheck)
                battery.driftCheckFinished(advice: await actor.recalibrationAdvice())
            default:
                XCTFail("unexpected step \(battery.step)")
                battery.abort()
            }
        }

        let shown = await log.shown
        XCTAssertEqual(shown.map(\.kind), Array(repeating: .calibration, count: 5) + Array(repeating: .validation, count: 4) + [.driftCheck, .driftCheck])
        XCTAssertEqual(shown.first?.windowSec ?? 0, 1.6, accuracy: 1e-9)

        let (session, frames) = await actor.finalizeSession()
        XCTAssertEqual(frames.count, 360, "only clip frames are recorded")
        let quality = try XCTUnwrap(session.trackingQuality)
        XCTAssertEqual(quality["calibration"]?["quality"], .string("good"))
        XCTAssertEqual(quality["calibration"]?["model"], .string("screenAffine"))
        XCTAssertNotNil(quality["validation"]?["accuracyDeg"])
        XCTAssertNotNil(quality["estimatedAccuracyDeg"])
        let verdict = quality["verdict"]
        XCTAssertNotEqual(verdict, .string("unusable"), "verdict \(String(describing: verdict))")
        XCTAssertTrue([JSONValue.string("good"), .string("fair")].contains(verdict ?? .null), "verdict \(String(describing: verdict))")
        XCTAssertNotNil(session.calibratedDistanceCm, "QS-5: an in-session calibration records its distance")
        let clips = try XCTUnwrap(session.clipTracking)
        XCTAssertEqual(clips.map(\.clipId), ["clip_0", "clip_1"])
        XCTAssertNotNil(clips[0].tracking.nearestCheckOffsetDeg, "each clip has its following check")
    }

    /// CAL-1: a calibration belongs to its session. The next session (a new wizard run, or
    /// another child) starts with a new pipeline and no calibration, and records `skipped`.
    func testCalibrationNeverCarriesIntoTheNextSession() async throws {
        let (model, looks) = try Self.looks()
        let actor = GazeProcessingActor(deviceModelIdentifier: model, orientation: .portrait)
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let maybeSchedule = await actor.targetSchedule()
        let schedule = try XCTUnwrap(maybeSchedule)
        let viewer = SyntheticViewer(pipeline: actor, looks: looks)
        let outcome = try await Self.runner(actor, viewer, schedule).runCalibration()
        XCTAssertTrue(outcome.applied)

        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: true)
        let status = await actor.calibrationStatus()
        XCTAssertNil(status)
        await viewer.play(clip: "next_run", frames: 60)
        let (summary, _) = await actor.finalizeSession()
        let quality = try XCTUnwrap(summary.trackingQuality)
        XCTAssertEqual(quality["calibration"]?["quality"], .string("skipped"))
        XCTAssertNil(summary.calibratedDistanceCm)
    }

    /// CAL-3 through the runner: a starved target is shown again by the pipeline (attempt 2),
    /// then marked missed, and the fit uses only measured targets.
    func testStarvedTargetIsReShownOnceThenMissed() async throws {
        let (model, looks) = try Self.looks()
        let actor = GazeProcessingActor(deviceModelIdentifier: model, orientation: .portrait)
        await actor.startSession(captureSource: .trueDepth, orientation: .portrait, beginsClip: false)
        let maybeSchedule = await actor.targetSchedule()
        let schedule = try XCTUnwrap(maybeSchedule)
        let viewer = SyntheticViewer(pipeline: actor, looks: looks)
        let starved = GazeVec2(x: 0.88, y: 0.88)
        await viewer.ignore(starved)
        let log = Log()
        let outcome = try await Self.runner(actor, viewer, schedule, shown: { await log.add($0) }).runCalibration()

        let shown = await log.shown
        let atStarved = shown.filter { $0.target == starved }
        XCTAssertEqual(atStarved.map(\.attempt), [1, 2], "re-shown exactly once")
        let last = try XCTUnwrap(outcome.targets.last)
        XCTAssertEqual(last.outcome, .missed)
        XCTAssertNil(last.rawMedianNorm, "no point is invented for a missed target")
        let record = try JSONValue(json: outcome.recordJson)
        XCTAssertEqual(record["missingPoints"], .number(1))
        XCTAssertEqual(record["usablePoints"], .number(4))
    }
}
