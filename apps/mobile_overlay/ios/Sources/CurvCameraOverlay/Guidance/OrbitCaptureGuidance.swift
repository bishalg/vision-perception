import CoreGraphics
import Foundation

public enum OrbitGuidanceState: Equatable, Sendable {
    case idle
    case searching
    case lockPose
    case fitHand
    case moveCloser
    case addLight
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
    /// Rigidity reference for the CURRENT bin only — refreshed on every bin
    /// change. (Comparing every bin against the flat pose made any real
    /// tilt/twist fail the rigid check and stalled scans near 0%.)
    private var referenceLandmarks: [CGPoint]?
    /// Frozen flat-view reference + MCP-axis angle. All per-bin "did the hand
    /// actually move" gates are measured on the HAND against this — never on
    /// device roll, so a propped-up or steady phone works fine.
    private var flatReference: [CGPoint]?
    private var flatAxisAngle: CGFloat?
    private var lastTargetBin: OrbitBin?
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
        flatReference = nil
        flatAxisAngle = nil
        lastTargetBin = nil
        currentBinIndex = 0
        steadyStart = nil
    }

    public func evaluate(
        pose: SubjectPose?,
        landmarks: [CGPoint]?,
        photometrics: PhotometricSample,
        timestamp: TimeInterval
    ) -> (state: OrbitGuidanceState, shouldCapture: Bool, rigidCheck: RigidHandChecker.Result?) {
        // No hand tracked at all vs hand visible but unusable — coached
        // differently so the user is never stuck guessing.
        guard let landmarks, landmarks.count >= 21 else {
            steadyStart = nil
            return (.searching, false, nil)
        }
        guard let pose else {
            steadyStart = nil
            return (.fitHand, false, nil)
        }

        // Center + scale + illumination (skip roll/flatness for orbit).
        let centerDx = Float(pose.centroid.x - 0.5)
        let centerDy = Float(pose.centroid.y - 0.5)
        let centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)
        let illumOk = photometrics.meanLuminance >= thresholds.minLuminance &&
            photometrics.glareRatio <= thresholds.maxGlareRatio

        if centerOffset > thresholds.maxCenterOffset {
            steadyStart = nil
            return (.lockPose, false, nil)
        }
        if pose.scale < thresholds.minScale * 0.92 {
            steadyStart = nil
            return (.moveCloser, false, nil)
        }
        if pose.scale > thresholds.maxScale * 1.1 {
            steadyStart = nil
            return (.fitHand, false, nil)
        }
        if !illumOk {
            steadyStart = nil
            return (.addLight, false, nil)
        }

        let remaining = OrbitBin.allCases.filter { !session.filledBins.contains($0) }
        guard let targetBin = remaining.first else {
            session.completedAt = timestamp
            return (.complete, false, nil)
        }

        // Advance bin index to first unfilled.
        if let idx = OrbitBin.allCases.firstIndex(of: targetBin) {
            currentBinIndex = idx
        }

        // New angle → drop the old rigidity reference. It is re-latched below
        // once the hand is actually in the new angle, so moving between bins
        // is never punished as "not rigid".
        if targetBin != lastTargetBin {
            lastTargetBin = targetBin
            referenceLandmarks = nil
            steadyStart = nil
        }

        // Pose gate first: is the hand actually in the target angle? Measured
        // on hand landmarks only — tilting the phone is never required.
        if !poseGateSatisfied(for: targetBin, landmarks: landmarks) {
            steadyStart = nil
            return (.coach(bin: targetBin), false, nil)
        }

        // Latch the rigidity reference on the first in-angle frame.
        if referenceLandmarks == nil {
            referenceLandmarks = landmarks
            session.referenceLandmarks = landmarks.map { [Float($0.x), Float($0.y)] }
        }

        let rigid = RigidHandChecker.evaluate(current: landmarks, reference: referenceLandmarks)
        if !rigid.accepted {
            steadyStart = nil
            return (.rejected(reason: rigid.reason ?? "Keep hand rigid"), false, rigid)
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
        // Freeze the flat-view reference from the captured flat keyframe.
        // All per-bin pose gates compare against this.
        if frame.bin == .flatCenter, flatReference == nil {
            let pts = frame.landmarks.map { CGPoint(x: CGFloat($0[0]), y: CGFloat($0[1])) }
            if pts.count >= 21 {
                flatReference = pts
                flatAxisAngle = Self.mcpAxisAngleDeg(pts)
            }
        }
        session.frames.append(frame)
        if session.filledBins.count >= OrbitBin.allCases.count {
            session.completedAt = frame.timestamp
        }
    }

    // MARK: - Hand-measured pose gates

    /// MCP-axis angle (index knuckle -> pinky knuckle) in degrees, image space.
    /// Rotation magnitude is mirror/hand-agnostic, so left and right hands
    /// pass the tilt bins identically.
    private static func mcpAxisAngleDeg(_ landmarks: [CGPoint]) -> CGFloat? {
        guard landmarks.count > 17 else { return nil }
        let a = landmarks[5]
        let b = landmarks[17]
        return atan2(b.y - a.y, b.x - a.x) * 180 / .pi
    }

    private static func angleDeltaDeg(_ from: CGFloat, _ to: CGFloat) -> CGFloat {
        var d = to - from
        while d > 180 { d -= 360 }
        while d < -180 { d += 360 }
        return d
    }

    /// Direction-agnostic "is the hand actually in this angle" check.
    /// Thresholds deliberately lenient: a false pass costs one imperfect
    /// viewpoint frame, while a false reject stalls the whole scan.
    private func poseGateSatisfied(for bin: OrbitBin, landmarks: [CGPoint]) -> Bool {
        guard bin != .flatCenter else { return true }
        guard let flat = flatReference, flat.count >= 21, landmarks.count >= 21 else { return true }
        switch bin {
        case .flatCenter:
            return true
        case .tiltLeft, .tiltRight:
            guard let f = flatAxisAngle, let c = Self.mcpAxisAngleDeg(landmarks) else { return true }
            return abs(Self.angleDeltaDeg(f, c)) >= 8
        case .thumbToward, .thumbAway:
            return hypot(landmarks[4].x - flat[4].x, landmarks[4].y - flat[4].y) >= 0.03
        case .tiltForward:
            let tips = [8, 12, 16, 20]
            let peak = tips.map { hypot(landmarks[$0].x - flat[$0].x, landmarks[$0].y - flat[$0].y) }.max() ?? 0
            return peak >= 0.025
        }
    }

    public var statusMessage: String {
        switch evaluateStateOnly() {
        case .idle: return "Orbit scan ready"
        case .searching: return "Show your open palm"
        case .lockPose: return "Move your palm to the center"
        case .fitHand: return "Pull back — keep the whole hand visible"
        case .moveCloser: return "Move your hand closer to the camera"
        case .addLight: return "Add more light, avoid glare"
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
