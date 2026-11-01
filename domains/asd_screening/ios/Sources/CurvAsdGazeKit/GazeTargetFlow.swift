import AsdCore
import Foundation

// WP-G.10: the app side of calibration (CAL-7, CAL-8), validation (VAL-1), drift checks (DRF-1)
// and re-calibration prompts (CAL-9).
//
// Everything that decides something about gaze lives in Rust (`asd-core::gaze`): when a target
// window has collected enough frame time, whether a target was hit, needs a re-show or was
// missed, the calibration fit and its quality, validation accuracy, drift and the advice.
// This file only sequences the screens: it shows the target the pipeline asks for, waits for
// the pipeline to close the window, and moves between calibration, validation and the clips
// on the pipeline's outcome and advice.

// MARK: - Schedule

/// Target layouts and window timing of a session pipeline (`TrackingQualityParams`), read from
/// the pipeline's own config so the UI animates exactly the windows Rust measures.
public struct GazeTargetSchedule: Sendable, Equatable {
    /// One target window: settle time (samples dropped) then collect time.
    public struct Window: Sendable, Equatable {
        public let settleSec: Double
        public let collectSec: Double
        public var windowSec: Double { settleSec + collectSec }

        public init(settleSec: Double, collectSec: Double) {
            self.settleSec = settleSec
            self.collectSec = collectSec
        }
    }

    /// Default calibration layout (CAL-7, 5 points).
    public let calibrationLayout: [GazeVec2]
    /// Optional 9-point calibration layout (CAL-7).
    public let calibrationLayout9: [GazeVec2]
    /// Validation layout (VAL-1), deliberately not calibration positions.
    public let validationLayout: [GazeVec2]
    public let calibration: Window
    public let validation: Window
    public let driftCheck: Window
    /// A calibration window pauses while tracking is lost and gives up after this (CAL-8).
    public let maxTargetWaitSec: Double
    /// Re-shows of a starved calibration target (CAL-3: once).
    public let maxRetriesPerTarget: Int

    public init(
        calibrationLayout: [GazeVec2],
        calibrationLayout9: [GazeVec2],
        validationLayout: [GazeVec2],
        calibration: Window,
        validation: Window,
        driftCheck: Window,
        maxTargetWaitSec: Double,
        maxRetriesPerTarget: Int
    ) {
        self.calibrationLayout = calibrationLayout
        self.calibrationLayout9 = calibrationLayout9
        self.validationLayout = validationLayout
        self.calibration = calibration
        self.validation = validation
        self.driftCheck = driftCheck
        self.maxTargetWaitSec = maxTargetWaitSec
        self.maxRetriesPerTarget = maxRetriesPerTarget
    }

    private struct Config: Decodable {
        struct Params: Decodable {
            struct Calibration: Decodable {
                let settleMs: Double
                let collectMs: Double
                let maxTargetWaitMs: Double
                let maxRetriesPerTarget: Int
                let layout5: [[Double]]
                let layout9: [[Double]]
            }
            struct Validation: Decodable {
                let layout: [[Double]]
                let settleMs: Double
                let collectMs: Double
            }
            struct Drift: Decodable {
                let settleMs: Double
                let collectMs: Double
            }
            let calibration: Calibration
            let validation: Validation
            let drift: Drift
        }
        let params: Params
    }

    /// Reads the schedule from a `PipelineConfig` JSON (`GazePipelineHandle.configJson()`).
    public init(configJSON: String) throws {
        let p = try JSONDecoder().decode(Config.self, from: Data(configJSON.utf8)).params
        func points(_ raw: [[Double]]) throws -> [GazeVec2] {
            try raw.map { pair in
                guard pair.count == 2 else {
                    throw AsdError.InvalidJson(message: "layout point is not [x, y]")
                }
                return GazeVec2(x: pair[0], y: pair[1])
            }
        }
        self.init(
            calibrationLayout: try points(p.calibration.layout5),
            calibrationLayout9: try points(p.calibration.layout9),
            validationLayout: try points(p.validation.layout),
            calibration: Window(settleSec: p.calibration.settleMs / 1000, collectSec: p.calibration.collectMs / 1000),
            validation: Window(settleSec: p.validation.settleMs / 1000, collectSec: p.validation.collectMs / 1000),
            driftCheck: Window(settleSec: p.drift.settleMs / 1000, collectSec: p.drift.collectMs / 1000),
            maxTargetWaitSec: p.calibration.maxTargetWaitMs / 1000,
            maxRetriesPerTarget: max(0, p.calibration.maxRetriesPerTarget)
        )
    }

