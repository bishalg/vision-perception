import Foundation
import CoreGraphics
import Combine
#if canImport(ARKit) && !os(macOS)
import ARKit
#endif

/// Summary report produced at the conclusion of a gaze observation session.
public struct GazeSessionReportSummary: Codable, Sendable, Equatable {
    public let sessionId: String
    public let durationSec: Double
    public let totalFrames: Int
    public let validFrames: Int
    public let validRatio: Double
    public let socialPreferenceScore: Double
    public let socialDwellSec: Double
    public let geometricDwellSec: Double
    public let eyeDwellRatio: Double
    public let mouthDwellRatio: Double
    public let firstEyeLatencyMs: Double
    public let socialSmileIntensityMean: Double
    public let smileReactivityIndex: Double
    public let flatAffectScore: Double
    public let surpriseScoreMean: Double
    public let qualityVerdict: GazeQualityGate.Verdict

    public init(
        sessionId: String = UUID().uuidString.prefix(8).uppercased(),
        durationSec: Double,
        totalFrames: Int,
        validFrames: Int,
        validRatio: Double,
        socialPreferenceScore: Double,
        socialDwellSec: Double,
        geometricDwellSec: Double,
        eyeDwellRatio: Double = 0.28,
        mouthDwellRatio: Double = 0.14,
        firstEyeLatencyMs: Double = 480.0,
        socialSmileIntensityMean: Double = 0.45,
        smileReactivityIndex: Double = 0.38,
        flatAffectScore: Double = 0.12,
        surpriseScoreMean: Double = 0.24,
        qualityVerdict: GazeQualityGate.Verdict = .optimal
    ) {
        self.sessionId = sessionId
        self.durationSec = durationSec
        self.totalFrames = totalFrames
        self.validFrames = validFrames
        self.validRatio = validRatio
        self.socialPreferenceScore = socialPreferenceScore
        self.socialDwellSec = socialDwellSec
        self.geometricDwellSec = geometricDwellSec
        self.eyeDwellRatio = eyeDwellRatio
        self.mouthDwellRatio = mouthDwellRatio
        self.firstEyeLatencyMs = firstEyeLatencyMs
        self.socialSmileIntensityMean = socialSmileIntensityMean
        self.smileReactivityIndex = smileReactivityIndex
        self.flatAffectScore = flatAffectScore
        self.surpriseScoreMean = surpriseScoreMean
        self.qualityVerdict = qualityVerdict
    }
}

/// Coordinates live 60 Hz gaze, facial affect, and head pose telemetry acquisition.
/// Supports both TrueDepth hardware (`ARSession`) and deterministic mathematical simulation.
///
/// Concurrency architecture (Swift 6):
/// - Math, raycasting, 1-Euro filtering, and volatile telemetry buffering run strictly off
///   the Main Thread (`arDelegateQueue` / `GazeProcessingActor`).
/// - Telemetry buffer is held purely in volatile RAM during recording (no disk writes at 60 Hz).
/// - Only lightweight `GazeUIState` snapshots are dispatched to `@MainActor` to update HUD reticles.
public final class GazeSessionController: NSObject, ObservableObject, @unchecked Sendable {
    @Published public var currentGazePoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @Published public var currentRawGazePoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @Published public private(set) var calibrationProfile: GazeCalibrationProfile
    @Published public var currentDistanceCm: Double = 48.0
    @Published public var currentQuality: GazeQualityGate.Evaluation
    @Published public var currentAffect: FacialAffectMetrics = .neutral
    @Published public var activeAOIs: [String] = []
    @Published public var isRecording: Bool = false
    @Published public var headYawDeg: Double = 0.0
    @Published public var headPitchDeg: Double = 0.0
    @Published public var headRollDeg: Double = 0.0

    /// Post-session finalized frames. Kept off `@MainActor` during active 60 Hz streaming,
    /// and populated once the session is completed to avoid 60 Hz SwiftUI view tree invalidations.
    @Published public var recordedFrames: [GazeFrame] = []

    @Published public private(set) var trackingStage: TrackingStage = .searchingFace

    public let raycaster: ScreenRaycaster
    public let qualityGate: GazeQualityGate
    public var aoiTracker: StimulusAOITracker

    /// Swift 6 background actor for direct asynchronous processing pipelines.
    public let processingActor: GazeProcessingActor

