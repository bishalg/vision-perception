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
    private var referenceLandmarks: [CGPoint]?
    private var flatReference: [CGPoint]?
    private var flatAxisAngle: CGFloat?
    private var lastTargetBin: OrbitBin?
    private var currentBinIndex: Int = 0
    private var steadyStart: TimeInterval?
    private var dwellProgress: Float = 0.0
    private var lastEvalTime: TimeInterval?
    private let requiredHoldDuration: TimeInterval = 0.38
    private var skippedBins: Set<OrbitBin> = []

    public init(thresholds: CaptureThresholds = CaptureThresholds(), handSide: String = "auto") {
        self.thresholds = thresholds
        self.session = OrbitCaptureSession(handSide: handSide)
    }

    public var orbitSession: OrbitCaptureSession { session }

    public var canCompleteEarly: Bool {
        session.filledBins.count >= 3 && session.filledBins.contains(.flatCenter)
    }

    public func reset(handSide: String = "auto") {
        session = OrbitCaptureSession(handSide: handSide)
        referenceLandmarks = nil
        flatReference = nil
        flatAxisAngle = nil
        lastTargetBin = nil
        currentBinIndex = 0
        steadyStart = nil
        dwellProgress = 0.0
        lastEvalTime = nil
        skippedBins.removeAll()
        lastState = .idle
    }

    public func skipCurrentBin() {
        let remaining = OrbitBin.allCases.filter { !session.filledBins.contains($0) && !skippedBins.contains($0) }
        if let current = remaining.first {
            skippedBins.insert(current)
            referenceLandmarks = nil
            steadyStart = nil
            dwellProgress = 0.0
        }
    }

    public func forceComplete() {
        session.completedAt = Date().timeIntervalSince1970
        lastState = .complete
    }

    private func decayDwell(timestamp: TimeInterval) {
        if let last = lastEvalTime {
            let dt = Float(max(0, timestamp - last))
            dwellProgress = max(0, dwellProgress - dt * 1.5)
        } else {
            dwellProgress = 0
        }
        lastEvalTime = timestamp
        steadyStart = nil
    }

    public func evaluate(
        pose: SubjectPose?,
        landmarks: [CGPoint]?,
        photometrics: PhotometricSample,
        timestamp: TimeInterval,
        deviceRollDeg: Float? = nil
    ) -> (state: OrbitGuidanceState, shouldCapture: Bool, rigidCheck: RigidHandChecker.Result?) {
        // No hand tracked at all vs hand visible but unusable
        guard let landmarks, landmarks.count >= 21 else {
            decayDwell(timestamp: timestamp)
            return (.searching, false, nil)
        }
        guard let pose else {
            decayDwell(timestamp: timestamp)
            return (.fitHand, false, nil)
        }

        // Center + scale + illumination (tolerant for mobile freehand tilt)
        let centerDx = Float(pose.centroid.x - 0.5)
        let centerDy = Float(pose.centroid.y - 0.5)
        let centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)

        let maxCenter: Float = max(thresholds.maxCenterOffset, 0.28)
        let minScale: Float = min(thresholds.minScale * 0.92, 0.22)
        let maxScale: Float = max(thresholds.maxScale * 1.15, 0.72)
        let minLum: Float = min(thresholds.minLuminance, 25.0)
        let maxGlare: Float = max(thresholds.maxGlareRatio, 0.08)

        let illumOk = photometrics.meanLuminance >= minLum && photometrics.glareRatio <= maxGlare

        if centerOffset > maxCenter {
            decayDwell(timestamp: timestamp)
            return (.lockPose, false, nil)
        }
        if pose.scale < minScale {
            decayDwell(timestamp: timestamp)
            return (.moveCloser, false, nil)
        }
        if pose.scale > maxScale {
            decayDwell(timestamp: timestamp)
            return (.fitHand, false, nil)
        }
        if !illumOk {
            decayDwell(timestamp: timestamp)
            return (.addLight, false, nil)
        }

        let remaining = OrbitBin.allCases.filter { !session.filledBins.contains($0) && !skippedBins.contains($0) }
        guard let targetBin = remaining.first else {
            session.completedAt = timestamp
            return (.complete, false, nil)
        }

        if let idx = OrbitBin.allCases.firstIndex(of: targetBin) {
            currentBinIndex = idx
        }

        if targetBin != lastTargetBin {
            lastTargetBin = targetBin
            referenceLandmarks = nil
            steadyStart = nil
            dwellProgress = 0.0
        }

        // Check if hand or device is in target angle
        if !poseGateSatisfied(for: targetBin, landmarks: landmarks, deviceRoll: deviceRollDeg) {
            decayDwell(timestamp: timestamp)
            referenceLandmarks = nil
            return (.coach(bin: targetBin), false, nil)
        }

        if referenceLandmarks == nil {
            referenceLandmarks = landmarks
            session.referenceLandmarks = landmarks.map { [Float($0.x), Float($0.y)] }
        }

        let rigid = RigidHandChecker.evaluate(current: landmarks, reference: referenceLandmarks)
        if !rigid.accepted {
            decayDwell(timestamp: timestamp)
            // As user settles into the target angle, update reference so moving INTO position is not rejected
            if dwellProgress <= 0.10 {
                referenceLandmarks = landmarks
            }
            return (.rejected(reason: rigid.reason ?? "Keep hand steady"), false, rigid)
        }

        // Smooth dwell accumulation (resilient against single-frame jitters)
        if steadyStart == nil {
            steadyStart = timestamp
        }
        let dt = Float(max(0, timestamp - (lastEvalTime ?? timestamp)))
        lastEvalTime = timestamp
        dwellProgress = min(1.0, dwellProgress + (dt > 0 ? dt / Float(requiredHoldDuration) : Float(1.0 / 30.0 / requiredHoldDuration)))

        if dwellProgress >= 1.0 {
            steadyStart = nil
            dwellProgress = 0.0
            return (.capturing(bin: targetBin), true, rigid)
        }
        return (.holdSteady(bin: targetBin, progress: dwellProgress), false, rigid)
    }

    public func recordFrame(_ frame: OrbitFrame) {
        if session.referenceGravity == nil {
            session.referenceGravity = frame.gravity
        }
        if frame.bin == .flatCenter, flatReference == nil {
            let pts = frame.landmarks.map { CGPoint(x: CGFloat($0[0]), y: CGFloat($0[1])) }
            if pts.count >= 21 {
                flatReference = pts
                flatAxisAngle = Self.mcpAxisAngleDeg(pts)
            }
        }
        session.frames.append(frame)
        let unhandled = OrbitBin.allCases.filter { !session.filledBins.contains($0) && !skippedBins.contains($0) }
        if unhandled.isEmpty || session.filledBins.count >= OrbitBin.allCases.count {
            session.completedAt = frame.timestamp
        }
    }

    // MARK: - Hand & Gyro pose gates

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

    private func poseGateSatisfied(for bin: OrbitBin, landmarks: [CGPoint], deviceRoll: Float? = nil) -> Bool {
        guard bin != .flatCenter else { return true }
        guard let flat = flatReference, flat.count >= 21, landmarks.count >= 21 else { return true }

        // 1. Hardware device gyro fallback: user tilted phone to target angle
        if let roll = deviceRoll {
            switch bin {
            case .tiltLeft:
                if roll >= 10.0 { return true }
            case .tiltRight:
                if roll <= -10.0 { return true }
            default:
                break
            }
        }

        // 2. In-plane roll & 3D yaw/foreshortening
        let flatAngle = flatAxisAngle ?? Self.mcpAxisAngleDeg(flat) ?? 0
        let currentAngle = Self.mcpAxisAngleDeg(landmarks) ?? 0
        let deltaAngle = Self.angleDeltaDeg(flatAngle, currentAngle)

        let p0 = landmarks[0]
        let p9 = landmarks[9]
        let axisDx = p9.x - p0.x
        let axisDy = p9.y - p0.y
        let axisLen = hypot(axisDx, axisDy)
        let normAxisX = axisLen > 1e-4 ? axisDx / axisLen : 0
        let normAxisY = axisLen > 1e-4 ? axisDy / axisLen : 1
        let perpX = -normAxisY
        let perpY = normAxisX

        let curThumbLat = (landmarks[4].x - p0.x) * perpX + (landmarks[4].y - p0.y) * perpY
        let curPinkyLat = (landmarks[17].x - p0.x) * perpX + (landmarks[17].y - p0.y) * perpY
        let flatP0 = flat[0]
        let flatThumbLat = (flat[4].x - flatP0.x) * perpX + (flat[4].y - flatP0.y) * perpY
        let flatPinkyLat = (flat[17].x - flatP0.x) * perpX + (flat[17].y - flatP0.y) * perpY

        let curPalmWidth = hypot(landmarks[17].x - landmarks[5].x, landmarks[17].y - landmarks[5].y)
        let flatPalmWidth = hypot(flat[17].x - flat[5].x, flat[17].y - flat[5].y)
        let widthRatio = flatPalmWidth > 1e-4 ? (curPalmWidth / flatPalmWidth) : 1.0

        switch bin {
        case .flatCenter:
            return true

        case .tiltLeft:
            // Tilting left: negative deltaAngle (<= -5.0 deg), positive device roll (>= 6.0 deg), or left lateral displacement
            let inPlaneTiltLeft = deltaAngle <= -5.0
            let gyroTiltLeft = (deviceRoll ?? 0) >= 6.0
            let lateralShiftLeft = (curPinkyLat - flatPinkyLat) < -0.012 || (curThumbLat - flatThumbLat) < -0.012
            let yawLeft = widthRatio < 0.94 && deltaAngle <= 0.0
            return inPlaneTiltLeft || gyroTiltLeft || lateralShiftLeft || yawLeft

        case .tiltRight:
            // Tilting right: positive deltaAngle (>= 5.0 deg), negative device roll (<= -6.0 deg), or right lateral displacement
            let inPlaneTiltRight = deltaAngle >= 5.0
            let gyroTiltRight = (deviceRoll ?? 0) <= -6.0
            let lateralShiftRight = (curPinkyLat - flatPinkyLat) > 0.012 || (curThumbLat - flatThumbLat) > 0.012
            let yawRight = widthRatio < 0.94 && deltaAngle >= 0.0
            return inPlaneTiltRight || gyroTiltRight || lateralShiftRight || yawRight

        case .thumbToward:
            let thumbDistance = hypot(landmarks[4].x - landmarks[9].x, landmarks[4].y - landmarks[9].y)
            let flatThumbDistance = hypot(flat[4].x - flat[9].x, flat[4].y - flat[9].y)
            return (thumbDistance < flatThumbDistance - 0.012) || hypot(landmarks[4].x - flat[4].x, landmarks[4].y - flat[4].y) >= 0.022

        case .thumbAway:
            let thumbDistance = hypot(landmarks[4].x - landmarks[9].x, landmarks[4].y - landmarks[9].y)
            let flatThumbDistance = hypot(flat[4].x - flat[9].x, flat[4].y - flat[9].y)
            return (thumbDistance > flatThumbDistance + 0.012) || hypot(landmarks[4].x - flat[4].x, landmarks[4].y - flat[4].y) >= 0.022

        case .tiltForward:
            let curFingerLen = hypot(landmarks[12].x - landmarks[9].x, landmarks[12].y - landmarks[9].y)
            let flatFingerLen = hypot(flat[12].x - flat[9].x, flat[12].y - flat[9].y)
            let foreshortened = curFingerLen < flatFingerLen * 0.92
            let tipsDisplaced = [8, 12, 16, 20].map { hypot(landmarks[$0].x - flat[$0].x, landmarks[$0].y - flat[$0].y) }.max() ?? 0
            return foreshortened || tipsDisplaced >= 0.018
        }
    }

    public var statusMessage: String {
        switch evaluateStateOnly() {
        case .idle: return "Orbit scan ready"
        case .searching: return "Show your open palm"
        case .lockPose: return "Center palm in frame"
        case .fitHand: return "Pull back slightly — show whole hand"
        case .moveCloser: return "Move hand closer to camera"
        case .addLight: return "Add more light"
        case .coach(let bin): return bin.displayName
        case .holdSteady(let bin, let p): return "\(bin.displayName) — hold (\(Int(p * 100))%)"
        case .capturing(let bin): return "Capturing \(bin.rawValue)…"
        case .rejected(let reason): return reason
        case .complete: return "3D Orbit scan complete"
        }
    }

    private var lastState: OrbitGuidanceState = .idle

    private func evaluateStateOnly() -> OrbitGuidanceState { lastState }

    public func setLastState(_ state: OrbitGuidanceState) {
        lastState = state
    }
}
