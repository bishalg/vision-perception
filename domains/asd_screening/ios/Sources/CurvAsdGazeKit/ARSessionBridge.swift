import Foundation
import CoreGraphics
import CoreVideo
import os
import simd
#if canImport(ARKit) && !os(macOS)
import ARKit
#endif

/// Ordered, thread-safe ingress into `GazeProcessingActor` (CONC-1, CONC-2, CONC-4).
///
/// Every input to the gaze pipeline (ARKit frames, session start, calibration,
/// AOI battery switches, clip clock, camera interruptions, finalize) is submitted
/// through this object. Submitting is synchronous and safe from any thread: the
/// event is appended to one FIFO `AsyncStream`, and a single consumer task applies
/// the events to the actor one at a time. The order in which callers submit is
/// therefore exactly the order the actor sees, which makes "frames before
/// `stopSession` are kept, frames after are dropped" deterministic.
///
/// On iOS the bridge is the `ARSessionDelegate` (see the extension below); it only
/// builds `FaceAnchorSnapshot`s and hands them on. It holds no mutable state of its
/// own, so it is `Sendable` without `@unchecked`.
public final class ARSessionBridge: NSObject, Sendable {
    /// One pipeline input. Internal: the public API is the submit methods.
    enum Event: Sendable {
        case start(
            battery: StimulusAOITracker?,
            profile: GazeCalibrationProfile,
            captureSource: CaptureSource,
            stimulusId: String?,
            originTimestamp: TimeInterval
        )
        case frame(FaceAnchorSnapshot, timestamp: TimeInterval)
        case simulated(timeSec: Double, target: CGPoint?, clipTimeSec: Double?, stimulusId: String?, clipMediaTimeSec: Double?)
        case calibration(GazeCalibrationProfile)
        case switchClip(battery: StimulusAOITracker, stimulusId: String?, timestamp: TimeInterval?)
        case clipMediaTime(Double)
        case interruptionBegan(TimeInterval)
        case interruptionEnded(TimeInterval)
        case sessionFailed(TimeInterval)
        case finalize(
            timestamp: TimeInterval?,
            subjectAgeSignal: SubjectAgeSignal?,
            reply: AsyncStream<FinalizedSession>.Continuation
        )
        case barrier(AsyncStream<Void>.Continuation)
    }

    /// Result of a finalize request.
    public struct FinalizedSession: Sendable {
        public let summary: GazeSessionReportSummary
        public let frames: [GazeFrame]
    }

    /// Handle returned by `requestFinalize`: the cut point is already fixed when it
    /// is returned; `value` waits for the actor to finish the frames before it.
    public struct PendingFinalize: Sendable {
        fileprivate let stream: AsyncStream<FinalizedSession>

        public var value: (summary: GazeSessionReportSummary, frames: [GazeFrame]) {
            get async {
                for await result in stream {
                    return (result.summary, result.frames)
                }
                // The pipeline was torn down before the request was served: nothing
                // can be claimed as captured.
                let empty = GazeSessionReportSummary.aggregate(
                    frames: [],
                    sessionId: GazeSessionReportSummary.makeSessionId(),
                    captureSource: .simulated,
                    subjectAgeSignal: nil
                )
                return (empty, [])
            }
        }
    }

    public let processingActor: GazeProcessingActor
    /// Assistive age estimator fed from the AR delegate queue; gated by its own
    /// `isEnabled` (CNS-8). `nil` disables the path entirely.
    let ageEstimator: SubjectAgeEstimator?
    private let continuation: AsyncStream<Event>.Continuation
    private let ageSink: @Sendable (SubjectAgeSignal) -> Void

    /// - Parameters:
    ///   - processingActor: the single owner of per-frame state.
    ///   - ageEstimator: optional subject-age estimator (receives frames only when enabled).
    ///   - uiSink: receives the UI state produced by each event, in order, on the
    ///     consumer task (not the main actor). Keep it cheap.
    ///   - ageSink: receives new age signals on the AR delegate queue.
    public init(
        processingActor: GazeProcessingActor,
        ageEstimator: SubjectAgeEstimator? = nil,
        uiSink: @escaping @Sendable (GazeUIState) -> Void = { _ in },
        ageSink: @escaping @Sendable (SubjectAgeSignal) -> Void = { _ in }
    ) {
        self.processingActor = processingActor
        self.ageEstimator = ageEstimator
        self.ageSink = ageSink
        let (stream, continuation) = AsyncStream<Event>.makeStream(bufferingPolicy: .unbounded)
        self.continuation = continuation
        super.init()
        let actor = processingActor
        Task(priority: .userInitiated) {
            for await event in stream {
                await Self.apply(event, to: actor, uiSink: uiSink)
            }
        }
    }