    public func window(for kind: GazeTargetKind) -> Window {
        switch kind {
        case .calibration: return calibration
        case .validation: return validation
        case .driftCheck: return driftCheck
        }
    }

    /// Display frames after which the runner closes a window the pipeline has not closed.
    /// The pipeline closes every window itself on frame time; this only guards against a
    /// camera that stops delivering frames, so it is generous (wait limit + window + 2 s).
    public func watchdogFrames(for kind: GazeTargetKind, frameRate: Double) -> Int {
        let seconds = maxTargetWaitSec + window(for: kind).windowSec + 2
        return Int((seconds * max(1, frameRate)).rounded(.up))
    }
}

/// Validation of the calibration in effect, as the pipeline recorded it (VAL-2, VAL-3).
public struct GazeValidationSummary: Sendable, Equatable, Decodable {
    /// At least `minTargetsHit` targets were hit.
    public let complete: Bool
    /// Median accuracy over hit targets, degrees; `nil` when no target was hit.
    public let accuracyDeg: Double?
    /// Median RMS sample-to-sample precision over hit targets, degrees.
    public let precisionRmsDeg: Double?
    public let dataLoss: Double

    public init(complete: Bool, accuracyDeg: Double?, precisionRmsDeg: Double?, dataLoss: Double) {
        self.complete = complete
        self.accuracyDeg = accuracyDeg
        self.precisionRmsDeg = precisionRmsDeg
        self.dataLoss = dataLoss
    }
}

// MARK: - Session

/// The pipeline's target API (PIPE-1). `GazeSessionController` (app) and `GazeProcessingActor`
/// (tests, replay) both provide it; every call is ordered with the frames already submitted.
public protocol GazeTargetSession: Sendable {
    func startCalibration(layout: [GazeVec2]?, reason: GazeRecalReason?) async throws
    func calibrationStatus() async -> GazeCalibrationStatus?
    func beginTarget(kind: GazeTargetKind, at target: GazeVec2) async throws
    func targetComplete() async -> Bool
    func endTarget() async -> GazeTargetResultRecord?
    func finishCalibration() async throws -> GazeCalibrationOutcomeRecord
    func recalibrationAdvice() async -> GazeRecalAdviceRecord?
    func nextCheckTarget() async -> GazeVec2?
    func validationSummary() async -> GazeValidationSummary?
    func targetSchedule() async -> GazeTargetSchedule?
}

extension GazeProcessingActor: GazeTargetSession {}
// `GazeSessionController` conforms in its own file (Sendable conformance rule).

// MARK: - Runner

/// One target on screen: what to draw, where, and for how long the window is.
public struct GazeTargetPresentation: Sendable, Equatable {
    public let kind: GazeTargetKind
    /// Screen-normalised position (0...1, origin top left).
    public let target: GazeVec2
    /// Position in its layout (calibration and validation); 0 for a drift check.
    public let index: Int
    /// Number of targets in the layout (1 for a drift check).
    public let count: Int
    /// 1 for the first showing, 2 for a CAL-3 re-show.
    public let attempt: Int
    /// Nominal window (settle + collect) for the shrink animation; the pipeline may hold the
    /// window open longer while tracking is lost (CAL-8 pause, up to `maxTargetWaitSec`).
    public let windowSec: Double

    public init(kind: GazeTargetKind, target: GazeVec2, index: Int, count: Int, attempt: Int, windowSec: Double) {
        self.kind = kind
        self.target = target
        self.index = index
        self.count = count
        self.attempt = attempt
        self.windowSec = windowSec
    }
}

public enum GazeTargetRunnerError: Error, Equatable {
    /// The pipeline kept asking for a target that never received a frame (camera stalled).
    case calibrationDidNotProgress
    /// No calibration is running in the pipeline (it was never started or was replaced).
    case noCalibration
}

