package org.curv.overlay.guidance

import kotlin.math.abs
import kotlin.math.min
import kotlin.math.sqrt

/**
 * Domain-agnostic 5-gate capture evaluator. Palm / vastu apps only supply
 * `SubjectPose` + photometrics; they do not reimplement the state machine.
 */
class CaptureGuidance(
    var thresholds: CaptureThresholds = CaptureThresholds()
) {
    private var previousPoints: List<android.graphics.PointF> = emptyList()
    private var previousLuminance: Float = -1f
    private var steadyStartTimeSec: Double? = null

    fun reset() {
        previousPoints = emptyList()
        previousLuminance = -1f
        steadyStartTimeSec = null
    }

    fun evaluate(
        pose: SubjectPose?,
        photometrics: PhotometricSample,
        timestampSec: Double
    ): GuidanceEvaluation {
        val metrics = QualityMetrics(
            meanLuminance = photometrics.meanLuminance,
            glareRatio = photometrics.glareRatio,
            isIlluminationOk = photometrics.meanLuminance >= thresholds.minLuminance &&
                photometrics.glareRatio <= thresholds.maxGlareRatio
        )

        if (pose == null) {
            resetMotion()
            return if (thresholds.requireDetectedSubject) {
                GuidanceEvaluation(GuidanceState.Searching, metrics)
            } else {
                evaluateIlluminationAndSteady(metrics, photometrics, timestampSec)
            }
        }

        val centerDx = pose.centroid.x - 0.5f
        val centerDy = pose.centroid.y - 0.5f
        metrics.scale = pose.scale
        metrics.centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)
        metrics.rollAngleDeg = abs(pose.rollDeg)
        metrics.flatnessRatio = pose.aspectRatio
        metrics.isCenteringOk = metrics.centerOffset <= thresholds.maxCenterOffset
        metrics.isScaleOk = pose.scale >= thresholds.minScale && pose.scale <= thresholds.maxScale
        metrics.isAngleOk =
            metrics.rollAngleDeg <= thresholds.maxRollAngleDeg &&
                metrics.flatnessRatio >= thresholds.minFlatnessRatio

        updateMotion(pose, photometrics, metrics)

        if (!thresholds.skipGeometryGates) {
            if (!metrics.isCenteringOk) {
                steadyStartTimeSec = null
                return GuidanceEvaluation(GuidanceState.CenterSubject, metrics)
            }
            if (pose.scale < thresholds.minScale) {
                steadyStartTimeSec = null
                return GuidanceEvaluation(GuidanceState.MoveCloser, metrics)
            }
            if (pose.scale > thresholds.maxScale) {
                steadyStartTimeSec = null
                return GuidanceEvaluation(GuidanceState.MoveBack, metrics)
            }
            if (!metrics.isAngleOk) {
                steadyStartTimeSec = null
                return GuidanceEvaluation(GuidanceState.HoldFlat, metrics)
            }
        }

        return evaluateIlluminationAndSteady(metrics, photometrics, timestampSec)
    }

    private fun evaluateIlluminationAndSteady(
        metrics: QualityMetrics,
        photometrics: PhotometricSample,
        timestampSec: Double
    ): GuidanceEvaluation {
        if (photometrics.meanLuminance < thresholds.minLuminance) {
            steadyStartTimeSec = null
            return GuidanceEvaluation(GuidanceState.TooDark, metrics)
        }
        if (photometrics.glareRatio > thresholds.maxGlareRatio) {
            steadyStartTimeSec = null
            return GuidanceEvaluation(GuidanceState.GlareDetected, metrics)
        }
        if (!metrics.isSteadinessOk) {
            steadyStartTimeSec = null
            return GuidanceEvaluation(GuidanceState.HoldSteady, metrics)
        }

        val start = steadyStartTimeSec
        return if (start != null) {
            val progress = min(1.0f, ((timestampSec - start) / thresholds.steadyDurationSec).toFloat())
            metrics.isReadyForCapture = progress >= 1.0f
            GuidanceEvaluation(GuidanceState.Ready(progress), metrics)
        } else {
            steadyStartTimeSec = timestampSec
            GuidanceEvaluation(GuidanceState.Ready(0f), metrics)
        }
    }

    private fun updateMotion(pose: SubjectPose, photometrics: PhotometricSample, metrics: QualityMetrics) {
        if (pose.samplePoints.size >= 2 && previousPoints.size == pose.samplePoints.size) {
            var sum = 0f
            for (i in pose.samplePoints.indices) {
                val dx = pose.samplePoints[i].x - previousPoints[i].x
                val dy = pose.samplePoints[i].y - previousPoints[i].y
                sum += sqrt(dx * dx + dy * dy)
            }
            metrics.motionDisplacement = sum / pose.samplePoints.size
        } else if (previousLuminance >= 0f) {
            metrics.motionDisplacement = abs(photometrics.meanLuminance - previousLuminance) / 80f
        } else {
            metrics.motionDisplacement = 0f
        }
        previousPoints = pose.samplePoints
        previousLuminance = photometrics.meanLuminance
        metrics.isSteadinessOk = metrics.motionDisplacement <= thresholds.maxMotionDisplacement
    }

    private fun resetMotion() {
        previousPoints = emptyList()
        previousLuminance = -1f
        steadyStartTimeSec = null
    }
}
