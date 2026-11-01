import AsdCore
import Foundation
import CoreGraphics
import simd

#if canImport(ARKit) && !os(macOS)
import ARKit
#endif

/// Lightweight immutable snapshot of face anchor data safe for cross-actor transfer.
public struct FaceAnchorSnapshot: Sendable, Equatable {
    public let isTracked: Bool
    public let transform: simd_float4x4
    public let lookAtPoint: simd_float3
    public let leftEyeTransform: simd_float4x4
    public let rightEyeTransform: simd_float4x4
    public let blendShapes: [String: Double]
    /// `ARLightEstimate.ambientIntensity` (lumens) of the frame, when ARKit gave one (CNF-2).
    public let ambientIntensity: Double?
    /// `ARLightEstimate.ambientColorTemperature` (kelvin), when ARKit gave one.
    public let ambientColorTemperature: Double?
    /// `ARDirectionalLightEstimate.primaryLightDirection`, when ARKit gave one.
    public let lightDirection: simd_float3?

    public init(
        isTracked: Bool,
        transform: simd_float4x4,
        lookAtPoint: simd_float3,
        leftEyeTransform: simd_float4x4 = matrix_identity_float4x4,
        rightEyeTransform: simd_float4x4 = matrix_identity_float4x4,
        blendShapes: [String: Double] = [:],
        ambientIntensity: Double? = nil,
        ambientColorTemperature: Double? = nil,
        lightDirection: simd_float3? = nil
    ) {
        self.isTracked = isTracked
        self.transform = transform
        self.lookAtPoint = lookAtPoint
        self.leftEyeTransform = leftEyeTransform
        self.rightEyeTransform = rightEyeTransform
        self.blendShapes = blendShapes
        self.ambientIntensity = ambientIntensity
        self.ambientColorTemperature = ambientColorTemperature
        self.lightDirection = lightDirection
    }

    #if canImport(ARKit) && !os(macOS)
    public init(faceAnchor: ARFaceAnchor, lightEstimate: ARLightEstimate? = nil) {
        var shapes: [String: Double] = [:]
        for (key, value) in faceAnchor.blendShapes {
            shapes[key.rawValue] = value.doubleValue
        }
        self.init(
            isTracked: faceAnchor.isTracked,
            transform: faceAnchor.transform,
            lookAtPoint: faceAnchor.lookAtPoint,
            leftEyeTransform: faceAnchor.leftEyeTransform,
            rightEyeTransform: faceAnchor.rightEyeTransform,
            blendShapes: shapes,
            ambientIntensity: lightEstimate.map { Double($0.ambientIntensity) },
            ambientColorTemperature: lightEstimate.map { Double($0.ambientColorTemperature) },
            lightDirection: (lightEstimate as? ARDirectionalLightEstimate)?.primaryLightDirection
        )
    }
    #endif
}

/// Consolidated, immutable UI state snapshot dispatched to `@MainActor` at 60 Hz.
/// Contains only the lightweight fields required by SwiftUI views to update
/// reticles and badges without causing heavy view tree invalidations.
public struct GazeUIState: Sendable, Equatable {
    public let currentGazePoint: CGPoint
    public let currentRawGazePoint: CGPoint
    public let distanceCm: Double
    public let currentQuality: GazeQualityGate.Evaluation
    public let currentAffect: FacialAffectMetrics
    public let activeAOIs: [String]
    public let trackingStage: TrackingStage
    public let headYawDeg: Double
    public let headPitchDeg: Double
    public let headRollDeg: Double
    /// Raw ARKit blend-shape coefficients of this frame (empty for simulated frames).
    /// Numbers only; forwarded to the throttled blend-shape broadcast on the main actor.
    public let blendShapes: [String: Double]

    public init(
        currentGazePoint: CGPoint,
        currentRawGazePoint: CGPoint,
        distanceCm: Double,
        currentQuality: GazeQualityGate.Evaluation,
        currentAffect: FacialAffectMetrics,
        activeAOIs: [String],
        trackingStage: TrackingStage,
        headYawDeg: Double,
        headPitchDeg: Double,
        headRollDeg: Double,
        blendShapes: [String: Double] = [:]
    ) {
        self.currentGazePoint = currentGazePoint
        self.currentRawGazePoint = currentRawGazePoint
        self.distanceCm = distanceCm
        self.currentQuality = currentQuality
        self.currentAffect = currentAffect
        self.activeAOIs = activeAOIs
        self.trackingStage = trackingStage
        self.headYawDeg = headYawDeg
        self.headPitchDeg = headPitchDeg
        self.headRollDeg = headRollDeg
        self.blendShapes = blendShapes
    }

    /// UI state while live tracking cannot run (CAM-2): no gaze, explicit reason.
    public static func unavailable(_ reason: TrackingUnavailableReason) -> GazeUIState {
        let stage = TrackingStage.unavailable(reason: reason)
        return GazeUIState(
            currentGazePoint: CGPoint(x: 0.5, y: 0.5),
            currentRawGazePoint: CGPoint(x: 0.5, y: 0.5),
            distanceCm: 48.0,
            currentQuality: GazeQualityGate.Evaluation(
                isValid: false,
                verdict: .rejected,
                failureReasons: [stage.label]
            ),
            currentAffect: .neutral,
            activeAOIs: [],
            trackingStage: stage,
            headYawDeg: 0.0,
            headPitchDeg: 0.0,
            headRollDeg: 0.0
        )
    }

    public static let initial = GazeUIState(
        currentGazePoint: CGPoint(x: 0.5, y: 0.5),
        currentRawGazePoint: CGPoint(x: 0.5, y: 0.5),
        distanceCm: 48.0,
        currentQuality: GazeQualityGate.Evaluation(
            isValid: false,
            verdict: .rejected,
            failureReasons: ["Waiting for camera…"]
        ),
        currentAffect: .neutral,
        activeAOIs: [],
        trackingStage: .searchingFace,
        headYawDeg: 0.0,
        headPitchDeg: 0.0,
        headRollDeg: 0.0
    )
}

