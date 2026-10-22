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

    public init(
        isTracked: Bool,
        transform: simd_float4x4,
        lookAtPoint: simd_float3,
        leftEyeTransform: simd_float4x4 = matrix_identity_float4x4,
        rightEyeTransform: simd_float4x4 = matrix_identity_float4x4,
        blendShapes: [String: Double] = [:]
    ) {
        self.isTracked = isTracked
        self.transform = transform
        self.lookAtPoint = lookAtPoint
        self.leftEyeTransform = leftEyeTransform
        self.rightEyeTransform = rightEyeTransform
        self.blendShapes = blendShapes
    }

    #if canImport(ARKit) && !os(macOS)
    public init(faceAnchor: ARFaceAnchor) {
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
            blendShapes: shapes
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
        headRollDeg: Double
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

/// Swift 6 Actor isolating high-frequency (60 Hz) mathematical operations:
/// - 3D-to-2D Screen Raycasting (`ScreenRaycaster`)
/// - Adaptive 1-Euro Low-Pass Filtering (`OneEuroFilter2D`)
/// - Dynamic Stimulus Area-of-Interest (AOI) Hit Testing (`StimulusAOITracker`)
/// - 52-FACS Facial Affect and Emotion Synthesis
/// - Volatile RAM buffering of telemetry frames (7,200+ samples)
///
/// Running this work within an actor guarantees that the Main Thread remains free
/// for AVPlayer video playback and SwiftUI rendering, preventing dropped video frames.
public actor GazeProcessingActor {
    public private(set) var raycaster: ScreenRaycaster
    public private(set) var qualityGate: GazeQualityGate
    public private(set) var aoiTracker: StimulusAOITracker
    public private(set) var calibrationProfile: GazeCalibrationProfile

    private let gazeFilter: OneEuroFilter2D
    private let rawGazeFilter: OneEuroFilter2D

    /// Last verified valid fixation points to freeze reticle during eyelid occlusion / blinks.
    private var lastValidCalibratedPoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    private var lastValidRawPoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    /// Post-blink debounce counter (~50ms / 3 frames) to allow pupil tracking settling after eye reopening.
    private var blinkRecoveryFramesRemaining: Int = 0
    private let blinkRecoveryFramesCount: Int = 3

    /// Volatile in-memory telemetry buffer. Kept strictly in RAM during active recording.
    /// Under 2 MB footprint for 7,200 samples (2-minute session at 60 Hz).
    private var volatileFrameBuffer: [GazeFrame] = []
    public private(set) var isRecording: Bool = false
    private var sessionStartTime: Date?
    private var clipStartTime: Date?
    /// Stimulus clip currently playing (playlist video name). Nil when no playlist context.
    public private(set) var currentStimulusId: String?
    /// AVPlayer media clock seconds within the active clip (wraps on loop). Nil until reported.
    public private(set) var clipMediaTimeSec: Double?

    public init(
        raycaster: ScreenRaycaster = ScreenRaycaster(),
        qualityGate: GazeQualityGate = GazeQualityGate(),
        aoiTracker: StimulusAOITracker = StimulusAOITracker.makeGeoPrefBattery(),
        calibrationProfile: GazeCalibrationProfile = .identity
    ) {
        self.raycaster = raycaster
        self.qualityGate = qualityGate
        self.aoiTracker = aoiTracker
        self.calibrationProfile = calibrationProfile
        self.gazeFilter = OneEuroFilter2D(minCutoff: 1.2, beta: 0.015, dCutoff: 1.0)
        self.rawGazeFilter = OneEuroFilter2D(minCutoff: 1.2, beta: 0.015, dCutoff: 1.0)
    }

    /// Prepares actor for a fresh recording session.
    public func startSession(
        battery: StimulusAOITracker? = nil,
        profile: GazeCalibrationProfile? = nil
    ) {
        if let battery = battery {
            self.aoiTracker = battery
        }
        if let profile = profile {
            self.calibrationProfile = profile
        }
        self.volatileFrameBuffer.removeAll(keepingCapacity: true)
        self.sessionStartTime = Date()
        self.clipStartTime = Date()
        self.currentStimulusId = nil
        self.clipMediaTimeSec = nil
        self.isRecording = true
        self.lastValidCalibratedPoint = CGPoint(x: 0.5, y: 0.5)
        self.lastValidRawPoint = CGPoint(x: 0.5, y: 0.5)
        self.blinkRecoveryFramesRemaining = 0
        self.gazeFilter.reset()
        self.rawGazeFilter.reset()
    }

    /// Dynamically switches the active stimulus clip and AOI battery mid-session.
    public func switchStimulusClip(battery: StimulusAOITracker, stimulusId: String? = nil) {
        self.aoiTracker = battery
        self.clipStartTime = Date()
        self.currentStimulusId = stimulusId
        self.clipMediaTimeSec = nil
    }

    /// Receives the authoritative AVPlayer media clock for the active clip (10 Hz periodic observer).
    public func updateClipMediaTime(_ timeSec: Double) {
        self.clipMediaTimeSec = timeSec
    }

    /// Updates calibration profile and resets smoothing filters.
    public func updateCalibrationProfile(_ profile: GazeCalibrationProfile) {
        self.calibrationProfile = profile
        self.lastValidCalibratedPoint = CGPoint(x: 0.5, y: 0.5)
        self.lastValidRawPoint = CGPoint(x: 0.5, y: 0.5)
        self.blinkRecoveryFramesRemaining = 0
        self.gazeFilter.reset()
        self.rawGazeFilter.reset()
    }

    /// Resets calibration profile to identity.
    public func resetCalibration() {
        self.calibrationProfile = .identity
        self.lastValidCalibratedPoint = CGPoint(x: 0.5, y: 0.5)
        self.lastValidRawPoint = CGPoint(x: 0.5, y: 0.5)
        self.blinkRecoveryFramesRemaining = 0
        self.gazeFilter.reset()
        self.rawGazeFilter.reset()
    }

    /// Ingests a face anchor snapshot from ARKit, executes 3D raycasting,
    /// applies 1-Euro smoothing and affine calibration, performs AOI hit testing,
    /// buffers the frame in volatile RAM if recording, and returns a lightweight UI state.
    public func processFaceAnchor(snapshot: FaceAnchorSnapshot, elapsedSec: Double) -> GazeUIState {
        guard snapshot.isTracked else {
            return GazeUIState(
                currentGazePoint: CGPoint(x: 0.5, y: 0.5),
                currentRawGazePoint: CGPoint(x: 0.5, y: 0.5),
                distanceCm: 48.0,
                currentQuality: GazeQualityGate.Evaluation(
                    isValid: false,
                    verdict: .rejected,
                    failureReasons: ["Face anchor lost or untracked"]
                ),
                currentAffect: .neutral,
                activeAOIs: [],
                trackingStage: .searchingFace,
                headYawDeg: 0.0,
                headPitchDeg: 0.0,
                headRollDeg: 0.0
            )
        }

        let distanceMeters = max(0.2, -snapshot.transform.columns.3.z)
        let distanceCm = Double(distanceMeters * 100.0)

        // 1. Raycast 3D gaze onto 2D screen plane
        let raycast = raycaster.projectGazeToScreen(
            faceTransform: snapshot.transform,
            lookAtPoint: snapshot.lookAtPoint,
            leftEyeTransform: snapshot.leftEyeTransform,
            rightEyeTransform: snapshot.rightEyeTransform
        )

        // 2. Correct raw estimate with per-child affine calibration
        let calibratedPoint = calibrationProfile.apply(to: raycast.normalizedPoint)

        // 3. Decompose Euler angles from face transform
        let pitch = Double(asin(max(-1.0, min(1.0, -snapshot.transform.columns.2.y))))
        let yaw = Double(atan2(snapshot.transform.columns.2.x, snapshot.transform.columns.2.z))
        let roll = Double(atan2(snapshot.transform.columns.0.y, snapshot.transform.columns.1.y))

        // 4. Extract facial blend shapes
        let shapes = snapshot.blendShapes
        let leftBlink = shapes["eyeBlinkLeft"] ?? 0.0
        let rightBlink = shapes["eyeBlinkRight"] ?? 0.0
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

        // 5. 1-Euro adaptive low-pass BEFORE hit testing. AOI reaction and the stored
        // sample follow the same point the reticle draws. Blink samples never enter the filter.
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

        // 6. AOI windows align to the AVPlayer media clock when the periodic observer is live.
        let clipElapsedSec = clipStartTime.map { Date().timeIntervalSince($0) } ?? elapsedSec
        let aoiTimeSec = clipMediaTimeSec ?? clipElapsedSec
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
            stimulusId: currentStimulusId,
            clipMediaTimeSec: clipMediaTimeSec
        )
        let eval = qualityGate.evaluateSample(sample)

        // 7. Progressive 3-Stage Tracking pipeline
        let stage: TrackingStage
        let eyesOpen = leftEyeOpenness >= qualityGate.blinkThreshold && rightEyeOpenness >= qualityGate.blinkThreshold
        if !eyesOpen {
            stage = .faceAligned(distanceCm: distanceCm)
        } else if eval.isValid && sampleIsValid {
            stage = .gazeActive
        } else {
            stage = .eyesTracking(leftOpenness: leftEyeOpenness, rightOpenness: rightEyeOpenness)
        }

        // 8. Volatile RAM buffering (no disk writes during 60 Hz stream)
        let frame = GazeFrame(
            gaze: sample,
            affect: affect,
            activeAOINames: activeHits,
            aoiDwellMs: 16.67,
            stimulusEventTag: nil,
            qualityVerdict: eval.verdict,
            stimulusId: currentStimulusId,
            clipMediaTimeSec: clipMediaTimeSec
        )
        if isRecording {
            volatileFrameBuffer.append(frame)
        }

        return GazeUIState(
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
    }

    /// Ingests a simulated frame (for Simulator and mathematical testing).
    public func processSimulatedFrame(
        timeSec: Double,
        target: CGPoint?,
        stimulusId: String? = nil,
        clipMediaTimeSec: Double? = nil
    ) -> GazeUIState {
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

        let aoiTimeSec = clipMediaTimeSec ?? self.clipMediaTimeSec ?? timeSec
        let smoothedGaze = gazeFilter.filter(point: gazePoint, timestamp: timeSec)
        let smoothedRaw = rawGazeFilter.filter(point: gazePoint, timestamp: timeSec)
        let activeHits = aoiTracker.activeAOIs(
            atTimeSec: aoiTimeSec,
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
            clipMediaTimeSec: clipMediaTimeSec ?? self.clipMediaTimeSec
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
            clipMediaTimeSec: clipMediaTimeSec ?? self.clipMediaTimeSec
        )

        if isRecording {
            volatileFrameBuffer.append(frame)
        }

        return GazeUIState(
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
    }

    /// Concludes the session, stops recording, computes clinical aggregates,
    /// and returns the session summary alongside the raw volatile frames.
    public func finalizeSession() -> (summary: GazeSessionReportSummary, frames: [GazeFrame]) {
        self.isRecording = false
        let totalFrames = volatileFrameBuffer.count
        let framesSnapshot = volatileFrameBuffer

        guard totalFrames > 0 else {
            let emptySummary = GazeSessionReportSummary(
                durationSec: 0,
                totalFrames: 0,
                validFrames: 0,
                validRatio: 0,
                socialPreferenceScore: 0,
                socialDwellSec: 0,
                geometricDwellSec: 0,
                qualityVerdict: .rejected
            )
            return (emptySummary, framesSnapshot)
        }

        let validFrames = framesSnapshot.filter { $0.gaze.isValid && $0.qualityVerdict != .rejected }
        let validRatio = Double(validFrames.count) / Double(totalFrames)

        let durationSec = Double(totalFrames) * SessionBiomarkerAggregator.frameIntervalSec
        let biomarkers = SessionBiomarkerAggregator.aggregate(
            validFrames: validFrames,
            durationSec: durationSec
        )
        let verdict: GazeQualityGate.Verdict = validRatio >= 0.75 ? .optimal : (validRatio >= 0.50 ? .acceptable : .degraded)

        let summary = GazeSessionReportSummary(
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

        return (summary, framesSnapshot)
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