    private var sessionStartTime: Date?
    public private(set) var clipStartTime: Date?
    public var isSimulationMode: Bool = false
    @Published public var calibrationTarget: CGPoint? = nil

    /// Lock-guarded clip attribution state — written on the main thread by playback UI,
    /// read from `arDelegateQueue` at 60 Hz.
    private let clipStateLock = NSLock()
    private var _currentStimulusId: String?
    private var _clipMediaTimeSec: Double?

    /// Stimulus clip currently playing (playlist video name). Nil when no playlist context.
    public var currentStimulusId: String? {
        clipStateLock.lock()
        defer { clipStateLock.unlock() }
        return _currentStimulusId
    }
    /// AVPlayer media clock seconds within the active clip (wraps on loop). Nil until reported.
    public var clipMediaTime: Double? {
        clipStateLock.lock()
        defer { clipStateLock.unlock() }
        return _clipMediaTimeSec
    }

    /// High-frequency volatile RAM buffer for telemetry frames during recording.
    private let frameBufferLock = NSLock()
    private var volatileFrameBuffer: [GazeFrame] = []

    /// Adaptive 1 Euro Filter for calibrated gaze.
    private let gazeFilter = OneEuroFilter2D(minCutoff: 1.2, beta: 0.015, dCutoff: 1.0)
    /// Separate filter for uncalibrated raw signal.
    private let rawGazeFilter = OneEuroFilter2D(minCutoff: 1.2, beta: 0.015, dCutoff: 1.0)

    /// Last verified valid fixation points to freeze reticle during eyelid occlusion / blinks.
    private var lastValidCalibratedPoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    private var lastValidRawPoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    /// Post-blink debounce counter (~50ms / 3 frames) to allow pupil tracking settling after eye reopening.
    private var blinkRecoveryFramesRemaining: Int = 0
    private let blinkRecoveryFramesCount: Int = 3

    public static let didUpdateCalibrationNotification = Notification.Name("CurvAsdGazeKit.calibrationDidUpdate")
    public static let didUpdateFaceBlendShapesNotification = Notification.Name("CurvAsdGazeKit.faceBlendShapesDidUpdate")
    public static let didStartRecordingNotification = Notification.Name("CurvAsdGazeKit.recordingDidStart")
    public static let didStopRecordingNotification = Notification.Name("CurvAsdGazeKit.recordingDidStop")
    public static let blendShapesUserInfoKey = "blendShapes"
    public static let affectUserInfoKey = "affect"
    public static let isRecordingUserInfoKey = "isRecording"
    public static let sessionIdUserInfoKey = "sessionId"

    private var lastBlendShapeBroadcast: TimeInterval = 0
    private var recordingSessionToken: String = UUID().uuidString
    private var calibrationObserver: NSObjectProtocol?

    #if canImport(ARKit) && !os(macOS)
    @Published public var arSession: ARSession?
    private let arDelegateQueue = DispatchQueue(label: "org.curv.asd.arSessionQueue", qos: .userInteractive)
    #endif

    public init(
        raycaster: ScreenRaycaster = ScreenRaycaster(),
        qualityGate: GazeQualityGate = GazeQualityGate(),
        aoiTracker: StimulusAOITracker = StimulusAOITracker.makeGeoPrefBattery(),
        calibrationProfile: GazeCalibrationProfile = GazeCalibrationStore.load()
    ) {
        self.raycaster = raycaster
        self.qualityGate = qualityGate
        self.aoiTracker = aoiTracker
        self.calibrationProfile = calibrationProfile
        self.processingActor = GazeProcessingActor(
            raycaster: raycaster,
            qualityGate: qualityGate,
            aoiTracker: aoiTracker,
            calibrationProfile: calibrationProfile
        )
        self.currentQuality = GazeQualityGate.Evaluation(isValid: false, verdict: .rejected, failureReasons: ["Not started"])
        super.init()

        calibrationObserver = NotificationCenter.default.addObserver(
            forName: Self.didUpdateCalibrationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.reloadCalibrationProfile()
        }
    }

    deinit {
        if let observer = calibrationObserver {
            NotificationCenter.default.removeObserver(observer)
        }
        #if canImport(ARKit) && !os(macOS)
        arSession?.pause()
        arSession?.delegate = nil
        arSession?.delegateQueue = nil
        arSession = nil
        #endif
    }