/// Swift 6 actor that owns the per-session Rust gaze pipeline and the frame buffer.
///
/// Per frame (60 Hz) it turns the ARKit snapshot into a `RawSampleRecord`, pushes it through
/// `GazePipelineHandle` (raycast with the device geometry table, calibration apply, track
/// state, confidence, 1-Euro filter, AOI hit test: `asd-core::gaze`, PIPE-1) and writes the
/// processed frame as a frames-file sample with `GazeTelemetryWriterHandle` (CNF-4, CORE-7).
/// Swift adds only the facial affect blend shapes (out of the gaze module) and the UI state.
///
/// It is the single owner of every piece of per-frame state (CONC-1): the pipeline handle and
/// its writer (one per session, CAL-1), AOI battery, calibration copy, recording flag, session
/// and clip clocks, interruption bookkeeping and the frame buffer. It is also the only place
/// a session summary is aggregated (trap 11). Live frames reach it in arrival order through
/// `ARSessionBridge`.
///
/// Clips (QS-3): a session opens a clip at start (the battery and stimulus it starts with);
/// `switchStimulusClip` ends it and opens the next; `endStimulusClip` ends it without a next
/// (call it before a drift check); `finalizeSession` ends the last one. The pipeline records
/// (summary frames) exactly while a clip is open (`set_recording`, G.3), and the frame buffer
/// holds the same frames.
///
/// Time: every timestamp handed to the actor is in seconds on one monotonic clock
/// (ARKit's `ARFrame.timestamp`, i.e. system uptime). `startSession` fixes the
/// session origin; the pipeline runs on session time (`t_sec`), so sample times, clip times,
/// dwell and interruptions are derived from timestamps, never from an assumed frame rate
/// (PLC-5).
public actor GazeProcessingActor {
    public private(set) var qualityGate: GazeQualityGate
    public private(set) var aoiTracker: StimulusAOITracker
    public private(set) var calibrationProfile: GazeCalibrationProfile
    /// Hardware model for the geometry table (GEO-2).
    public let deviceModelIdentifier: String
    /// Interface orientation the pipeline maps the screen in (GEO-3).
    public private(set) var orientation: GazeOrientation

    /// The session's pipeline (live sessions; also a fresh one before any start so frames are
    /// never dropped silently). `nil` only when the config could not be built.
    private var pipeline: GazePipelineHandle?
    private var telemetryWriter: GazeTelemetryWriterHandle?
    /// Processed frames parallel to `volatileFrameBuffer` (live frames only), for gap filling
    /// at finalize (GAP-1).
    private var processedBuffer: [ProcessedSampleRecord] = []
    /// Index in `volatileFrameBuffer` of each entry of `processedBuffer`.
    private var processedFrameIndex: [Int] = []

    /// Last drawn points, held while the eyes are closed or the face is lost (drawing only).
    private var lastDrawnPoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    private var lastDrawnRawPoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    private var lastDistanceCm: Double = 48.0

    /// Volatile in-memory telemetry buffer. Kept strictly in RAM during active recording.
    private var volatileFrameBuffer: [GazeFrame] = []
    public private(set) var isRecording: Bool = false
    /// A stimulus clip is open: frames are buffered and count in the pipeline summary.
    public private(set) var isClipOpen: Bool = false
    /// Provenance of the current session; `nil` before the first start and after a finalize.
    public private(set) var captureSource: CaptureSource?
    /// Monotonic timestamp (seconds) that maps to session time 0.
    private var originTimestamp: TimeInterval = 0
    /// Session time of the last buffered frame, for the measured `aoiDwellMs` interval.
    private var lastBufferedSampleSec: Double?
    /// Stimulus clip currently playing (playlist video name). Nil when no playlist context.
    public private(set) var currentStimulusId: String?
    /// AVPlayer media clock seconds within the active clip (wraps on loop). Nil until reported.
    public private(set) var clipMediaTimeSec: Double?

    /// Camera-session interruptions during the current recording (CAM-2).
    public private(set) var interruptionCount: Int = 0
    /// Closed interruption time so far (seconds).
    private var interruptedSecAccumulated: Double = 0
    /// Monotonic timestamp of the open interruption, if any.
    private var openInterruptionTimestamp: TimeInterval?
    /// Set when the camera session failed; nothing is appended until the next start.
    private var hasFailed: Bool = false
    private var unavailableReason: TrackingUnavailableReason? {
        if hasFailed { return .sessionFailed }
        if openInterruptionTimestamp != nil { return .interrupted }
        return nil
    }

    public init(
        qualityGate: GazeQualityGate = GazeQualityGate(),
        aoiTracker: StimulusAOITracker = StimulusAOITracker.makeGeoPrefBattery(),
        calibrationProfile: GazeCalibrationProfile = .identity,
        deviceModelIdentifier: String = GazeDeviceInfo.modelIdentifier,
        orientation: GazeOrientation = .portrait
    ) {
        self.qualityGate = qualityGate
        self.aoiTracker = aoiTracker
        self.calibrationProfile = calibrationProfile
        self.deviceModelIdentifier = deviceModelIdentifier
        self.orientation = orientation
        let (pipeline, writer) = Self.makePipeline(
            modelIdentifier: deviceModelIdentifier,
            orientation: orientation,
            gate: qualityGate,
            profile: calibrationProfile
        )
        self.pipeline = pipeline
        self.telemetryWriter = writer
    }

    /// A fresh pipeline and frames-file writer (session clock origin 0: the actor pushes
    /// session time).
    private static func makePipeline(
        modelIdentifier: String,
        orientation: GazeOrientation,
        gate: GazeQualityGate,
        profile: GazeCalibrationProfile
    ) -> (GazePipelineHandle?, GazeTelemetryWriterHandle?) {
        do {
            let config = try GazePipelineDocuments.config(
                modelIdentifier: modelIdentifier,
                orientation: orientation,
                gate: gate
            )
            let pipeline = try GazePipelineHandle(configJson: config)
            if let model = try GazePipelineDocuments.calibrationModel(profile) {
                try pipeline.setCalibrationJson(modelJson: model)
            }
            let gateJSON = String(decoding: try JSONEncoder().encode(CoreQualityGate(
                minDistanceCm: gate.minDistanceCm,
                maxDistanceCm: gate.maxDistanceCm,
                maxYawRad: gate.maxYawRad,
                maxPitchRad: gate.maxPitchRad,
                minConfidence: gate.minConfidence,
                blinkThreshold: gate.blinkThreshold
            )), as: UTF8.self)
            let writer = try GazeTelemetryWriterHandle(originSec: 0, qualityGateJson: gateJSON)
            return (pipeline, writer)
        } catch {
            AsdCoreBridge.report("gaze_pipeline_new", error)
            return (nil, nil)
        }
    }

    /// Prepares actor for a fresh recording session. Live (`.trueDepth`) unless the
    /// caller is the Debug-only simulated path.
    ///
    /// A new pipeline is created for every session (CAL-1: no calibration state carries over
    /// except the profile bound to this child and session), and the first clip opens.
    ///
    /// - Parameters:
    ///   - originTimestamp: monotonic timestamp of session time 0 (the bridge passes system
    ///     uptime at start; tests using `elapsedSec` keep 0).
    ///   - orientation: interface orientation at start (GEO-3); `nil` keeps the current one.
    ///   - beginsClip: open the first clip now (default). Pass `false` when the session starts
    ///     with positioning or calibration and the first clip opens later with
    ///     `switchStimulusClip`.
    public func startSession(
        battery: StimulusAOITracker? = nil,
        profile: GazeCalibrationProfile? = nil,
        captureSource: CaptureSource = .trueDepth,
        stimulusId: String? = nil,
        originTimestamp: TimeInterval = 0,
        orientation: GazeOrientation? = nil,
        beginsClip: Bool = true
    ) {
        self.captureSource = captureSource
        if let battery = battery {
            self.aoiTracker = battery
        }
        if let profile = profile {
            self.calibrationProfile = profile
        }
        if let orientation {
            self.orientation = orientation
        }
        self.volatileFrameBuffer.removeAll(keepingCapacity: true)
        self.processedBuffer.removeAll(keepingCapacity: true)
        self.processedFrameIndex.removeAll(keepingCapacity: true)
        self.originTimestamp = originTimestamp
        self.lastBufferedSampleSec = nil
        self.currentStimulusId = stimulusId
        self.clipMediaTimeSec = nil
        self.interruptionCount = 0
        self.interruptedSecAccumulated = 0
        self.openInterruptionTimestamp = nil
        self.hasFailed = false
        self.isRecording = true
        self.lastDrawnPoint = CGPoint(x: 0.5, y: 0.5)
        self.lastDrawnRawPoint = CGPoint(x: 0.5, y: 0.5)
        self.lastDistanceCm = 48.0
        if captureSource == .trueDepth {
            (pipeline, telemetryWriter) = Self.makePipeline(
                modelIdentifier: deviceModelIdentifier,
                orientation: self.orientation,
                gate: qualityGate,
                profile: calibrationProfile
            )
        } else {
            // Synthetic frames never enter the gaze pipeline (SIM-3): no trackingQuality.
            (pipeline, telemetryWriter) = (nil, nil)
        }
        isClipOpen = false
        if beginsClip {
            beginClip()
        }
    }

    #if ASD_SIMULATION_ENABLED
    /// Prepares a simulated session (Debug builds only, SIM-3).
    public func startSimulatedSession(
        battery: StimulusAOITracker? = nil,
        profile: GazeCalibrationProfile? = nil,
        stimulusId: String? = nil
    ) {
        startSession(battery: battery, profile: profile, captureSource: .simulated, stimulusId: stimulusId)
    }
    #endif

    /// Opens a clip with the current battery and stimulus: the pipeline collects its frames
    /// and records them (QS-3, G.3 `set_recording`).
    private func beginClip() {
        isClipOpen = true
        guard let pipeline else { return }
        do {
            let aois = try GazePipelineDocuments.clipAOIs(battery: aoiTracker, stimulusId: currentStimulusId)
            try pipeline.beginClipJson(
                clipId: currentStimulusId ?? PerVideoMetricsExtractor.untaggedClipId,
                aoisJson: aois
            )
        } catch {
            AsdCoreBridge.report("gaze_begin_clip", error)
        }
        pipeline.setRecording(on: true)
    }

    /// Ends the open clip, if any: its frames stop counting and are no longer buffered.
    /// Call it when a clip stops playing and before the drift check that follows (QS-3).
    public func endStimulusClip() {
        guard isClipOpen else { return }
        isClipOpen = false
        clipMediaTimeSec = nil
        pipeline?.endClip()
        pipeline?.setRecording(on: false)
    }

    /// Dynamically switches the active stimulus clip and AOI battery mid-session: ends the
    /// open clip and opens the next one.
    ///
    /// - Parameter timestamp: monotonic time of the switch (kept for callers). The pipeline
    ///   starts the clip clock at the next frame, and AOI windows follow the AVPlayer media
    ///   clock once `updateClipMediaTime` reports it.
    public func switchStimulusClip(
        battery: StimulusAOITracker,
        stimulusId: String? = nil,
        at timestamp: TimeInterval? = nil
    ) {
        endStimulusClip()
        self.aoiTracker = battery
        self.currentStimulusId = stimulusId
        self.clipMediaTimeSec = nil
        if isRecording {
            beginClip()
        }
    }

    /// Receives the authoritative AVPlayer media clock for the active clip (10 Hz periodic observer).
    public func updateClipMediaTime(_ timeSec: Double) {
        self.clipMediaTimeSec = timeSec
    }

    /// The interface orientation changed (GEO-3). The pipeline re-maps the screen and resets
    /// its filter; its advice reports `orientationChanged` against the calibration.
    public func setInterfaceOrientation(_ orientation: GazeOrientation) {
        self.orientation = orientation
        pipeline?.setOrientation(orientation: orientation)
    }

    /// Updates the calibration in effect (the kit's affine profile as a `screenAffine` model).
    public func updateCalibrationProfile(_ profile: GazeCalibrationProfile) {
        self.calibrationProfile = profile
        guard let pipeline else { return }
        do {
            try pipeline.setCalibrationJson(modelJson: GazePipelineDocuments.calibrationModel(profile))
        } catch {
            AsdCoreBridge.report("gaze_set_calibration", error)
        }
    }

    /// Resets calibration profile to identity.
    public func resetCalibration() {
        updateCalibrationProfile(.identity)
    }

    // MARK: - Targets (calibration, validation, drift checks; PIPE-1)

    /// Errors of the target API when no live pipeline runs (simulated or not started).
    public enum TargetError: Error, Equatable {
        case noPipeline
    }

    private func requirePipeline() throws -> GazePipelineHandle {
        guard let pipeline else { throw TargetError.noPipeline }
        return pipeline
    }

    /// Starts a (re-)calibration in the pipeline (`layout` defaults to the configured 5 points).
    public func startCalibration(layout: [GazeVec2]? = nil, reason: GazeRecalReason? = nil) throws {
        try requirePipeline().startCalibration(layout: layout, reason: reason)
    }

    /// What the calibration UI should show next; `nil` when no calibration is running.
    public func calibrationStatus() -> GazeCalibrationStatus? {
        pipeline?.calibrationStatus()
    }

    /// Opens a target window at the next frame.
    public func beginTarget(kind: GazeTargetKind, at target: GazeVec2) throws {
        try requirePipeline().beginTarget(kind: kind, target: target)
    }

    /// Whether the open window has collected its frame time.
    public func targetComplete() -> Bool {
        pipeline?.targetComplete() ?? false
    }

    /// Closes the open window.
    public func endTarget() -> GazeTargetResultRecord? {
        pipeline?.endTarget()
    }

    /// Fits and scores the running calibration; applies it unless it failed.
    public func finishCalibration() throws -> GazeCalibrationOutcomeRecord {
        try requirePipeline().finishCalibration()
    }

    /// Re-calibration advice (CAL-9, VAL-4, DRF-3); act on it only at a clip boundary.
    public func recalibrationAdvice() -> GazeRecalAdviceRecord? {
        pipeline?.advice()
    }

    /// Where to show the next drift check target.
    public func nextCheckTarget() -> GazeVec2? {
        pipeline?.nextCheckTarget()
    }

    /// Validation of the calibration in effect (`ValidationRecord`, VAL-2/VAL-3); `nil` before
    /// any validation target or without a pipeline.
    public func validationSummary() -> GazeValidationSummary? {
        guard let pipeline else { return nil }
        do {
            guard let json = try pipeline.validationJson() else { return nil }
            return try JSONDecoder().decode(GazeValidationSummary.self, from: Data(json.utf8))
        } catch {
            AsdCoreBridge.report("gaze_validation", error)
            return nil
        }
    }

    /// Target layouts and window timing from this session's pipeline config; `nil` without a
    /// pipeline (simulated session).
    public func targetSchedule() -> GazeTargetSchedule? {
        guard let pipeline else { return nil }
        do {
            return try GazeTargetSchedule(configJSON: pipeline.configJson())
        } catch {
            AsdCoreBridge.report("gaze_target_schedule", error)
            return nil
        }
    }

    // MARK: - Camera session lifecycle (CAM-2)

    /// The camera session was interrupted (phone call, another app took the camera).
    /// Frames are not appended until `interruptionEnded`. Counted only while recording.
    public func interruptionBegan(at timestamp: TimeInterval) -> GazeUIState {
        if isRecording, openInterruptionTimestamp == nil, !hasFailed {
            interruptionCount += 1
            openInterruptionTimestamp = timestamp
        }
        return .unavailable(unavailableReason ?? .interrupted)
    }

    /// The interruption ended; frames are appended again.
    public func interruptionEnded(at timestamp: TimeInterval) -> GazeUIState? {
        guard !hasFailed else { return .unavailable(.sessionFailed) }
        if let began = openInterruptionTimestamp {
            interruptedSecAccumulated += max(0, timestamp - began)
            openInterruptionTimestamp = nil
        }
        return nil
    }

    /// The camera session failed. Nothing more is appended to this recording; the
    /// time from the failure to the stop counts as interrupted.
    public func sessionFailed(at timestamp: TimeInterval) -> GazeUIState {
        if isRecording, !hasFailed {
            if openInterruptionTimestamp == nil {
                interruptionCount += 1
                openInterruptionTimestamp = timestamp
            }
            hasFailed = true
        }
        return .unavailable(.sessionFailed)
    }

    // MARK: - Frames

    /// Ingests a face anchor snapshot captured at a monotonic `timestamp` (seconds,
    /// same clock as `originTimestamp`). This is the path the AR bridge uses.
    public func processFaceAnchor(snapshot: FaceAnchorSnapshot, timestamp: TimeInterval) -> GazeUIState {
        processFaceAnchor(snapshot: snapshot, elapsedSec: timestamp - originTimestamp)
    }

    /// Ingests one ARKit frame: pushes it through the Rust pipeline, writes the frames-file
    /// sample, buffers the frame while a clip is recording, and returns the UI state.
    ///
    /// Every frame is pushed, with or without a face (TRK-1): an untracked frame is buffered
    /// as `lost`, invalid, confidence 0, with the last known point and pose (never zeros)
    /// and the measured interval, so `validRatio` reflects track loss.
    ///
    /// - Parameter elapsedSec: session time of the sample (seconds since start).
    public func processFaceAnchor(snapshot: FaceAnchorSnapshot, elapsedSec: Double) -> GazeUIState {
        if let reason = unavailableReason {
            return .unavailable(reason)
        }
        guard let pipeline, let telemetryWriter else {
            return Self.searchingState(reason: "Gaze pipeline unavailable")
        }
        let raw = RawSampleRecord(
            snapshot: snapshot,
            tSec: elapsedSec,
            orientation: orientation,
            clipMediaTimeSec: clipMediaTimeSec,
            stimulusId: currentStimulusId
        )
        let processed: ProcessedSampleRecord
        do {
            processed = try pipeline.push(raw: raw)
        } catch {
            // Only a malformed matrix is refused; ARKit matrices are always 4x4.
            AsdCoreBridge.report("gaze_push", error)
            return Self.searchingState(reason: "Tracking data unreadable")
        }
        let written = telemetryWriter.write(sample: processed)
        // TRK-1 / WP-G.0: a frame without a face is `.rejected`. The CORE-7 gate alone would call
        // it `.degraded` (its only failure is confidence 0, with the writer's held pose), so
        // the lost state decides here; every other verdict is the gate's.
        let verdict: GazeQualityGate.Verdict = processed.trackState == .lost
            ? .rejected
            : GazeQualityGate.Verdict(rawValue: written.qualityVerdict) ?? .rejected
        let trackState = FrameTrackState(processed.trackState)
        let sample = GazeTelemetrySample(
            timestampMs: written.timestampMs,
            videoTimeSec: written.videoTimeSec,
            xNorm: written.xNorm,
            yNorm: written.yNorm,
            confidence: written.confidence,
            leftEyeOpenness: written.leftEyeOpenness,
            rightEyeOpenness: written.rightEyeOpenness,
            headPitchRad: written.headPitchRad,
            headYawRad: written.headYawRad,
            headRollRad: written.headRollRad,
            distanceToScreenCm: written.distanceToScreenCm,
            isValid: written.isValid,
            stimulusId: written.stimulusId,
            clipMediaTimeSec: written.clipMediaTimeSec,
            trackState: trackState
        )
        let eval = GazeQualityGate.Evaluation(
            isValid: written.isValid && verdict != .rejected,
            verdict: verdict,
            failureReasons: written.failureReasons,
            isBlink: written.isBlink
        )

        let tracked = snapshot.isTracked
        let affect = tracked ? Self.affect(from: snapshot.blendShapes) : .neutral

        if isRecording && isClipOpen {
            processedBuffer.append(processed)
            processedFrameIndex.append(volatileFrameBuffer.count)
            volatileFrameBuffer.append(GazeFrame(
                gaze: sample,
                affect: affect,
                activeAOINames: processed.activeAoiNames,
                aoiDwellMs: measuredIntervalMs(at: elapsedSec),
                stimulusEventTag: nil,
                qualityVerdict: verdict,
                stimulusId: currentStimulusId,
                clipMediaTimeSec: clipMediaTimeSec
            ))
        }

        guard tracked, processed.trackState != .lost else {
            return Self.searchingState(reason: "Face anchor lost or untracked")
        }

        // Drawing only (TRK-2): the stored point is never clamped.
        if processed.trackState == .tracked {
            if let p = processed.filteredPointNorm { lastDrawnPoint = Self.clampedForDrawing(p) }
            if let r = processed.rawPointNorm { lastDrawnRawPoint = Self.clampedForDrawing(r) }
        }
        if let distance = processed.distanceCm { lastDistanceCm = distance }

        let stage: TrackingStage
        if processed.trackState == .blink {
            stage = .faceAligned(distanceCm: lastDistanceCm)
        } else if eval.isValid {
            stage = .gazeActive
        } else {
            stage = .eyesTracking(
                leftOpenness: processed.leftEyeOpenness ?? sample.leftEyeOpenness,
                rightOpenness: processed.rightEyeOpenness ?? sample.rightEyeOpenness
            )
        }

        return GazeUIState(
            currentGazePoint: lastDrawnPoint,
            currentRawGazePoint: lastDrawnRawPoint,
            distanceCm: lastDistanceCm,
            currentQuality: eval,
            currentAffect: affect,
            activeAOIs: processed.activeAoiNames,
            trackingStage: stage,
            headYawDeg: sample.headYawRad * 180.0 / .pi,
            headPitchDeg: sample.headPitchRad * 180.0 / .pi,
            headRollDeg: sample.headRollRad * 180.0 / .pi,
            blendShapes: snapshot.blendShapes
        )
    }

    /// Facial affect from the ARKit blend shapes (affect stays native, PIPE-2).
    private static func affect(from shapes: [String: Double]) -> FacialAffectMetrics {
        FacialAffectMetrics(
            mouthSmileLeft: shapes["mouthSmileLeft"] ?? 0.0,
            mouthSmileRight: shapes["mouthSmileRight"] ?? 0.0,
            cheekSquintLeft: shapes["cheekSquintLeft"] ?? 0.0,
            cheekSquintRight: shapes["cheekSquintRight"] ?? 0.0,
            browInnerUp: shapes["browInnerUp"] ?? 0.0,
            browOuterUpLeft: shapes["browOuterUpLeft"] ?? 0.0,
            browOuterUpRight: shapes["browOuterUpRight"] ?? 0.0,
            eyeWideLeft: shapes["eyeWideLeft"] ?? 0.0,
            eyeWideRight: shapes["eyeWideRight"] ?? 0.0,
            jawOpen: shapes["jawOpen"] ?? 0.0,
            browDownLeft: shapes["browDownLeft"] ?? 0.0,
            browDownRight: shapes["browDownRight"] ?? 0.0,
            mouthFrownLeft: shapes["mouthFrownLeft"] ?? 0.0,
            mouthFrownRight: shapes["mouthFrownRight"] ?? 0.0
        )
    }

    private static func clampedForDrawing(_ p: GazeVec2) -> CGPoint {
        CGPoint(x: min(max(p.x, 0), 1), y: min(max(p.y, 0), 1))
    }

    /// UI state while no face is tracked.
    private static func searchingState(reason: String) -> GazeUIState {
        GazeUIState(
            currentGazePoint: CGPoint(x: 0.5, y: 0.5),
            currentRawGazePoint: CGPoint(x: 0.5, y: 0.5),
            distanceCm: 48.0,
            currentQuality: GazeQualityGate.Evaluation(
                isValid: false,
                verdict: .rejected,
                failureReasons: [reason]
            ),
            currentAffect: .neutral,
            activeAOIs: [],
            trackingStage: .searchingFace,
            headYawDeg: 0.0,
            headPitchDeg: 0.0,
            headRollDeg: 0.0
        )
    }

    /// Interval since the previously buffered frame (PLC-5: `aoiDwellMs` holds the
    /// measured interval, uncapped; the 100 ms cap applies when dwell is summed).
    /// 0 for the first frame of a session.
    private func measuredIntervalMs(at sampleSec: Double) -> Double {
        defer { lastBufferedSampleSec = sampleSec }
        guard let previous = lastBufferedSampleSec else { return 0 }
        return max(0, sampleSec - previous) * 1000.0
    }

    /// Ingests a simulated frame (Debug simulation and mathematical testing).
    ///
    /// Returns `nil` and buffers nothing unless the session was started with
    /// `startSimulatedSession` (SIM-3); synthetic frames never enter a live session, and they
    /// never enter the gaze pipeline (there is no face to raycast), so a simulated summary
    /// carries no `trackingQuality`. The synthetic point is used as is (no smoothing).
    /// Compiled to a stub without `ASD_SIMULATION_ENABLED`.
    ///
    /// - Parameters:
    ///   - timeSec: session time of the synthetic sample.
    ///   - clipTimeSec: AOI time override (playlist clip clock), if the caller has one.
    public func processSimulatedFrame(
        timeSec: Double,
        target: CGPoint?,
        clipTimeSec: Double? = nil,
        stimulusId: String? = nil,
        clipMediaTimeSec: Double? = nil
    ) -> GazeUIState? {
        #if ASD_SIMULATION_ENABLED
        guard captureSource == .simulated else { return nil }
        let gazePoint: CGPoint
        if let target = target {
            let jitterX = 0.015 * sin(timeSec * 7.2)
            let jitterY = 0.015 * cos(timeSec * 6.4)
            gazePoint = CGPoint(
                x: max(0.05, min(0.95, target.x + jitterX)),
                y: max(0.05, min(0.95, target.y + jitterY))
            )
        } else {
            let wave = sin(timeSec * 1.8)
            let x = wave > -0.2 ? (0.28 + 0.12 * sin(timeSec * 3.5)) : (0.72 + 0.10 * cos(timeSec * 2.2))
            let y = 0.40 + 0.10 * sin(timeSec * 2.0)
            gazePoint = CGPoint(x: max(0.05, min(0.95, x)), y: max(0.05, min(0.95, y)))
        }

        let aoiTimeSec = clipTimeSec ?? clipMediaTimeSec ?? self.clipMediaTimeSec ?? timeSec
        let activeHits = aoiTracker.activeAOIs(
            atTimeSec: aoiTimeSec,
            xNorm: gazePoint.x,
            yNorm: gazePoint.y
        )

        let phase = fmod(timeSec, 16.0)
        let isSmiling = (phase < 4.0)
        let isSurprised = (phase >= 4.0 && phase < 8.0)
        let isExcited = (phase >= 8.0 && phase < 12.0)

        let smileVal = isExcited ? 0.78 : (isSmiling ? 0.65 : 0.06)
        let browUp = isSurprised ? 0.65 : (isExcited ? 0.40 : 0.04)
        let eyeWide = isSurprised ? 0.60 : (isExcited ? 0.45 : 0.05)
        let jawOpenVal = isSurprised ? 0.45 : 0.0

        let affect = FacialAffectMetrics(
            mouthSmileLeft: smileVal,
            mouthSmileRight: smileVal * 0.96,
            cheekSquintLeft: (isSmiling || isExcited) ? 0.48 : 0.02,
            cheekSquintRight: (isSmiling || isExcited) ? 0.46 : 0.02,
            browInnerUp: browUp,
            browOuterUpLeft: browUp * 0.9,
            browOuterUpRight: browUp * 0.9,
            eyeWideLeft: eyeWide,
            eyeWideRight: eyeWide,
            jawOpen: jawOpenVal
        )

        let sample = GazeTelemetrySample(
            timestampMs: timeSec * 1000.0,
            videoTimeSec: timeSec,
            xNorm: gazePoint.x,
            yNorm: gazePoint.y,
            confidence: 0.96,
            leftEyeOpenness: 0.95,
            rightEyeOpenness: 0.95,
            headPitchRad: 0.02 * sin(timeSec),
            headYawRad: 0.03 * cos(timeSec * 0.7),
            headRollRad: 0.01 * sin(timeSec * 0.5),
            distanceToScreenCm: 48.0 + 1.5 * sin(timeSec * 0.3),
            isValid: true,
            stimulusId: stimulusId ?? currentStimulusId,
            clipMediaTimeSec: clipMediaTimeSec ?? self.clipMediaTimeSec
        )

        let eval = qualityGate.evaluateSample(sample)
        if isRecording && isClipOpen {
            let frame = GazeFrame(
                gaze: sample,
                affect: affect,
                activeAOINames: activeHits,
                aoiDwellMs: measuredIntervalMs(at: timeSec),
                stimulusEventTag: isSmiling ? "character_laugh_onset" : nil,
                qualityVerdict: eval.verdict,
                stimulusId: stimulusId ?? currentStimulusId,
                clipMediaTimeSec: clipMediaTimeSec ?? self.clipMediaTimeSec
            )
            volatileFrameBuffer.append(frame)
        }

        return GazeUIState(
            currentGazePoint: gazePoint,
            currentRawGazePoint: gazePoint,
            distanceCm: sample.distanceToScreenCm,
            currentQuality: eval,
            currentAffect: affect,
            activeAOIs: activeHits,
            trackingStage: .gazeActive,
            headYawDeg: sample.headYawRad * 180.0 / .pi,
            headPitchDeg: sample.headPitchRad * 180.0 / .pi,
            headRollDeg: sample.headRollRad * 180.0 / .pi
        )
        #else
        return nil
        #endif
    }

    // MARK: - Finalize (the only aggregation point)

    /// Concludes the session, stops recording, computes clinical aggregates,
    /// and returns the session summary alongside the raw volatile frames.
    ///
    /// For a live session the last clip ends, the buffered frames are gap filled (GAP-1:
    /// filled frames are marked `gap_filled`, keep `valid = false` and so add no dwell,
    /// GAP-2), and the summary carries the pipeline's `trackingQuality` (QS-1) and per-clip
    /// `ClipTracking` (QS-3), read after the last frame as the pipeline requires.
    ///
    /// The buffer is handed over and cleared, so a second finalize cannot replay a
    /// previous session. Without a started session nothing was captured: the empty
    /// `.rejected` summary is labelled `.simulated` (quarantined, never claimed as
    /// TrueDepth), the same rule the app applies to records of unknown provenance.
    ///
    /// - Parameters:
    ///   - timestamp: monotonic stop time; closes an open interruption. When `nil`
    ///     an open interruption contributes no further time.
    ///   - subjectAgeSignal: assistive age-band signal to attach (CNS-8: `nil` unless enabled).
    public func finalizeSession(
        at timestamp: TimeInterval? = nil,
        subjectAgeSignal: SubjectAgeSignal? = nil
    ) -> (summary: GazeSessionReportSummary, frames: [GazeFrame]) {
        if let began = openInterruptionTimestamp, let timestamp {
            interruptedSecAccumulated += max(0, timestamp - began)
        }
        openInterruptionTimestamp = nil
        let started = isRecording
        endStimulusClip()

        var trackingQuality: JSONValue?
        var clipTracking: [ClipTrackingEntry]?
        if started, captureSource == .trueDepth, let pipeline {
            applyGapFill(pipeline)
            do {
                trackingQuality = try JSONValue(json: pipeline.summaryJson())
                clipTracking = try JSONDecoder().decode(
                    [ClipTrackingEntry].self,
                    from: Data(pipeline.clipTrackingJson().utf8)
                )
            } catch {
                AsdCoreBridge.report("gaze_summary", error)
            }
        }

        let framesSnapshot = volatileFrameBuffer
        let summary = GazeSessionReportSummary.aggregate(
            frames: framesSnapshot,
            sessionId: GazeSessionReportSummary.makeSessionId(),
            captureSource: captureSource ?? .simulated,
            subjectAgeSignal: subjectAgeSignal,
            interruptionCount: interruptionCount,
            interruptedSec: interruptedSecAccumulated,
            trackingQuality: trackingQuality,
            clipTracking: clipTracking
        )

        isRecording = false
        captureSource = nil
        volatileFrameBuffer.removeAll(keepingCapacity: false)
        processedBuffer.removeAll(keepingCapacity: false)
        processedFrameIndex.removeAll(keepingCapacity: false)
        lastBufferedSampleSec = nil
        interruptionCount = 0
        interruptedSecAccumulated = 0
        hasFailed = false
        return (summary, framesSnapshot)
    }

    /// GAP-1 over the buffered live frames: a filled frame gets the interpolated point and
    /// `gap_filled = true`; it stays invalid (GAP-2), so dwell and `validRatio` are untouched.
    private func applyGapFill(_ pipeline: GazePipelineHandle) {
        guard !processedBuffer.isEmpty else { return }
        let filled = pipeline.gapFill(samples: processedBuffer)
        guard filled.count == processedBuffer.count else { return }
        for (k, sample) in filled.enumerated() where sample.gapFilled {
            let index = processedFrameIndex[k]
            let frame = volatileFrameBuffer[index]
            let g = frame.gaze
            let point = sample.filteredPointNorm
            volatileFrameBuffer[index] = GazeFrame(
                gaze: GazeTelemetrySample(
                    timestampMs: g.timestampMs,
                    videoTimeSec: g.videoTimeSec,
                    xNorm: point?.x ?? g.xNorm,
                    yNorm: point?.y ?? g.yNorm,
                    confidence: g.confidence,
                    leftEyeOpenness: g.leftEyeOpenness,
                    rightEyeOpenness: g.rightEyeOpenness,
                    headPitchRad: g.headPitchRad,
                    headYawRad: g.headYawRad,
                    headRollRad: g.headRollRad,
                    distanceToScreenCm: g.distanceToScreenCm,
                    isValid: false,
                    stimulusId: g.stimulusId,
                    clipMediaTimeSec: g.clipMediaTimeSec,
                    trackState: g.trackState,
                    gapFilled: true
                ),
                affect: frame.affect,
                activeAOINames: frame.activeAOINames,
                aoiDwellMs: frame.aoiDwellMs,
                stimulusEventTag: frame.stimulusEventTag,
                qualityVerdict: frame.qualityVerdict,
                stimulusId: frame.stimulusId,
                clipMediaTimeSec: frame.clipMediaTimeSec
            )
        }
    }

    /// Accessor for current count of buffered frames in volatile memory.
    public func getRecordedFramesCount() -> Int {
        return volatileFrameBuffer.count
    }

    /// Accessor for frames in volatile memory.
    public func getVolatileFrames() -> [GazeFrame] {
        return volatileFrameBuffer
    }
}

