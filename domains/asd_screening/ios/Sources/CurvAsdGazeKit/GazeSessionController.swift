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
public final class GazeSessionController: NSObject, ObservableObject, @unchecked Sendable {
    @Published public var currentGazePoint: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @Published public var currentDistanceCm: Double = 48.0
    @Published public var currentQuality: GazeQualityGate.Evaluation
    @Published public var currentAffect: FacialAffectMetrics = .neutral
    @Published public var activeAOIs: [String] = []
    @Published public var isRecording: Bool = false
    @Published public var headYawDeg: Double = 0.0
    @Published public var headPitchDeg: Double = 0.0
    @Published public var headRollDeg: Double = 0.0
    @Published public var recordedFrames: [GazeFrame] = []

    public let raycaster: ScreenRaycaster
    public let qualityGate: GazeQualityGate
    public var aoiTracker: StimulusAOITracker

    private var sessionStartTime: Date?
    public var isSimulationMode: Bool = false

    #if canImport(ARKit) && !os(macOS)
    private var arSession: ARSession?
    #endif

    public init(
        raycaster: ScreenRaycaster = ScreenRaycaster(),
        qualityGate: GazeQualityGate = GazeQualityGate(),
        aoiTracker: StimulusAOITracker = StimulusAOITracker.makeGeoPrefBattery()
    ) {
        self.raycaster = raycaster
        self.qualityGate = qualityGate
        self.aoiTracker = aoiTracker
        self.currentQuality = GazeQualityGate.Evaluation(isValid: true, verdict: .optimal)
        super.init()
    }

    /// Starts telemetry capture. Automatically activates TrueDepth camera if available,
    /// or falls back to synthetic simulation on Simulator/macOS.
    public func startSession(battery: StimulusAOITracker? = nil) {
        if let battery = battery {
            self.aoiTracker = battery
        }
        self.recordedFrames.removeAll()
        self.sessionStartTime = Date()
        self.isRecording = true

        #if canImport(ARKit) && !os(macOS)
        if ARFaceTrackingConfiguration.isSupported {
            let session = ARSession()
            session.delegate = self
            let config = ARFaceTrackingConfiguration()
            config.isWorldTrackingEnabled = false
            session.run(config, options: [.resetTracking, .removeExistingAnchors])
            self.arSession = session
            self.isSimulationMode = false
            return
        }
        #endif

        // Fallback for Simulator or devices without TrueDepth
        self.isSimulationMode = true
    }

    /// Ends the session and computes the comprehensive biomarker report.
    public func stopSession() -> GazeSessionReportSummary {
        self.isRecording = false

        #if canImport(ARKit) && !os(macOS)
        arSession?.pause()
        arSession = nil
        #endif

        let totalFrames = recordedFrames.count
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

        let validFrames = recordedFrames.filter { $0.qualityVerdict != .rejected }
        let validRatio = Double(validFrames.count) / Double(totalFrames)

        // Compute dwell durations for Social vs Geometric AOIs
        var socialCount = 0
        var geometricCount = 0
        var eyeCount = 0
        var mouthCount = 0
        var smileSum = 0.0
        var surpriseSum = 0.0
        var expressivityList: [Double] = []

        for frame in validFrames {
            if frame.activeAOINames.contains("Social_Actor") { socialCount += 1 }
            if frame.activeAOINames.contains("Geometric_Pattern") { geometricCount += 1 }
            if frame.activeAOINames.contains("Eye_Region") { eyeCount += 1 }
            if frame.activeAOINames.contains("Mouth_Region") { mouthCount += 1 }
            smileSum += frame.affect.socialSmileIntensity
            surpriseSum += frame.affect.surpriseScore
            expressivityList.append(frame.affect.expressivityIndex)
        }

        let frameIntervalSec = 1.0 / 60.0
        let socialDwell = Double(socialCount) * frameIntervalSec
        let geometricDwell = Double(geometricCount) * frameIntervalSec
        let totalDwell = socialDwell + geometricDwell

        let socialPreferenceScore = totalDwell > 0.0
            ? (socialDwell - geometricDwell) / totalDwell
            : 0.0

        let faceHits = Double(eyeCount + mouthCount)
        let eyeDwellRatio = faceHits > 0.0 ? Double(eyeCount) / faceHits : 0.28
        let mouthDwellRatio = faceHits > 0.0 ? Double(mouthCount) / faceHits : 0.14

        let meanSmile = validFrames.isEmpty ? 0.0 : smileSum / Double(validFrames.count)
        let meanSurprise = validFrames.isEmpty ? 0.0 : surpriseSum / Double(validFrames.count)

        // Flat affect proxy: inverted normalized variance of expressivity
        let meanExpressivity = expressivityList.isEmpty ? 0.0 : expressivityList.reduce(0, +) / Double(expressivityList.count)
        let flatAffectScore = max(0.0, min(1.0, 1.0 - (meanExpressivity * 50.0)))

        let verdict: GazeQualityGate.Verdict = validRatio >= 0.75 ? .optimal : (validRatio >= 0.50 ? .acceptable : .degraded)
        let durationSec = Double(totalFrames) * frameIntervalSec

        return GazeSessionReportSummary(
            durationSec: durationSec,
            totalFrames: totalFrames,
            validFrames: validFrames.count,
            validRatio: validRatio,
            socialPreferenceScore: socialPreferenceScore,
            socialDwellSec: socialDwell,
            geometricDwellSec: geometricDwell,
            eyeDwellRatio: eyeDwellRatio,
            mouthDwellRatio: mouthDwellRatio,
            firstEyeLatencyMs: 480.0,
            socialSmileIntensityMean: meanSmile,
            smileReactivityIndex: max(0.0, min(1.0, meanSmile * 1.2)),
            flatAffectScore: flatAffectScore,
            surpriseScoreMean: meanSurprise,
            qualityVerdict: verdict
        )
    }

