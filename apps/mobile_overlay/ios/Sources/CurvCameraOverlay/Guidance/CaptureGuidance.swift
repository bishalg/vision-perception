import CoreGraphics
import Foundation

/**
 * Domain-agnostic 5-gate capture evaluator.
 *
 * Operates on `SubjectPose` (supplied by the app's tracker) plus a photometric
 * sample. Palm, vastu, and future apps share this state machine and only swap
 * the pose mapper + overlay chrome.
 */
public final class CaptureGuidance {
    public var thresholds: CaptureThresholds
    private var previousPoints: [CGPoint] = []
    private var previousLuminance: Float = -1
    private var steadyStartTime: TimeInterval?

    public init(thresholds: CaptureThresholds = CaptureThresholds()) {
        self.thresholds = thresholds
    }

    public func reset() {
        previousPoints.removeAll()
        previousLuminance = -1
        steadyStartTime = nil
    }

    public func evaluate(
        pose: SubjectPose?,
        photometrics: PhotometricSample,
        timestamp: TimeInterval
    ) -> GuidanceEvaluation {
        var metrics = QualityMetrics()
        metrics.meanLuminance = photometrics.meanLuminance
        metrics.glareRatio = photometrics.glareRatio
        metrics.isIlluminationOk =
            photometrics.meanLuminance >= thresholds.minLuminance &&
            photometrics.glareRatio <= thresholds.maxGlareRatio

        guard let pose else {
            resetMotion()
            if thresholds.requireDetectedSubject {
                return GuidanceEvaluation(state: .searching, metrics: metrics)
            }
            return evaluateIlluminationAndSteady(
                metrics: metrics,
                photometrics: photometrics,
                timestamp: timestamp
            )
        }

        let centerDx = Float(pose.centroid.x - 0.5)
        let centerDy = Float(pose.centroid.y - 0.5)
        metrics.scale = pose.scale
        metrics.centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)
        metrics.rollAngleDeg = abs(pose.rollDeg)
        metrics.flatnessRatio = pose.aspectRatio
        metrics.isCenteringOk = metrics.centerOffset <= thresholds.maxCenterOffset
        metrics.isScaleOk = pose.scale >= thresholds.minScale && pose.scale <= thresholds.maxScale
        metrics.isAngleOk =
            metrics.rollAngleDeg <= thresholds.maxRollAngleDeg &&
            metrics.flatnessRatio >= thresholds.minFlatnessRatio

        updateMotion(pose: pose, photometrics: photometrics, metrics: &metrics)

        if !thresholds.skipGeometryGates {
            if !metrics.isCenteringOk {
                steadyStartTime = nil
                return GuidanceEvaluation(state: .centerSubject, metrics: metrics)
            }
            if pose.scale < thresholds.minScale {
                steadyStartTime = nil
                return GuidanceEvaluation(state: .moveCloser, metrics: metrics)
            }
            if pose.scale > thresholds.maxScale {
                steadyStartTime = nil
                return GuidanceEvaluation(state: .moveBack, metrics: metrics)
            }
            if !metrics.isAngleOk {
                steadyStartTime = nil
                return GuidanceEvaluation(state: .holdFlat, metrics: metrics)
            }
        }

        return evaluateIlluminationAndSteady(
            metrics: metrics,
            photometrics: photometrics,
            timestamp: timestamp
        )
    }

    private func evaluateIlluminationAndSteady(
        metrics: QualityMetrics,
        photometrics: PhotometricSample,
        timestamp: TimeInterval
    ) -> GuidanceEvaluation {
        var metrics = metrics
        if photometrics.meanLuminance < thresholds.minLuminance {
            steadyStartTime = nil
            return GuidanceEvaluation(state: .tooDark, metrics: metrics)
        }
        if photometrics.glareRatio > thresholds.maxGlareRatio {
            steadyStartTime = nil
            return GuidanceEvaluation(state: .glareDetected, metrics: metrics)
        }
        if !metrics.isSteadinessOk {
            steadyStartTime = nil
            return GuidanceEvaluation(state: .holdSteady, metrics: metrics)
        }

        if let start = steadyStartTime {
            let progress = Float(min(1.0, (timestamp - start) / thresholds.steadyDurationSec))
            metrics.isReadyForCapture = progress >= 1
            return GuidanceEvaluation(state: .ready(countdownProgress: progress), metrics: metrics)
        }
        steadyStartTime = timestamp
        return GuidanceEvaluation(state: .ready(countdownProgress: 0), metrics: metrics)
    }

    private func updateMotion(
        pose: SubjectPose,
        photometrics: PhotometricSample,
        metrics: inout QualityMetrics
    ) {
        if pose.samplePoints.count >= 2, previousPoints.count == pose.samplePoints.count {
            var sum: Float = 0
            for i in 0..<pose.samplePoints.count {
                let dx = Float(pose.samplePoints[i].x - previousPoints[i].x)
                let dy = Float(pose.samplePoints[i].y - previousPoints[i].y)
                sum += sqrt(dx * dx + dy * dy)
            }
            metrics.motionDisplacement = sum / Float(pose.samplePoints.count)
        } else if previousLuminance >= 0 {
            metrics.motionDisplacement = abs(photometrics.meanLuminance - previousLuminance) / 80.0
        } else {
            metrics.motionDisplacement = 0
        }
        previousPoints = pose.samplePoints
        previousLuminance = photometrics.meanLuminance
        metrics.isSteadinessOk = metrics.motionDisplacement <= thresholds.maxMotionDisplacement
    }

    private func resetMotion() {
        previousPoints.removeAll()
        previousLuminance = -1
        steadyStartTime = nil
    }
}