    /// Persists freshly fitted calibration profile.
    public func applyAndPersist(calibrationProfile profile: GazeCalibrationProfile) {
        calibrationProfile = profile
        GazeCalibrationStore.save(profile)
        gazeFilter.reset()
        rawGazeFilter.reset()
        Task { [weak processingActor] in
            await processingActor?.updateCalibrationProfile(profile)
        }
        NotificationCenter.default.post(name: GazeSessionController.didUpdateCalibrationNotification, object: nil)
    }

    /// Resets calibration profile to identity transform.
    public func resetCalibration() {
        calibrationProfile = .identity
        GazeCalibrationStore.reset()
        gazeFilter.reset()
        rawGazeFilter.reset()
        Task { [weak processingActor] in
            await processingActor?.resetCalibration()
        }
        NotificationCenter.default.post(name: GazeSessionController.didUpdateCalibrationNotification, object: nil)
    }

    /// Reloads persisted profile from store.
    public func reloadCalibrationProfile() {
        calibrationProfile = GazeCalibrationStore.load()
        let profile = calibrationProfile
        Task { [weak processingActor] in
            await processingActor?.updateCalibrationProfile(profile)
        }
    }

    /// Starts telemetry capture. Volatile RAM buffer is reset.
    public func startSession(battery: StimulusAOITracker? = nil, stimulusId: String? = nil) {
        if let battery = battery {
            self.aoiTracker = battery
        }
        frameBufferLock.lock()
        volatileFrameBuffer.removeAll(keepingCapacity: true)
        frameBufferLock.unlock()

        self.recordedFrames.removeAll()
        self.sessionStartTime = Date()
        self.clipStartTime = Date()
        clipStateLock.lock()
        _currentStimulusId = stimulusId
        _clipMediaTimeSec = nil
        clipStateLock.unlock()
        self.isRecording = true
        self.recordingSessionToken = UUID().uuidString
        NotificationCenter.default.post(name: Self.didStartRecordingNotification, object: self)

        reloadCalibrationProfile()
        gazeFilter.reset()
        rawGazeFilter.reset()

        let profile = calibrationProfile
        let batt = battery
        Task { [weak processingActor] in
            await processingActor?.startSession(battery: batt, profile: profile)
        }

        trackingStage = .searchingFace
        currentQuality = GazeQualityGate.Evaluation(isValid: false, verdict: .rejected, failureReasons: ["Waiting for camera…"])
        lastValidCalibratedPoint = CGPoint(x: 0.5, y: 0.5)
        lastValidRawPoint = CGPoint(x: 0.5, y: 0.5)
        blinkRecoveryFramesRemaining = 0

        #if canImport(ARKit) && !os(macOS)
        if ARFaceTrackingConfiguration.isSupported {
            if let existingSession = arSession {
                existingSession.pause()
                existingSession.delegate = nil
                existingSession.delegateQueue = nil
                self.arSession = nil
            }
            let session = ARSession()
            session.delegateQueue = arDelegateQueue
            session.delegate = self
            let config = ARFaceTrackingConfiguration()
            config.isWorldTrackingEnabled = false
            session.run(config, options: [.resetTracking, .removeExistingAnchors])
            self.arSession = session
            self.isSimulationMode = false
            return
        }
        #endif

        self.isSimulationMode = true
    }

    /// Dynamically switches the active stimulus clip and AOI battery mid-session.
    public func switchStimulusClip(battery: StimulusAOITracker, stimulusId: String? = nil) {
        self.aoiTracker = battery
        self.clipStartTime = Date()
        clipStateLock.lock()
        _currentStimulusId = stimulusId
        _clipMediaTimeSec = nil
        clipStateLock.unlock()
        Task { [weak processingActor] in
            await processingActor?.switchStimulusClip(battery: battery, stimulusId: stimulusId)
        }
    }

    /// Receives the authoritative AVPlayer media clock for the active clip (10 Hz periodic observer).
    /// Time-coded AOI windows and per-clip segmentation align to this clock, not wall-clock.
    public func updateClipMediaTime(_ timeSec: Double) {
        clipStateLock.lock()
        _clipMediaTimeSec = timeSec
        clipStateLock.unlock()
        Task { [weak processingActor] in
            await processingActor?.updateClipMediaTime(timeSec)
        }
    }

