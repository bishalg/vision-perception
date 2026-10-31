// Former Swift body of `GazeQualityGate` at dev@3718ea6, moved here by WP-A.11 (FFI-3).
// The production type now delegates to the Rust core; this copy is the parity oracle for
// one release (GoldenParityTests, LegacyParityTests). Do not change its behaviour.
@testable import CurvAsdGazeKit
import Foundation

/// `GazeQualityGate.evaluateSample` as it was, reading the thresholds from a production gate.
struct LegacyGazeQualityGate {
    typealias Verdict = GazeQualityGate.Verdict
    typealias Evaluation = GazeQualityGate.Evaluation

    let gate: GazeQualityGate
    var minDistanceCm: Double { gate.minDistanceCm }
    var maxDistanceCm: Double { gate.maxDistanceCm }
    var maxYawRad: Double { gate.maxYawRad }
    var maxPitchRad: Double { gate.maxPitchRad }
    var minConfidence: Double { gate.minConfidence }
    var blinkThreshold: Double { gate.blinkThreshold }

    init(_ gate: GazeQualityGate = GazeQualityGate()) {
        self.gate = gate
    }

    /// Evaluates a single sample against pediatric acquisition criteria.
    func evaluateSample(_ sample: GazeTelemetrySample) -> Evaluation {
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