// MARK: - Aggregation

extension GazeSessionReportSummary {
    /// Longest interval a single frame may contribute to dwell (PLC-5). Gaps longer
    /// than this (dropped frames, lost face, interruption) are not counted as looking.
    static let maxFrameDwellSec: Double = 0.1

    /// Aggregates buffered frames into a summary. Called only by
    /// `GazeProcessingActor.finalizeSession` (trap 11).
    ///
    /// PLC-5: `durationSec` is the last sample time minus the first. Each frame
    /// contributes `min(interval to the next frame, 100 ms)` of dwell; the last
    /// frame has no next frame and contributes nothing, so total dwell never exceeds
    /// the duration. CAM-2: an interrupted session is at best `.degraded`.
    static func aggregate(
        frames: [GazeFrame],
        sessionId: String,
        captureSource: CaptureSource,
        subjectAgeSignal: SubjectAgeSignal?,
        interruptionCount: Int = 0,
        interruptedSec: Double = 0,
        trackingQuality: JSONValue? = nil,
        clipTracking: [ClipTrackingEntry]? = nil
    ) -> GazeSessionReportSummary {
        let totalFrames = frames.count
        guard totalFrames > 0 else {
            return GazeSessionReportSummary(
                sessionId: sessionId,
                captureSource: captureSource,
                durationSec: 0,
                totalFrames: 0,
                validFrames: 0,
                validRatio: 0,
                socialPreferenceScore: 0,
                socialDwellSec: 0,
                geometricDwellSec: 0,
                eyeDwellRatio: nil,
                mouthDwellRatio: nil,
                firstEyeLatencyMs: nil,
                socialSmileIntensityMean: 0,
                smileReactivityIndex: 0,
                flatAffectScore: 0,
                surpriseScoreMean: 0,
                qualityVerdict: .rejected,
                subjectAgeSignal: subjectAgeSignal,
                interruptionCount: interruptionCount,
                interruptedSec: interruptedSec,
                trackingQuality: trackingQuality,
                clipTracking: clipTracking
            )
        }

        let sampleSec = frames.map { $0.gaze.timestampMs / 1000.0 }
        let durationSec = max(0, sampleSec[totalFrames - 1] - sampleSec[0])

        var validFrames: [GazeFrame] = []
        validFrames.reserveCapacity(totalFrames)
        var socialDwell = 0.0
        var geometricDwell = 0.0
        for index in 0..<totalFrames {
            let frame = frames[index]
            guard frame.gaze.isValid && frame.qualityVerdict != .rejected else { continue }
            validFrames.append(frame)
            let dwell = index + 1 < totalFrames
                ? min(max(0, sampleSec[index + 1] - sampleSec[index]), maxFrameDwellSec)
                : 0
            if AOIRole.intersects(frame.activeAOINames, kind: .social) {
                socialDwell += dwell
            }
            if AOIRole.intersects(frame.activeAOINames, kind: .geometric) {
                geometricDwell += dwell
            }
        }
        let validRatio = Double(validFrames.count) / Double(totalFrames)

        // Ratios, latency and affect come from the shared reducer; dwell seconds and
        // preference are recomputed above from timestamps (its dwell assumes 60 fps).
        let biomarkers = SessionBiomarkerAggregator.aggregate(
            validFrames: validFrames,
            durationSec: durationSec
        )
        let totalPreferenceDwell = socialDwell + geometricDwell
        let preference = totalPreferenceDwell > 0
            ? (socialDwell - geometricDwell) / totalPreferenceDwell
            : 0

        var verdict: GazeQualityGate.Verdict = validRatio >= 0.75 ? .optimal : (validRatio >= 0.50 ? .acceptable : .degraded)
        if interruptionCount > 0 || interruptedSec > 0, verdict == .optimal || verdict == .acceptable {
            verdict = .degraded
        }

        return GazeSessionReportSummary(
            sessionId: sessionId,
            captureSource: captureSource,
            durationSec: durationSec,
            totalFrames: totalFrames,
            validFrames: validFrames.count,
            validRatio: validRatio,
            socialPreferenceScore: preference,
            socialDwellSec: socialDwell,
            geometricDwellSec: geometricDwell,
            // PLC-1/PLC-2: measured from smoothed fixations and catalog AOI roles; nil when unobserved.
            eyeDwellRatio: biomarkers.eyeDwellRatio,
            mouthDwellRatio: biomarkers.mouthDwellRatio,
            firstEyeLatencyMs: biomarkers.firstOrientLatencyMs,
            socialSmileIntensityMean: biomarkers.socialSmileIntensityMean,
            smileReactivityIndex: biomarkers.smileReactivityIndex,
            flatAffectScore: biomarkers.flatAffectScore,
            surpriseScoreMean: biomarkers.surpriseScoreMean,
            qualityVerdict: verdict,
            subjectAgeSignal: subjectAgeSignal,
            interruptionCount: interruptionCount,
            interruptedSec: interruptedSec,
            trackingQuality: trackingQuality,
            clipTracking: clipTracking
        )
    }

    /// Short uppercase session identifier used when none is supplied by the caller.
    static func makeSessionId() -> String {
        String(UUID().uuidString.prefix(8)).uppercased()
    }
}