    /// Ends the session, computes the comprehensive biomarker report, and exposes recorded frames.
    public func stopSession() -> GazeSessionReportSummary {
        self.isRecording = false
        NotificationCenter.default.post(name: Self.didStopRecordingNotification, object: self)

        #if canImport(ARKit) && !os(macOS)
        arSession?.pause()
        arSession?.delegate = nil
        arSession?.delegateQueue = nil
        arSession = nil
        #endif

        frameBufferLock.lock()
        let framesSnapshot = volatileFrameBuffer
        frameBufferLock.unlock()

        self.recordedFrames = framesSnapshot
        let totalFrames = framesSnapshot.count

        guard totalFrames > 0 else {
            return GazeSessionReportSummary(
                durationSec: 0,
                totalFrames: 0,
                validFrames: 0,
                validRatio: 0,
                socialPreferenceScore: 0,
                socialDwellSec: 0,
                geometricDwellSec: 0,
                qualityVerdict: .rejected
            )
        }

        let validFrames = framesSnapshot.filter { $0.gaze.isValid && $0.qualityVerdict != .rejected }
        let validRatio = Double(validFrames.count) / Double(totalFrames)

        let durationSec = Double(totalFrames) * SessionBiomarkerAggregator.frameIntervalSec
        let biomarkers = SessionBiomarkerAggregator.aggregate(
            validFrames: validFrames,
            durationSec: durationSec
        )
        let verdict: GazeQualityGate.Verdict = validRatio >= 0.75 ? .optimal : (validRatio >= 0.50 ? .acceptable : .degraded)

        return GazeSessionReportSummary(
            durationSec: durationSec,
            totalFrames: totalFrames,
            validFrames: validFrames.count,
            validRatio: validRatio,
            socialPreferenceScore: biomarkers.socialPreferenceScore,
            socialDwellSec: biomarkers.socialDwellSec,
            geometricDwellSec: biomarkers.geometricDwellSec,
            eyeDwellRatio: biomarkers.eyeDwellRatio,
            mouthDwellRatio: biomarkers.mouthDwellRatio,
            firstEyeLatencyMs: biomarkers.firstOrientLatencyMs,
            socialSmileIntensityMean: biomarkers.socialSmileIntensityMean,
            smileReactivityIndex: biomarkers.smileReactivityIndex,
            flatAffectScore: biomarkers.flatAffectScore,
            surpriseScoreMean: biomarkers.surpriseScoreMean,
            qualityVerdict: verdict
        )
    }

    /// Asynchronously stops session and persists the volatile RAM buffer as an encrypted batch to disk.
    public func stopAndPersistBatchAsync(
        participantToken: String,
        stimulusBattery: String = "GeoPref_v1",
        vault: SessionVaultManager = SessionVaultManager(),
        classification: BiomarkerClassificationResult? = nil
    ) async throws -> (summary: GazeSessionReportSummary, vaultURL: URL) {
        let summary = stopSession()
        let frames = self.recordedFrames
        let header = SessionHeader(
            sessionId: summary.sessionId,
            participantToken: participantToken,
            stimulusBattery: stimulusBattery,
            stimulusDurationSec: summary.durationSec
        )
        let url = try await vault.saveSessionBatchAsync(
            header: header,
            frames: frames,
            report: summary,
            classification: classification
        )
        return (summary, url)
    }