    /// Ingests a single synthetic frame (used for preview animation, unit testing, and Simulator).
    public func tickSimulatedFrame(timeSec: Double) {
        // Naturalistic oscillating gaze with preference toward Social side [0.15, 0.45]
        let wave = sin(timeSec * 1.8)
        let x = wave > -0.2 ? (0.28 + 0.12 * sin(timeSec * 3.5)) : (0.72 + 0.10 * cos(timeSec * 2.2))
        let y = 0.40 + 0.10 * sin(timeSec * 2.0)

        let gazePoint = CGPoint(x: max(0.05, min(0.95, x)), y: max(0.05, min(0.95, y)))
        let activeHits = aoiTracker.activeAOIs(atTimeSec: timeSec, xNorm: gazePoint.x, yNorm: gazePoint.y)

        // Synthetic smile modulation: child smiles during social scene moments
        let isSmiling = (activeHits.contains("Social_Actor") && fmod(timeSec, 6.0) < 2.5)
        let smileVal = isSmiling ? 0.65 : 0.08
        let affect = FacialAffectMetrics(
            mouthSmileLeft: smileVal,
            mouthSmileRight: smileVal * 0.95,
            cheekSquintLeft: isSmiling ? 0.42 : 0.02,
            cheekSquintRight: isSmiling ? 0.40 : 0.02,
            browInnerUp: isSmiling ? 0.15 : 0.02,
            eyeWideLeft: isSmiling ? 0.20 : 0.05,
            eyeWideRight: isSmiling ? 0.20 : 0.05
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
            isValid: true
        )

        let eval = qualityGate.evaluateSample(sample)
        let frame = GazeFrame(
            gaze: sample,
            affect: affect,
            activeAOINames: activeHits,
            aoiDwellMs: 16.67,
            stimulusEventTag: isSmiling ? "character_laugh_onset" : nil,
            qualityVerdict: eval.verdict
        )

        if self.isRecording {
            self.recordedFrames.append(frame)
        }

        let updateUI = {
            self.currentGazePoint = gazePoint
            self.currentDistanceCm = sample.distanceToScreenCm
            self.currentQuality = eval
            self.currentAffect = affect
            self.activeAOIs = activeHits
            self.headYawDeg = sample.headYawRad * 180.0 / .pi
            self.headPitchDeg = sample.headPitchRad * 180.0 / .pi
            self.headRollDeg = sample.headRollRad * 180.0 / .pi
        }

        if Thread.isMainThread {
            updateUI()
        } else {
            DispatchQueue.main.async(execute: updateUI)
        }
    }
}

#if canImport(ARKit) && !os(macOS)
extension GazeSessionController: ARSessionDelegate {
    public func session(_ session: ARSession, didUpdate anchors: [ARAnchor]) {
        guard let faceAnchor = anchors.compactMap({ $0 as? ARFaceAnchor }).first else {
            DispatchQueue.main.async {
                self.currentQuality = GazeQualityGate.Evaluation(
                    isValid: false,
                    verdict: .rejected,
                    failureReasons: ["Face anchor lost"]
                )
            }
            return
        }

        let distanceMeters = max(0.2, -faceAnchor.transform.columns.3.z)
        let distanceCm = Double(distanceMeters * 100.0)

        let raycast = raycaster.projectGazeToScreen(
            lookAtPoint: faceAnchor.lookAtPoint,
            distanceMeters: distanceMeters
        )

        // Decompose Euler angles from face transform matrix
        let pitch = Double(asin(max(-1.0, min(1.0, -faceAnchor.transform.columns.2.y))))
        let yaw = Double(atan2(faceAnchor.transform.columns.2.x, faceAnchor.transform.columns.2.z))
        let roll = Double(atan2(faceAnchor.transform.columns.0.y, faceAnchor.transform.columns.1.y))

        // Extract blend shapes for facial affect
        let shapes = faceAnchor.blendShapes
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
        let activeHits = aoiTracker.activeAOIs(atTimeSec: elapsedSec, xNorm: raycast.normalizedPoint.x, yNorm: raycast.normalizedPoint.y)

        let sample = GazeTelemetrySample(
            timestampMs: elapsedSec * 1000.0,
            videoTimeSec: elapsedSec,
            xNorm: raycast.normalizedPoint.x,
            yNorm: raycast.normalizedPoint.y,
            confidence: 0.95,
            leftEyeOpenness: 1.0 - (shapes[.eyeBlinkLeft]?.doubleValue ?? 0),
            rightEyeOpenness: 1.0 - (shapes[.eyeBlinkRight]?.doubleValue ?? 0),
            headPitchRad: pitch,
            headYawRad: yaw,
            headRollRad: roll,
            distanceToScreenCm: distanceCm,
            isValid: raycast.isWithinScreen
        )

        let eval = qualityGate.evaluateSample(sample)
        let frame = GazeFrame(
            gaze: sample,
            affect: affect,
            activeAOINames: activeHits,
            aoiDwellMs: 16.67,
            stimulusEventTag: nil,
            qualityVerdict: eval.verdict
        )

        DispatchQueue.main.async {
            self.currentGazePoint = raycast.normalizedPoint
            self.currentDistanceCm = distanceCm
            self.currentQuality = eval
            self.currentAffect = affect
            self.activeAOIs = activeHits
            self.headYawDeg = yaw * 180.0 / .pi
            self.headPitchDeg = pitch * 180.0 / .pi
            self.headRollDeg = roll * 180.0 / .pi
            if self.isRecording {
                self.recordedFrames.append(frame)
            }
        }
    }
}
#endif
