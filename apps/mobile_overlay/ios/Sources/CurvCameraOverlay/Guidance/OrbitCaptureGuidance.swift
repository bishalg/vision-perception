import CoreGraphics
import Foundation

public enum OrbitGuidanceState: Equatable, Sendable {
    case idle
    case searching
    case lockPose
    case coach(bin: OrbitBin)
    case holdSteady(bin: OrbitBin, progress: Float)
    case capturing(bin: OrbitBin)
    case rejected(reason: String)
    case complete
}

/**
 * Mode B orbit state machine — runs beside CaptureGuidance (Mode A unchanged).
 * Relaxes roll/flatness gates; coaches discrete tilt bins; auto-grabs keyframes.
 */
public final class OrbitCaptureGuidance {
    public var thresholds: CaptureThresholds
    private var session: OrbitCaptureSession
    private var referenceLandmarks: [CGPoint]?
    private var currentBinIndex: Int = 0
    private var steadyStart: TimeInterval?
    private let steadyDuration: TimeInterval = 0.55

    public init(thresholds: CaptureThresholds = CaptureThresholds(), handSide: String = "auto") {
        self.thresholds = thresholds
        self.session = OrbitCaptureSession(handSide: handSide)
    }

    public var orbitSession: OrbitCaptureSession { session }

    public func reset(handSide: String = "auto") {
        session = OrbitCaptureSession(handSide: handSide)
        referenceLandmarks = nil
        currentBinIndex = 0
        steadyStart = nil
    }

    public func evaluate(
        pose: SubjectPose?,
        landmarks: [CGPoint]?,
        photometrics: PhotometricSample,
        deviceRollDeg: Float,
        timestamp: TimeInterval
    ) -> (state: OrbitGuidanceState, shouldCapture: Bool, rigidCheck: RigidHandChecker.Result?) {
        guard let pose, let landmarks, landmarks.count >= 21 else {
            steadyStart = nil
            return (.searching, false, nil)
        }

        let rigid = RigidHandChecker.evaluate(current: landmarks, reference: referenceLandmarks)
        if !rigid.accepted {
            steadyStart = nil
            return (.rejected(reason: rigid.reason ?? "Keep hand rigid"), false, rigid)
        }

        // Center + scale + illumination (skip roll/flatness for orbit).
        let centerDx = Float(pose.centroid.x - 0.5)
        let centerDy = Float(pose.centroid.y - 0.5)
        let centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)
        let illumOk = photometrics.meanLuminance >= thresholds.minLuminance &&
            photometrics.glareRatio <= thresholds.maxGlareRatio

        if centerOffset > thresholds.maxCenterOffset {
            steadyStart = nil
            return (.lockPose, false, rigid)
        }
        if pose.scale < thresholds.minScale * 0.92 {
            steadyStart = nil
            return (.searching, false, rigid)
        }
        if !illumOk {
            steadyStart = nil
            return (.lockPose, false, rigid)
        }

        if referenceLandmarks == nil {
            referenceLandmarks = landmarks
            session.referenceLandmarks = landmarks.map { [Float($0.x), Float($0.y)] }
        }

        let remaining = OrbitBin.allCases.filter { !session.filledBins.contains($0) }
        guard let targetBin = remaining.first else {
            session.completedAt = timestamp
            return (.complete, false, rigid)
        }

        // Advance bin index to first unfilled.
        if let idx = OrbitBin.allCases.firstIndex(of: targetBin) {
            currentBinIndex = idx
        }

        let rollOk = targetBin.expectedRollDeg.contains(deviceRollDeg)
        if !rollOk && (targetBin == .tiltLeft || targetBin == .tiltRight) {
            steadyStart = nil
            return (.coach(bin: targetBin), false, rigid)
        }

        if steadyStart == nil {
            steadyStart = timestamp
        }
        let elapsed = timestamp - (steadyStart ?? timestamp)
        let progress = Float(min(1.0, elapsed / steadyDuration))

        if progress >= 1.0 {
            steadyStart = nil
            return (.capturing(bin: targetBin), true, rigid)
        }
        return (.holdSteady(bin: targetBin, progress: progress), false, rigid)
    }

    public func recordFrame(_ frame: OrbitFrame) {
        if session.referenceGravity == nil {
            session.referenceGravity = frame.gravity
        }
        session.frames.append(frame)
        if session.filledBins.count >= OrbitBin.allCases.count {
            session.completedAt = frame.timestamp
        }
    }

    public var statusMessage: String {
        switch evaluateStateOnly() {
        case .idle: return "Orbit scan ready"
        case .searching: return "Show open palm to begin orbit scan"
        case .lockPose: return "Center palm and improve lighting"
        case .coach(let bin): return bin.displayName
        case .holdSteady(let bin, let p): return "\(bin.displayName) — hold (\(Int(p * 100))%)"
        case .capturing(let bin): return "Capturing \(bin.rawValue)…"
        case .rejected(let reason): return reason
        case .complete: return "Orbit scan complete"
        }
    }

    private var lastState: OrbitGuidanceState = .idle

    private func evaluateStateOnly() -> OrbitGuidanceState { lastState }

    public func setLastState(_ state: OrbitGuidanceState) {
        lastState = state
    }
}
