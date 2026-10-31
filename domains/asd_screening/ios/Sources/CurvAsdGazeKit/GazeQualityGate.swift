import Foundation

/// Real-time quality verification for pediatric gaze capture.
///
/// The checks and their reason strings live in the Rust core (`asd-core::quality`, CORE-7);
/// this type keeps the public Swift API and delegates (FFI-3). The default thresholds mirror
/// `ScoringProfile.qualityGate` of the core (checked by a test).
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

    public struct Evaluation: Sendable, Equatable, Decodable {
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
        // Encoded once: `evaluateSample` runs for every 60 Hz sample.
        self.gateJSON = (try? AsdCoreBridge.encode(CoreQualityGate(
            minDistanceCm: minDistanceCm,
            maxDistanceCm: maxDistanceCm,
            maxYawRad: maxYawRad,
            maxPitchRad: maxPitchRad,
            minConfidence: minConfidence,
            blinkThreshold: blinkThreshold
        ))) ?? ""
    }

    /// The thresholds as the core's gate document (`ScoringProfile.qualityGate`).
    private let gateJSON: String

    /// Evaluates a single sample against pediatric acquisition criteria.
    public func evaluateSample(_ sample: GazeTelemetrySample) -> Evaluation {
        do {
            return try AsdCoreBridge.evaluateQuality(sample, gateJSON: gateJSON)
        } catch {
            AsdCoreBridge.report("evaluate_quality", error)
            // A sample the core cannot read (for example a non-finite reading) is not data.
            return Evaluation(
                isValid: false,
                verdict: .rejected,
                failureReasons: ["Tracking data unreadable"],
                isBlink: false
            )
        }
    }
}