    /// Ingests a single synthetic frame (used for preview animation, unit testing, and Simulator).
    public func tickSimulatedFrame(
        timeSec: Double,
        clipTimeSec: Double? = nil,
        stimulusId: String? = nil,
        clipMediaTimeSec: Double? = nil
    ) {
        let gazePoint: CGPoint
        if let target = calibrationTarget {
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

        let smoothedGaze = gazeFilter.filter(point: gazePoint, timestamp: timeSec)
        let smoothedRaw = rawGazeFilter.filter(point: gazePoint, timestamp: timeSec)
        let activeHits = aoiTracker.activeAOIs(
            atTimeSec: clipTimeSec ?? timeSec,
            xNorm: smoothedGaze.x,
            yNorm: smoothedGaze.y
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
            xNorm: smoothedGaze.x,
            yNorm: smoothedGaze.y,
            confidence: 0.96,
            leftEyeOpenness: 0.95,
            rightEyeOpenness: 0.95,
            headPitchRad: 0.02 * sin(timeSec),
            headYawRad: 0.03 * cos(timeSec * 0.7),
            headRollRad: 0.01 * sin(timeSec * 0.5),
            distanceToScreenCm: 48.0 + 1.5 * sin(timeSec * 0.3),
            isValid: true,
            stimulusId: stimulusId ?? currentStimulusId,
            clipMediaTimeSec: clipMediaTimeSec ?? clipMediaTime
        )

        let eval = qualityGate.evaluateSample(sample)
        let frame = GazeFrame(
            gaze: sample,
            affect: affect,
            activeAOINames: activeHits,
            aoiDwellMs: 16.67,
            stimulusEventTag: isSmiling ? "character_laugh_onset" : nil,
            qualityVerdict: eval.verdict,
            stimulusId: stimulusId ?? currentStimulusId,
            clipMediaTimeSec: clipMediaTimeSec ?? clipMediaTime
        )

        if self.isRecording {
            frameBufferLock.lock()
            self.volatileFrameBuffer.append(frame)
            frameBufferLock.unlock()
        }

        let state = GazeUIState(
            currentGazePoint: smoothedGaze,
            currentRawGazePoint: smoothedRaw,
            distanceCm: sample.distanceToScreenCm,
            currentQuality: eval,
            currentAffect: affect,
            activeAOIs: activeHits,
            trackingStage: .gazeActive,
            headYawDeg: sample.headYawRad * 180.0 / .pi,
            headPitchDeg: sample.headPitchRad * 180.0 / .pi,
            headRollDeg: sample.headRollRad * 180.0 / .pi
        )

        if Thread.isMainThread {
            self.applyUIState(state)
            self.broadcastBlendShapes(affect: affect, shapes: [:])
        } else {
            DispatchQueue.main.async { [weak self] in
                self?.applyUIState(state)
                self?.broadcastBlendShapes(affect: affect, shapes: [:])
            }
        }
    }

    /// Consolidates UI property assignments on `@MainActor`.
    private func applyUIState(_ state: GazeUIState) {
        self.currentGazePoint = state.currentGazePoint
        self.currentRawGazePoint = state.currentRawGazePoint
        self.currentDistanceCm = state.distanceCm
        self.currentQuality = state.currentQuality
        self.currentAffect = state.currentAffect
        self.activeAOIs = state.activeAOIs
        self.trackingStage = state.trackingStage
        self.headYawDeg = state.headYawDeg
        self.headPitchDeg = state.headPitchDeg
        self.headRollDeg = state.headRollDeg
    }

    /// Throttled numeric blend-shape fan-out for mood fusion and Core ML CSV (no frames).
    func broadcastBlendShapes(affect: FacialAffectMetrics, shapes: [String: Double]) {
        let now = Date().timeIntervalSince1970
        guard now - lastBlendShapeBroadcast >= 0.2 else { return }
        lastBlendShapeBroadcast = now
        var info: [String: Any] = [
            Self.affectUserInfoKey: affect,
            Self.blendShapesUserInfoKey: shapes,
            Self.isRecordingUserInfoKey: isRecording,
            Self.sessionIdUserInfoKey: recordingSessionToken
        ]
        if shapes.isEmpty {
            info[Self.blendShapesUserInfoKey] = blendShapeMap(from: affect)
        }
        NotificationCenter.default.post(
            name: Self.didUpdateFaceBlendShapesNotification,
            object: self,
            userInfo: info
        )
    }

    private func blendShapeMap(from affect: FacialAffectMetrics) -> [String: Double] {
        [
            "mouthSmileLeft": affect.mouthSmileLeft,
            "mouthSmileRight": affect.mouthSmileRight,
            "cheekSquintLeft": affect.cheekSquintLeft,
            "cheekSquintRight": affect.cheekSquintRight,
            "browInnerUp": affect.browInnerUp,
            "browOuterUpLeft": affect.browOuterUpLeft,
            "browOuterUpRight": affect.browOuterUpRight,
            "eyeWideLeft": affect.eyeWideLeft,
            "eyeWideRight": affect.eyeWideRight,
            "jawOpen": affect.jawOpen,
            "browDownLeft": affect.browDownLeft,
            "browDownRight": affect.browDownRight,
            "mouthFrownLeft": affect.mouthFrownLeft,
            "mouthFrownRight": affect.mouthFrownRight
        ]
    }
}

#if canImport(ARKit) && !os(macOS)
extension GazeSessionController: ARSessionDelegate {
    public func session(_ session: ARSession, didUpdate anchors: [ARAnchor]) {
        guard let faceAnchor = anchors.compactMap({ $0 as? ARFaceAnchor }).first, faceAnchor.isTracked else {
            DispatchQueue.main.async { [weak self] in
                self?.trackingStage = .searchingFace
                self?.currentQuality = GazeQualityGate.Evaluation(
                    isValid: false,
                    verdict: .rejected,
                    failureReasons: ["Face anchor lost or untracked"]
                )
            }
            return
        }

        // Heavy geometric math executes on background queue `arDelegateQueue`
        let distanceMeters = max(0.2, -faceAnchor.transform.columns.3.z)
        let distanceCm = Double(distanceMeters * 100.0)

        let raycast = raycaster.projectGazeToScreen(
            faceTransform: faceAnchor.transform,
            lookAtPoint: faceAnchor.lookAtPoint,
            leftEyeTransform: faceAnchor.leftEyeTransform,
            rightEyeTransform: faceAnchor.rightEyeTransform
        )
        let calibratedPoint = calibrationProfile.apply(to: raycast.normalizedPoint)

        let pitch = Double(asin(max(-1.0, min(1.0, -faceAnchor.transform.columns.2.y))))
        let yaw = Double(atan2(faceAnchor.transform.columns.2.x, faceAnchor.transform.columns.2.z))
        let roll = Double(atan2(faceAnchor.transform.columns.0.y, faceAnchor.transform.columns.1.y))

        let shapes = faceAnchor.blendShapes
        let leftBlink = shapes[.eyeBlinkLeft]?.doubleValue ?? 0
        let rightBlink = shapes[.eyeBlinkRight]?.doubleValue ?? 0
        let leftEyeOpenness = max(0.0, min(1.0, 1.0 - leftBlink))
        let rightEyeOpenness = max(0.0, min(1.0, 1.0 - rightBlink))

        // Blink detection & post-blink recovery debounce window
        let isBlink = leftEyeOpenness < qualityGate.blinkThreshold || rightEyeOpenness < qualityGate.blinkThreshold
        let isRecoveringFromBlink = blinkRecoveryFramesRemaining > 0
        if isBlink {
            blinkRecoveryFramesRemaining = blinkRecoveryFramesCount
        } else if isRecoveringFromBlink {
            blinkRecoveryFramesRemaining -= 1
        }
        let isOcularArtifact = isBlink || isRecoveringFromBlink

        let affect = FacialAffectMetrics(
            mouthSmileLeft: shapes[.mouthSmileLeft]?.doubleValue ?? 0,
            mouthSmileRight: shapes[.mouthSmileRight]?.doubleValue ?? 0,
            cheekSquintLeft: shapes[.cheekSquintLeft]?.doubleValue ?? 0,
            cheekSquintRight: shapes[.cheekSquintRight]?.doubleValue ?? 0,
            browInnerUp: shapes[.browInnerUp]?.doubleValue ?? 0,
            browOuterUpLeft: shapes[.browOuterUpLeft]?.doubleValue ?? 0,
            browOuterUpRight: shapes[.browOuterUpRight]?.doubleValue ?? 0,
            eyeWideLeft: shapes[.eyeWideLeft]?.doubleValue ?? 0,
            eyeWideRight: shapes[.eyeWideRight]?.doubleValue ?? 0,
            jawOpen: shapes[.jawOpen]?.doubleValue ?? 0,
            browDownLeft: shapes[.browDownLeft]?.doubleValue ?? 0,
            browDownRight: shapes[.browDownRight]?.doubleValue ?? 0,
            mouthFrownLeft: shapes[.mouthFrownLeft]?.doubleValue ?? 0,
            mouthFrownRight: shapes[.mouthFrownRight]?.doubleValue ?? 0
        )

        let elapsedSec = sessionStartTime.map { Date().timeIntervalSince($0) } ?? 0.0
        let clipElapsedSec = clipStartTime.map { Date().timeIntervalSince($0) } ?? elapsedSec
        let activeStimulusId = currentStimulusId
        let activeClipMediaTime = clipMediaTime

        // Smooth before hit testing so the stored fixation matches the reticle.
        // Blink samples stay out of the filter and hold the last steady point.
        let nowSec = Date().timeIntervalSince1970
        let smoothedCalibrated: CGPoint
        let smoothedRaw: CGPoint
        if isOcularArtifact {
            smoothedCalibrated = lastValidCalibratedPoint
            smoothedRaw = lastValidRawPoint
        } else {
            smoothedCalibrated = gazeFilter.filter(point: calibratedPoint, timestamp: nowSec)
            smoothedRaw = rawGazeFilter.filter(point: raycast.normalizedPoint, timestamp: nowSec)
            lastValidCalibratedPoint = smoothedCalibrated
            lastValidRawPoint = smoothedRaw
        }

        let aoiTimeSec = activeClipMediaTime ?? clipElapsedSec
        let activeHits: [String] = isOcularArtifact
            ? []
            : aoiTracker.activeAOIs(
                atTimeSec: aoiTimeSec,
                xNorm: smoothedCalibrated.x,
                yNorm: smoothedCalibrated.y
            )

        let gazeCoord = smoothedCalibrated
        let sampleIsValid = raycast.isWithinScreen && !isOcularArtifact

        let sample = GazeTelemetrySample(
            timestampMs: elapsedSec * 1000.0,
            videoTimeSec: elapsedSec,
            xNorm: gazeCoord.x,
            yNorm: gazeCoord.y,
            confidence: isOcularArtifact ? 0.0 : 0.95,
            leftEyeOpenness: leftEyeOpenness,
            rightEyeOpenness: rightEyeOpenness,
            headPitchRad: pitch,
            headYawRad: yaw,
            headRollRad: roll,
            distanceToScreenCm: distanceCm,
            isValid: sampleIsValid,
            stimulusId: activeStimulusId,
            clipMediaTimeSec: activeClipMediaTime
        )

        let eval = qualityGate.evaluateSample(sample)
        let frame = GazeFrame(
            gaze: sample,
            affect: affect,
            activeAOINames: activeHits,
            aoiDwellMs: 16.67,
            stimulusEventTag: nil,
            qualityVerdict: eval.verdict,
            stimulusId: activeStimulusId,
            clipMediaTimeSec: activeClipMediaTime
        )

        let stage: TrackingStage
        let eyesOpen = leftEyeOpenness >= qualityGate.blinkThreshold && rightEyeOpenness >= qualityGate.blinkThreshold
        if !eyesOpen {
            stage = .faceAligned(distanceCm: distanceCm)
        } else if eval.isValid && sampleIsValid {
            stage = .gazeActive
        } else {
            stage = .eyesTracking(leftOpenness: leftEyeOpenness, rightOpenness: rightEyeOpenness)
        }

        // Buffer frame in volatile RAM without touching main thread
        if self.isRecording {
            frameBufferLock.lock()
            self.volatileFrameBuffer.append(frame)
            frameBufferLock.unlock()
        }

        // Only dispatch lightweight UI state to main thread
        let uiState = GazeUIState(
            currentGazePoint: smoothedCalibrated,
            currentRawGazePoint: smoothedRaw,
            distanceCm: distanceCm,
            currentQuality: eval,
            currentAffect: affect,
            activeAOIs: activeHits,
            trackingStage: stage,
            headYawDeg: yaw * 180.0 / .pi,
            headPitchDeg: pitch * 180.0 / .pi,
            headRollDeg: roll * 180.0 / .pi
        )

        DispatchQueue.main.async { [weak self] in
            self?.applyUIState(uiState)
            var map: [String: Double] = [:]
            for (key, value) in shapes {
                map[key.rawValue] = value.doubleValue
            }
            self?.broadcastBlendShapes(affect: affect, shapes: map)
        }
    }

    public func session(_ session: ARSession, didRemove anchors: [ARAnchor]) {
        if anchors.contains(where: { $0 is ARFaceAnchor }) {
            DispatchQueue.main.async { [weak self] in
                self?.trackingStage = .searchingFace
                self?.currentQuality = GazeQualityGate.Evaluation(
                    isValid: false,
                    verdict: .rejected,
                    failureReasons: ["Face anchor removed"]
                )
            }
        }
    }
}
#endif