    deinit {
        continuation.finish()
    }

    // MARK: - Submit (synchronous, any thread)

    /// Starts a recording. `originTimestamp` is session time 0 on the frame clock.
    public func start(
        battery: StimulusAOITracker?,
        profile: GazeCalibrationProfile,
        captureSource: CaptureSource,
        stimulusId: String?,
        originTimestamp: TimeInterval
    ) {
        continuation.yield(.start(
            battery: battery,
            profile: profile,
            captureSource: captureSource,
            stimulusId: stimulusId,
            originTimestamp: originTimestamp
        ))
    }

    /// Submits one face snapshot captured at `timestamp` (seconds, frame clock).
    public func submit(_ snapshot: FaceAnchorSnapshot, timestamp: TimeInterval) {
        continuation.yield(.frame(snapshot, timestamp: timestamp))
    }

    #if ASD_SIMULATION_ENABLED
    /// Submits one synthetic frame (Debug builds only, SIM-3).
    func submitSimulated(
        timeSec: Double,
        target: CGPoint?,
        clipTimeSec: Double?,
        stimulusId: String?,
        clipMediaTimeSec: Double?
    ) {
        continuation.yield(.simulated(
            timeSec: timeSec,
            target: target,
            clipTimeSec: clipTimeSec,
            stimulusId: stimulusId,
            clipMediaTimeSec: clipMediaTimeSec
        ))
    }
    #endif

    public func updateCalibration(_ profile: GazeCalibrationProfile) {
        continuation.yield(.calibration(profile))
    }

    public func resetCalibration() {
        continuation.yield(.calibration(.identity))
    }

    /// Switches AOI battery and clip; `at` is the frame-clock time of the switch
    /// (`nil` starts the clip clock at the next frame).
    public func switchStimulusClip(battery: StimulusAOITracker, stimulusId: String?, at timestamp: TimeInterval?) {
        continuation.yield(.switchClip(battery: battery, stimulusId: stimulusId, timestamp: timestamp))
    }

    public func updateClipMediaTime(_ timeSec: Double) {
        continuation.yield(.clipMediaTime(timeSec))
    }

    public func interruptionBegan(at timestamp: TimeInterval) {
        continuation.yield(.interruptionBegan(timestamp))
    }

    public func interruptionEnded(at timestamp: TimeInterval) {
        continuation.yield(.interruptionEnded(timestamp))
    }

    public func sessionFailed(at timestamp: TimeInterval) {
        continuation.yield(.sessionFailed(timestamp))
    }

    /// Fixes the stop point now: everything submitted before this call is part of
    /// the session, everything after is not. Await `value` for the summary.
    public func requestFinalize(
        at timestamp: TimeInterval?,
        subjectAgeSignal: SubjectAgeSignal? = nil
    ) -> PendingFinalize {
        let (stream, reply) = AsyncStream<FinalizedSession>.makeStream(bufferingPolicy: .bufferingNewest(1))
        continuation.yield(.finalize(timestamp: timestamp, subjectAgeSignal: subjectAgeSignal, reply: reply))
        return PendingFinalize(stream: stream)
    }

    /// Waits until every event submitted before this call has been applied and its
    /// UI state handed to `uiSink`.
    public func drain() async {
        let (stream, reply) = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        continuation.yield(.barrier(reply))
        for await _ in stream { return }
    }

    // MARK: - Consumer