/// Shows the targets the pipeline asks for and waits for the pipeline to close each window.
///
/// The runner never measures anything: the pipeline decides when a window is complete and what
/// its outcome is (CAL-3 re-show, missed, hit). `nextFrame` paces the polling: the app sleeps
/// one display frame, tests push one synthetic camera frame.
public struct GazeTargetRunner<Session: GazeTargetSession>: Sendable {
    public let session: Session
    public let schedule: GazeTargetSchedule
    public let frameRate: Double
    public let onTargetShown: @Sendable (GazeTargetPresentation) async -> Void
    public let onTargetEnded: @Sendable (GazeTargetPresentation, GazeTargetResultRecord?) async -> Void
    public let nextFrame: @Sendable () async -> Void

    public init(
        session: Session,
        schedule: GazeTargetSchedule,
        frameRate: Double = 60,
        onTargetShown: @escaping @Sendable (GazeTargetPresentation) async -> Void = { _ in },
        onTargetEnded: @escaping @Sendable (GazeTargetPresentation, GazeTargetResultRecord?) async -> Void = { _, _ in },
        nextFrame: @escaping @Sendable () async -> Void
    ) {
        self.session = session
        self.schedule = schedule
        self.frameRate = frameRate
        self.onTargetShown = onTargetShown
        self.onTargetEnded = onTargetEnded
        self.nextFrame = nextFrame
    }

    /// Shows one target and runs its window until the pipeline closes it.
    @discardableResult
    public func runWindow(_ presentation: GazeTargetPresentation) async throws -> GazeTargetResultRecord? {
        try Task.checkCancellation()
        await onTargetShown(presentation)
        try await session.beginTarget(kind: presentation.kind, at: presentation.target)
        let limit = schedule.watchdogFrames(for: presentation.kind, frameRate: frameRate)
        var frames = 0
        while !(await session.targetComplete()) && frames < limit {
            if Task.isCancelled {
                _ = await session.endTarget()
                throw CancellationError()
            }
            await nextFrame()
            frames += 1
        }
        let result = await session.endTarget()
        await onTargetEnded(presentation, result)
        return result
    }

    /// Runs a (re-)calibration through the pipeline (CAL-3 .. CAL-8) and returns its fitted,
    /// scored outcome. Re-shows and missed targets are the pipeline's decision.
    public func runCalibration(layout: [GazeVec2]? = nil, reason: GazeRecalReason? = nil) async throws -> GazeCalibrationOutcomeRecord {
        let points = layout ?? schedule.calibrationLayout
        try await session.startCalibration(layout: layout, reason: reason)
        // Every target can be shown 1 + maxRetries times; one spare for a window that saw no frame.
        let maxWindows = points.count * (schedule.maxRetriesPerTarget + 1) + 1
        var windows = 0
        while let status = await session.calibrationStatus() {
            try Task.checkCancellation()
            switch status {
            case .complete:
                return try await session.finishCalibration()
            case let .pending(index, target, attempt):
                guard windows < maxWindows else { throw GazeTargetRunnerError.calibrationDidNotProgress }
                windows += 1
                try await runWindow(GazeTargetPresentation(
                    kind: .calibration,
                    target: target,
                    index: Int(index),
                    count: points.count,
                    attempt: Int(attempt),
                    windowSec: schedule.calibration.windowSec
                ))
            case .running:
                // A window left open by an interrupted run: close it and continue.
                _ = await session.endTarget()
            }
        }
        throw GazeTargetRunnerError.noCalibration
    }

    /// Runs the validation layout after a calibration (VAL-1) and returns each target's result.
    public func runValidation() async throws -> [GazeTargetResultRecord] {
        let layout = schedule.validationLayout
        var results: [GazeTargetResultRecord] = []
        for (index, point) in layout.enumerated() {
            if let result = try await runWindow(GazeTargetPresentation(
                kind: .validation,
                target: point,
                index: index,
                count: layout.count,
                attempt: 1,
                windowSec: schedule.validation.windowSec
            )) {
                results.append(result)
            }
        }
        return results
    }

