import Foundation

/**
 * Phase 4 dermatoglyphics — one close-up per digit pad.
 * Stores pattern class only; never a biometric template.
 */
public enum FingertipDigit: String, CaseIterable, Codable, Sendable {
    case index, middle, ring, pinky, thumb

    public var displayName: String {
        rawValue.capitalized
    }

    /// Vision landmark index for fingertip.
    public var tipIndex: Int {
        switch self {
        case .thumb: return 4
        case .index: return 8
        case .middle: return 12
        case .ring: return 16
        case .pinky: return 20
        }
    }

    /// Vision landmark index for distal interphalangeal (or IP for thumb) joint.
    public var dipIndex: Int {
        switch self {
        case .thumb: return 3
        case .index: return 7
        case .middle: return 11
        case .ring: return 15
        case .pinky: return 19
        }
    }
}

public enum DermatoglyphicPattern: String, Codable, Sendable {
    case arch, loop, whorl, unknown
}

public struct FingertipScanResult: Codable, Identifiable, Sendable {
    public let id: UUID
    public let digit: FingertipDigit
    public let pattern: DermatoglyphicPattern
    public let ridgeFrequency: Float
    public let quality: Float
    public let timestamp: TimeInterval
    public var imagePath: String?

    public init(
        id: UUID = UUID(),
        digit: FingertipDigit,
        pattern: DermatoglyphicPattern,
        ridgeFrequency: Float,
        quality: Float,
        timestamp: TimeInterval,
        imagePath: String? = nil
    ) {
        self.id = id
        self.digit = digit
        self.pattern = pattern
        self.ridgeFrequency = ridgeFrequency
        self.quality = quality
        self.timestamp = timestamp
        self.imagePath = imagePath
    }
}

public struct FingertipScanSession: Codable, Sendable {
    public static let schemaVersion = "curv_fingertip_scan_v1"

    public let schema: String
    public var results: [FingertipScanResult]

    public init(results: [FingertipScanResult] = []) {
        self.schema = Self.schemaVersion
        self.results = results
    }

    public var scannedDigits: Set<FingertipDigit> { Set(results.map(\.digit)) }
}

public enum FingertipGuidanceState: Equatable, Sendable {
    case searching
    case adjustLighting(String)
    case positionPad(String)
    case holdingSteady(progress: Float)
    case cooldown(remaining: Float, capturedDigit: FingertipDigit)
    case complete
}

public final class FingertipCaptureGuidance {
    public var minPadFillRatio: Float = 0.28
    public var dwellDuration: CFTimeInterval = 1.2
    public var cooldownDuration: CFTimeInterval = 1.8

    private var steadyStartTime: CFTimeInterval?
    private var cooldownUntilTime: CFTimeInterval = 0
    private var lastCapturedDigit: FingertipDigit?

    public init(minPadFillRatio: Float = 0.28, dwellDuration: CFTimeInterval = 1.2, cooldownDuration: CFTimeInterval = 1.8) {
        self.minPadFillRatio = minPadFillRatio
        self.dwellDuration = dwellDuration
        self.cooldownDuration = cooldownDuration
    }

    public func reset() {
        steadyStartTime = nil
        cooldownUntilTime = 0
        lastCapturedDigit = nil
    }

    public func notifyCaptured(digit: FingertipDigit, timestamp: CFTimeInterval) {
        lastCapturedDigit = digit
        cooldownUntilTime = timestamp + cooldownDuration
        steadyStartTime = nil
    }

    public func nextDigit(in session: FingertipScanSession) -> FingertipDigit? {
        FingertipDigit.allCases.first { !session.scannedDigits.contains($0) }
    }

    /// Evaluates fingertip positioning, illumination, scale, and dwell hold.
    public func evaluate(
        landmarks: [CGPoint],
        target: FingertipDigit,
        photometrics: PhotometricSample,
        timestamp: CFTimeInterval
    ) -> (ready: Bool, progress: Float, state: FingertipGuidanceState, message: String) {
        // Cooldown between captured digits
        if timestamp < cooldownUntilTime, let last = lastCapturedDigit {
            steadyStartTime = nil
            let remaining = Float(max(0, cooldownUntilTime - timestamp))
            let msg = "\(last.displayName) captured! Next: \(target.displayName)"
            return (false, 0.0, .cooldown(remaining: remaining, capturedDigit: last), msg)
        }

        if photometrics.glareRatio > 0.38 {
            steadyStartTime = nil
            let msg = "Reduce glare on \(target.displayName) pad"
            return (false, 0.0, .adjustLighting(msg), msg)
        }
        if photometrics.meanLuminance < 0.15 {
            steadyStartTime = nil
            let msg = "More light needed on \(target.displayName) pad"
            return (false, 0.0, .adjustLighting(msg), msg)
        }

        // When hand landmarks are available
        if landmarks.count > target.tipIndex {
            let tip = landmarks[target.tipIndex]
            let dipIndex = target.dipIndex
            let padLength: CGFloat
            if landmarks.count > dipIndex {
                let dip = landmarks[dipIndex]
                padLength = hypot(tip.x - dip.x, tip.y - dip.y)
            } else if landmarks.count > 0 {
                padLength = hypot(tip.x - landmarks[0].x, tip.y - landmarks[0].y) * 0.20
            } else {
                padLength = 0.10
            }

            let centerDist = hypot(tip.x - 0.5, tip.y - 0.48)
            if centerDist > 0.35 {
                steadyStartTime = nil
                let msg = "Center \(target.displayName) pad in guide ring"
                return (false, 0.0, .positionPad(msg), msg)
            }

            if padLength < 0.045 {
                steadyStartTime = nil
                let msg = "Move closer — enlarge \(target.displayName) pad"
                return (false, 0.0, .positionPad(msg), msg)
            }

            // Steady in reticle: track dwell duration
            let start = steadyStartTime ?? timestamp
            steadyStartTime = start
            let elapsed = timestamp - start
            let progress = min(1.0, Float(elapsed / dwellDuration))

            if progress >= 1.0 {
                return (true, 1.0, .holdingSteady(progress: 1.0), "Capturing \(target.displayName)...")
            } else {
                return (false, progress, .holdingSteady(progress: progress), "Hold \(target.displayName) steady...")
            }
        }

        // When hand skeleton is not visible, user is centering pad directly in macro circle
        // Require adequate illumination and reasonable non-glare lighting
        if photometrics.meanLuminance >= 0.22 && photometrics.glareRatio <= 0.35 {
            let start = steadyStartTime ?? timestamp
            steadyStartTime = start
            let elapsed = timestamp - start
            let progress = min(1.0, Float(elapsed / dwellDuration))

            if progress >= 1.0 {
                return (true, 1.0, .holdingSteady(progress: 1.0), "Capturing \(target.displayName)...")
            } else {
                return (false, progress, .holdingSteady(progress: progress), "Hold \(target.displayName) pad steady...")
            }
        }

        steadyStartTime = nil
        let msg = "Place \(target.displayName) pad in circle"
        return (false, 0.0, .positionPad(msg), msg)
    }
}