    private static func apply(
        _ event: Event,
        to actor: GazeProcessingActor,
        uiSink: @Sendable (GazeUIState) -> Void
    ) async {
        switch event {
        case let .start(battery, profile, captureSource, stimulusId, origin):
            await actor.startSession(
                battery: battery,
                profile: profile,
                captureSource: captureSource,
                stimulusId: stimulusId,
                originTimestamp: origin
            )
        case let .frame(snapshot, timestamp):
            uiSink(await actor.processFaceAnchor(snapshot: snapshot, timestamp: timestamp))
        case let .simulated(timeSec, target, clipTimeSec, stimulusId, clipMediaTimeSec):
            if let state = await actor.processSimulatedFrame(
                timeSec: timeSec,
                target: target,
                clipTimeSec: clipTimeSec,
                stimulusId: stimulusId,
                clipMediaTimeSec: clipMediaTimeSec
            ) {
                uiSink(state)
            }
        case let .calibration(profile):
            await actor.updateCalibrationProfile(profile)
        case let .switchClip(battery, stimulusId, timestamp):
            await actor.switchStimulusClip(battery: battery, stimulusId: stimulusId, at: timestamp)
        case let .clipMediaTime(timeSec):
            await actor.updateClipMediaTime(timeSec)
        case let .interruptionBegan(timestamp):
            uiSink(await actor.interruptionBegan(at: timestamp))
        case let .interruptionEnded(timestamp):
            if let state = await actor.interruptionEnded(at: timestamp) {
                uiSink(state)
            }
        case let .sessionFailed(timestamp):
            uiSink(await actor.sessionFailed(at: timestamp))
        case let .finalize(timestamp, signal, reply):
            let result = await actor.finalizeSession(at: timestamp, subjectAgeSignal: signal)
            reply.yield(FinalizedSession(summary: result.summary, frames: result.frames))
            reply.finish()
        case let .barrier(reply):
            reply.yield(())
            reply.finish()
        }
    }
}

// MARK: - Main-actor hand-off

/// Coalesces pipeline output for the main actor: only the newest UI state is kept,
/// and at most one main-actor drain is scheduled at a time, so the 60 Hz stream
/// never queues stale states or floods the main actor.
final class PipelineMailbox: Sendable {
    private struct State {
        var latestUI: GazeUIState?
        var latestAge: SubjectAgeSignal?
        var scheduled = false
        var drain: (@MainActor @Sendable () -> Void)?
    }

    private let state = OSAllocatedUnfairLock(initialState: State())

    /// Installs the main-actor drain callback (once, after the owner is initialised).
    func setDrain(_ drain: @escaping @MainActor @Sendable () -> Void) {
        state.withLock { $0.drain = drain }
    }

    func post(ui: GazeUIState) {
        schedule { $0.latestUI = ui }
    }

    func post(age: SubjectAgeSignal) {
        schedule { $0.latestAge = age }
    }

    /// Takes everything posted so far and re-arms scheduling.
    func take() -> (ui: GazeUIState?, age: SubjectAgeSignal?) {
        state.withLock { s in
            let out = (s.latestUI, s.latestAge)
            s.latestUI = nil
            s.latestAge = nil
            s.scheduled = false
            return out
        }
    }

    private func schedule(_ update: @Sendable (inout State) -> Void) {
        let drain: (@MainActor @Sendable () -> Void)? = state.withLock { s in
            update(&s)
            guard !s.scheduled, let drain = s.drain else { return nil }
            s.scheduled = true
            return drain
        }
        if let drain {
            Task { @MainActor in drain() }
        }
    }
}

// MARK: - ARKit

extension FaceAnchorSnapshot {
    /// Frame without a tracked face (anchor missing or lost).
    static let untracked = FaceAnchorSnapshot(
        isTracked: false,
        transform: matrix_identity_float4x4,
        lookAtPoint: simd_float3(0, 0, 0)
    )
}

#if canImport(ARKit) && !os(macOS)
extension ARSessionBridge: ARSessionDelegate {
    /// One callback per camera frame. Builds the snapshot synchronously (ARKit
    /// reuses the frame's buffers; nothing is retained) and submits it in order.
    public func session(_ session: ARSession, didUpdate frame: ARFrame) {
        let faceAnchor = frame.anchors.lazy.compactMap { $0 as? ARFaceAnchor }.first

        // CNS-8: the estimator sees no frames unless explicitly enabled.
        if let estimator = ageEstimator, estimator.isEnabled {
            let metrics = Self.ageFrameMetrics(from: frame, faceAnchor: faceAnchor)
            let signal = estimator.record(
                capturedImage: frame.capturedImage,
                faceCenterNorm: metrics?.centerNorm,
                faceRadiusNorm: metrics?.radiusNorm,
                yawRad: metrics?.yawRad ?? 0,
                pitchRad: metrics?.pitchRad ?? 0,
                distanceCm: metrics?.distanceCm ?? 0,
                eyeOpenness: metrics?.eyeOpenness ?? 0,
                timestamp: frame.timestamp
            )
            if let signal {
                ageSink(signal)
            }
        }

        let snapshot = faceAnchor.map { FaceAnchorSnapshot(faceAnchor: $0) } ?? .untracked
        submit(snapshot, timestamp: frame.timestamp)
    }