    /// Runs one drift check at the pipeline's next check position (DRF-1).
    public func runDriftCheck() async throws -> GazeTargetResultRecord? {
        guard let target = await session.nextCheckTarget() else {
            throw GazeProcessingActor.TargetError.noPipeline
        }
        return try await runWindow(GazeTargetPresentation(
            kind: .driftCheck,
            target: target,
            index: 0,
            count: 1,
            attempt: 1,
            windowSec: schedule.driftCheck.windowSec
        ))
    }
}

// MARK: - Calibration flow (before the battery)

/// Screen sequence of one calibration round: calibration, its result when it needs the
/// operator, validation, its result when it needs the operator, then ready (CAL-9.1, VAL-1,
/// VAL-4). Transitions follow the pipeline's quality and advice; nothing is computed here.
public struct GazeCalibrationFlow: Sendable, Equatable {
    public enum Phase: Sendable, Equatable {
        /// Calibration targets are on screen; `reason` is recorded for a re-calibration.
        case calibrating(reason: GazeRecalReason?)
        /// Quality `poor` or `failed`: offer "try again" (when the limit allows) or "continue".
        case calibrationResult(GazeCalibrationQuality, canRetry: Bool)
        /// Validation targets are on screen.
        case validating
        /// Validation advice is `warn` or `recalibrate`: the operator decides.
        case validationResult(GazeRecalAdviceRecord, accuracyDeg: Double?)
        /// Done; the battery may start. `continuedAnyway` when the operator went on against advice.
        case ready(continuedAnyway: Bool)
    }

    public private(set) var phase: Phase
    /// Calibration attempts in this round (1 + retries).
    public private(set) var attempts: Int = 1
    /// The operator chose to go on against advice at least once in this round.
    public private(set) var continuedAnyway: Bool = false

    public init(reason: GazeRecalReason? = nil) {
        phase = .calibrating(reason: reason)
    }

    public var isReady: Bool {
        if case .ready = phase { return true }
        return false
    }

    /// The pipeline fitted and scored the calibration. `advice` is read right after it.
    public mutating func calibrationFinished(_ quality: GazeCalibrationQuality, advice: GazeRecalAdviceRecord?) {
        guard case .calibrating = phase else { return }
        switch quality {
        case .good, .fair:
            phase = .validating
        case .poor, .failed, .skipped:
            phase = .calibrationResult(quality, canRetry: !(advice?.limitReached ?? false))
        }
    }

    /// No live pipeline (simulated session) or the calibration could not run at all: the
    /// session records `skipped` and goes on without one (CAL-1).
    public mutating func calibrationUnavailable() {
        phase = .ready(continuedAnyway: true)
        continuedAnyway = true
    }

    /// Validation ended; `advice` and `accuracyDeg` come from the pipeline (VAL-4).
    public mutating func validationFinished(advice: GazeRecalAdviceRecord?, accuracyDeg: Double?) {
        guard phase == .validating else { return }
        guard var advice, advice.action != .none else {
            phase = .ready(continuedAnyway: continuedAnyway)
            return
        }
        if continuedAnyway {
            // The operator already accepted this calibration's quality; ask only about new reasons.
            advice.reasons.removeAll { $0 == .calibrationQuality }
            if advice.reasons.isEmpty {
                phase = .ready(continuedAnyway: true)
                return
            }
        }
        phase = .validationResult(advice, accuracyDeg: accuracyDeg)
    }

    /// Whether the result on screen offers "try again".
    public var canRetry: Bool {
        switch phase {
        case let .calibrationResult(_, canRetry): return canRetry
        case let .validationResult(advice, _): return !advice.limitReached
        default: return false
        }
    }

    /// "Try again" on a result screen: a new calibration with the reason that asked for it.
    public mutating func retry() {
        guard canRetry else { return }
        switch phase {
        case .calibrationResult:
            phase = .calibrating(reason: .calibrationQuality)
        case let .validationResult(advice, _):
            phase = .calibrating(reason: advice.reasons.first ?? .operatorRequest)
        default:
            return
        }
        attempts += 1
    }

