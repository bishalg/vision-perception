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

public final class FingertipCaptureGuidance {
    public var minPadFillRatio: Float = 0.28

    public init(minPadFillRatio: Float = 0.28) {
        self.minPadFillRatio = minPadFillRatio
    }

    public func nextDigit(in session: FingertipScanSession) -> FingertipDigit? {
        FingertipDigit.allCases.first { !session.scannedDigits.contains($0) }
    }

    /// Tip should occupy ~70% of frame width for ridge visibility.
    public func evaluate(
        landmarks: [CGPoint],
        target: FingertipDigit,
        photometrics: PhotometricSample
    ) -> (ready: Bool, message: String) {
        guard landmarks.count > target.tipIndex else {
            return (false, "Show \(target.displayName) fingertip")
        }
        let tip = landmarks[target.tipIndex]
        let wrist = landmarks[0]
        let padSize = hypot(tip.x - wrist.x, tip.y - wrist.y)
        if padSize < CGFloat(minPadFillRatio) {
            return (false, "Move closer — fill frame with \(target.displayName) pad")
        }
        if photometrics.glareRatio > 0.35 {
            return (false, "Reduce glare on \(target.displayName) pad")
        }
        return (true, "Hold \(target.displayName) pad steady")
    }
}