    /// CAM-2: the session failed (camera unavailable, permission revoked, ...).
    public func session(_ session: ARSession, didFailWithError error: Error) {
        sessionFailed(at: ProcessInfo.processInfo.systemUptime)
    }

    /// CAM-2: phone call, Control Center camera use, multitasking.
    public func sessionWasInterrupted(_ session: ARSession) {
        interruptionBegan(at: ProcessInfo.processInfo.systemUptime)
    }

    public func sessionInterruptionEnded(_ session: ARSession) {
        interruptionEnded(at: ProcessInfo.processInfo.systemUptime)
    }

    /// Projects the face anchor into captured-image normalized coordinates and
    /// derives the coarse pose metrics the age pipeline gates on. Projection
    /// conventions have shifted across OS versions, so the result is sanity-clamped
    /// and the estimator falls back to a centered crop when it looks unreliable.
    static func ageFrameMetrics(
        from frame: ARFrame,
        faceAnchor: ARFaceAnchor?
    ) -> (centerNorm: CGPoint?, radiusNorm: Double?, yawRad: Double, pitchRad: Double, distanceCm: Double, eyeOpenness: Double)? {
        guard let faceAnchor else { return nil }
        let transform = faceAnchor.transform

        let distanceCm = Double(max(0.2, -transform.columns.3.z) * 100.0)
        let yawRad = Double(atan2(transform.columns.2.x, transform.columns.2.z))
        let pitchRad = Double(asin(max(-1.0, min(1.0, -transform.columns.2.y))))

        let shapes = faceAnchor.blendShapes
        let leftOpenness = max(0.0, min(1.0, 1.0 - (shapes[.eyeBlinkLeft]?.doubleValue ?? 0)))
        let rightOpenness = max(0.0, min(1.0, 1.0 - (shapes[.eyeBlinkRight]?.doubleValue ?? 0)))
        let eyeOpenness = min(leftOpenness, rightOpenness)

        let imageWidth = CGFloat(CVPixelBufferGetWidth(frame.capturedImage))
        let imageHeight = CGFloat(CVPixelBufferGetHeight(frame.capturedImage))

        var centerNorm: CGPoint?
        var radiusNorm: Double?
        let faceCenter = simd_float3(
            transform.columns.3.x,
            transform.columns.3.y,
            transform.columns.3.z
        )
        let projected = frame.camera.projectPoint(
            faceCenter,
            orientation: .landscapeRight,
            viewportSize: CGSize(width: imageWidth, height: imageHeight)
        )
        // Project a point ~11 cm to the face's anatomical right to estimate the
        // on-screen face radius (half-width) in pixels.
        let rightVector = simd_float3(
            transform.columns.0.x,
            transform.columns.0.y,
            transform.columns.0.z
        )
        let edgePoint = faceCenter + rightVector * 0.11
        let edgeProjected = frame.camera.projectPoint(
            edgePoint,
            orientation: .landscapeRight,
            viewportSize: CGSize(width: imageWidth, height: imageHeight)
        )
        let dx = Double(edgeProjected.x - projected.x)
        let dy = Double(edgeProjected.y - projected.y)
        let radiusPx = (dx * dx + dy * dy).squareRoot()
        let candidateX = Double(projected.x / imageWidth)
        let candidateY = Double(projected.y / imageHeight)
        if (0.05...0.95).contains(candidateX), (0.05...0.95).contains(candidateY), radiusPx > 8 {
            centerNorm = CGPoint(x: candidateX, y: candidateY)
            radiusNorm = radiusPx / Double(min(imageWidth, imageHeight))
        }

        return (centerNorm, radiusNorm, yawRad, pitchRad, distanceCm, eyeOpenness)
    }
}
#endif
