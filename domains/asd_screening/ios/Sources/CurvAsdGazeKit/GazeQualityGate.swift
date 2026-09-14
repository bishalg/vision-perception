import Foundation

/// Real-time quality verification for pediatric gaze capture.
public struct GazeQualityGate: Sendable {
    public let minDistanceCm: Double
    public let maxDistanceCm: Double
    public let maxYawRad: Double
    public let maxPitchRad: Double
    public let minConfidence: Double
    public let blinkThreshold: Double

    public enum Verdict: String, Codable, Sendable {
        case optimal
        case acceptable
        case degraded
        case rejected
    }

    public struct Evaluation: Sendable, Equatable {
        public let isValid: Bool
        public let verdict: Verdict
        public let failureReasons: [String]
        public let isBlink: Bool

        public init(
            isValid: Bool,
            verdict: Verdict,
            failureReasons: [String] = [],
            isBlink: Bool = false
        ) {
            self.isValid = isValid
            self.verdict = verdict
            self.failureReasons = failureReasons
            self.isBlink = isBlink
        }
    }

    public init(
        minDistanceCm: Double = 35.0,
        maxDistanceCm: Double = 65.0,
        maxYawRad: Double = 0.61,    // ~35 degrees
        maxPitchRad: Double = 0.52,  // ~30 degrees
        minConfidence: Double = 0.50,
        blinkThreshold: Double = 0.45
    ) {
        self.minDistanceCm = minDistanceCm
        self.maxDistanceCm = maxDistanceCm
        self.maxYawRad = maxYawRad
        self.maxPitchRad = maxPitchRad
        self.minConfidence = minConfidence
        self.blinkThreshold = blinkThreshold
    }

    /// Evaluates a single sample against pediatric acquisition criteria.
    public func evaluateSample(_ sample: GazeTelemetrySample) -> Evaluation {
        var reasons: [String] = []

        // 1. Distance check
        if sample.distanceToScreenCm < minDistanceCm {
            reasons.append("Subject too close (< \(Int(minDistanceCm)) cm)")
        } else if sample.distanceToScreenCm > maxDistanceCm {
            reasons.append("Subject too far (> \(Int(maxDistanceCm)) cm)")
        }

        // 2. Head pose check
        if abs(sample.headYawRad) > maxYawRad {
            reasons.append("Excessive head yaw (> \(Int(maxYawRad * 180 / .pi))°)")
        }
        if abs(sample.headPitchRad) > maxPitchRad {
            reasons.append("Excessive head pitch (> \(Int(maxPitchRad * 180 / .pi))°)")
        }

        // 3. Eye closure / blink check (either eye occluded triggers blink rejection)
        let isBlink = sample.leftEyeOpenness < blinkThreshold || sample.rightEyeOpenness < blinkThreshold
        if isBlink {
            reasons.append("Blink or ocular occlusion")
        }

        // 4. Tracking confidence check
        if sample.confidence < minConfidence {
            reasons.append("Low tracking confidence (< \(Int(minConfidence * 100))%)")
        }

        if !reasons.isEmpty {
            let verdict: Verdict = (isBlink || reasons.count > 2) ? .rejected : .degraded
            return Evaluation(isValid: false, verdict: verdict, failureReasons: reasons, isBlink: isBlink)
        }

        return Evaluation(isValid: true, verdict: .optimal, isBlink: false)
    }
}