    /// "Continue anyway" on a result screen. A `poor` calibration still gets its validation; a
    /// `failed` one has no model to validate and goes straight on. The session record keeps the
    /// calibration quality, so the choice is visible in `trackingQuality`.
    public mutating func continueAnyway() {
        switch phase {
        case let .calibrationResult(quality, _):
            continuedAnyway = true
            phase = quality == .poor ? .validating : .ready(continuedAnyway: true)
        case let .validationResult(advice, _):
            if advice.action == .recalibrate { continuedAnyway = true }
            phase = .ready(continuedAnyway: continuedAnyway)
        default:
            return
        }
    }
}

// MARK: - Battery flow (clips, drift checks, re-calibration prompts)

/// Screen sequence of a stimulus battery: every clip is followed by one drift check (DRF-1,
/// also after the last clip); a re-calibration is offered only at a clip boundary, after the
/// check, never mid-clip (CAL-9), and never after the last clip.
public struct GazeBatteryFlow: Sendable, Equatable {
    public enum Step: Sendable, Equatable {
        case clip(Int)
        case driftCheck(afterClip: Int)
        case recalibrationPrompt(afterClip: Int, reasons: [GazeRecalReason])
        case recalibrating(afterClip: Int, reason: GazeRecalReason)
        case finished
    }

    public let clipCount: Int
    public private(set) var step: Step
    /// Advice that asked for a re-calibration while a clip was playing; held for the boundary.
    public private(set) var deferredAdvice: GazeRecalAdviceRecord?
    /// Reasons of the re-calibrations run inside the battery, in order.
    public private(set) var recalibrations: [GazeRecalReason] = []

    public init(clipCount: Int) {
        self.clipCount = max(0, clipCount)
        step = clipCount > 0 ? .clip(0) : .finished
    }

    /// Advice read while a clip plays. A `recalibrate` is never acted on here; it is held and
    /// offered after the clip's drift check (CAL-9: only at a clip boundary).
    public mutating func noteAdvice(_ advice: GazeRecalAdviceRecord?) {
        guard case .clip = step, let advice, advice.action == .recalibrate, !advice.limitReached else { return }
        deferredAdvice = advice
    }

    /// The playing clip ended: its drift check comes next.
    public mutating func clipEnded() {
        guard case let .clip(index) = step else { return }
        step = .driftCheck(afterClip: index)
    }

    /// The drift check ended; `advice` is read right after it. A re-calibration is offered when
    /// the current advice, or advice held from the clip, asks for one and the limit allows.
    public mutating func driftCheckFinished(advice: GazeRecalAdviceRecord?) {
        guard case let .driftCheck(index) = step else { return }
        let held = deferredAdvice
        deferredAdvice = nil
        let isLast = index + 1 >= clipCount
        // The current advice knows the limit best: once it is spent, nothing is offered.
        let limitSpent = advice?.limitReached ?? false
        var reasons: [GazeRecalReason] = []
        if let advice, advice.action == .recalibrate { reasons += advice.reasons }
        if let held { reasons += held.reasons.filter { !reasons.contains($0) } }
        if !isLast, !limitSpent, !reasons.isEmpty {
            step = .recalibrationPrompt(afterClip: index, reasons: reasons)
        } else {
            step = Self.next(after: index, clipCount: clipCount)
        }
    }

    /// The operator accepted the prompt: re-calibrate (with validation) before the next clip.
    public mutating func acceptRecalibration() {
        guard case let .recalibrationPrompt(index, reasons) = step else { return }
        step = .recalibrating(afterClip: index, reason: reasons.first ?? .operatorRequest)
    }

    /// The operator declined: the next clip plays on the calibration in effect.
    public mutating func declineRecalibration() {
        guard case let .recalibrationPrompt(index, _) = step else { return }
        step = Self.next(after: index, clipCount: clipCount)
    }

    /// The re-calibration (and its validation) ended: the next clip plays.
    public mutating func recalibrationFinished() {
        guard case let .recalibrating(index, reason) = step else { return }
        recalibrations.append(reason)
        step = Self.next(after: index, clipCount: clipCount)
    }

    /// The operator stopped the battery.
    public mutating func abort() {
        step = .finished
    }

    private static func next(after index: Int, clipCount: Int) -> Step {
        index + 1 < clipCount ? .clip(index + 1) : .finished
    }
}
