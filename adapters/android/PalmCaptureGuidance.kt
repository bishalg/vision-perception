package org.curv

import java.nio.ByteBuffer
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt

/**
 * Configuration thresholds for real-time palm capture guidance on Android.
 * Kept strictly synchronized with iOS PalmCaptureGuidance.swift.
 */
data class PalmGuidanceThresholds(
    val minPalmScale: Float = 0.35f,           // Palm length / frame height
    val maxPalmScale: Float = 0.60f,
    val maxCenterOffset: Float = 0.18f,         // Max distance from frame center (0.5, 0.5)
    val maxRollAngleDeg: Float = 22.0f,         // Max roll tilt relative to vertical
    val minFlatnessRatio: Float = 0.70f,        // Width / height ratio proxy for planar tilt
    val minLuminance: Float = 70.0f,           // Minimum average skin luminance (0..255)
    val maxGlareRatio: Float = 0.04f,          // Max percentage of specular glare pixels (> 245)
    val maxMotionDisplacement: Float = 0.025f,  // Max EMA landmark drift across frames
    val steadyDurationSec: Float = 0.30f        // Time in ready state before auto-trigger
)

/**
 * Real-time guidance states for rendering Liquid Glass HUD cues.
 */
sealed class PalmGuidanceState(val message: String) {
    class Searching(msg: String = "Place hand inside the outline") : PalmGuidanceState(msg)
    class CenterHand(msg: String = "Center your palm in the guide") : PalmGuidanceState(msg)
    class MoveCloser(msg: String = "Move closer to the camera") : PalmGuidanceState(msg)
    class MoveBack(msg: String = "Move slightly further back") : PalmGuidanceState(msg)
    class HoldFlat(msg: String = "Hold palm flat facing the camera") : PalmGuidanceState(msg)
    class TooDark(msg: String = "Too dark — increase lighting") : PalmGuidanceState(msg)
    class GlareDetected(msg: String = "Glare on skin — angle away from light") : PalmGuidanceState(msg)
    class HoldSteady(msg: String = "Hold steady...") : PalmGuidanceState(msg)
    data class Ready(val countdownProgress: Float, val msg: String = if (countdownProgress >= 1.0f) "Capturing..." else "Hold steady...") : PalmGuidanceState(msg)

    val isReady: Boolean get() = this is Ready
}

/**
 * Diagnostic metrics emitted every frame for HUD telemetry.
 */
data class PalmQualityMetrics(
    var scale: Float = 0.0f,
    var centerOffset: Float = 0.0f,
    var rollAngleDeg: Float = 0.0f,
    var flatnessRatio: Float = 0.0f,
    var meanLuminance: Float = 0.0f,
    var glareRatio: Float = 0.0f,
    var motionDisplacement: Float = 0.0f,

    var isCenteringOk: Boolean = false,
    var isScaleOk: Boolean = false,
    var isAngleOk: Boolean = false,
    var isIlluminationOk: Boolean = false,
    var isSteadinessOk: Boolean = false,
    var isReadyForCapture: Boolean = false
)

/**
 * High-performance 30+ FPS real-time capture guidance evaluator for Android.
 */
class PalmCaptureGuidance(
    val thresholds: PalmGuidanceThresholds = PalmGuidanceThresholds()
) {
    private var previousLandmarks = FloatArray(42)
    private var hasPrevious = false
    private var steadyStartTimeSec: Double? = null

    fun reset() {
        hasPrevious = false
        steadyStartTimeSec = null
    }

    /**
     * Evaluates a frame using 21 normalized landmarks (42 floats: x0, y0, x1, y1, ...).
     */
    fun evaluate(
        landmarks42: FloatArray,
        yBuffer: ByteBuffer? = null,
        width: Int = 0,
        height: Int = 0,
        timestampSec: Double = System.nanoTime() / 1_000_000_000.0
    ): Pair<PalmGuidanceState, PalmQualityMetrics> {
        val metrics = PalmQualityMetrics()

        if (landmarks42.size < 42) {
            steadyStartTimeSec = null
            hasPrevious = false
            return Pair(PalmGuidanceState.Searching(), metrics)
        }

        // Key anatomical landmarks: Wrist (0), Index MCP (5), Middle MCP (9), Pinky MCP (17)
        val wristX = landmarks42[0]
        val wristY = landmarks42[1]
        val indexMcpX = landmarks42[10]
        val indexMcpY = landmarks42[11]
        val middleMcpX = landmarks42[18]
        val middleMcpY = landmarks42[19]
        val pinkyMcpX = landmarks42[34]
        val pinkyMcpY = landmarks42[35]

        // 1. Centering Check
        val palmCenterX = (wristX + middleMcpX) * 0.5f
        val palmCenterY = (wristY + middleMcpY) * 0.5f
        val centerDx = palmCenterX - 0.5f
        val centerDy = palmCenterY - 0.5f
        metrics.centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)
        metrics.isCenteringOk = metrics.centerOffset <= thresholds.maxCenterOffset

        // 2. Scale / Distance Check
        val lengthDx = middleMcpX - wristX
        val lengthDy = middleMcpY - wristY
        metrics.scale = sqrt(lengthDx * lengthDx + lengthDy * lengthDy)
        metrics.isScaleOk = (metrics.scale >= thresholds.minPalmScale && metrics.scale <= thresholds.maxPalmScale)

        // 3. Angle & Planar Flatness Check
        val rollRad = atan2(lengthDx.toDouble(), -lengthDy.toDouble())
        metrics.rollAngleDeg = abs(rollRad * 180.0 / Math.PI).toFloat()

        val widthDx = pinkyMcpX - indexMcpX
        val widthDy = pinkyMcpY - indexMcpY
        val palmWidth = sqrt(widthDx * widthDx + widthDy * widthDy)
        metrics.flatnessRatio = if (metrics.scale > 0.001f) (palmWidth / (metrics.scale * 0.75f)) else 1.0f

        metrics.isAngleOk = (metrics.rollAngleDeg <= thresholds.maxRollAngleDeg) &&
                            (metrics.flatnessRatio >= thresholds.minFlatnessRatio)

        // 4. Illumination & Glare Check (from CameraX Y plane)
        if (yBuffer != null && width > 0 && height > 0) {
            val (lum, glare) = evaluatePhotometrics(yBuffer, width, height, palmCenterX, palmCenterY, metrics.scale * 0.3f)
            metrics.meanLuminance = lum
            metrics.glareRatio = glare
        } else {
            metrics.meanLuminance = 128.0f
            metrics.glareRatio = 0.0f
        }
        metrics.isIlluminationOk = (metrics.meanLuminance >= thresholds.minLuminance) &&
                                  (metrics.glareRatio <= thresholds.maxGlareRatio)

        // 5. Motion Steadiness Check
        if (hasPrevious) {
            var sumDist = 0.0f
            for (i in 0 until 21) {
                val dx = landmarks42[2 * i] - previousLandmarks[2 * i]
                val dy = landmarks42[2 * i + 1] - previousLandmarks[2 * i + 1]
                sumDist += sqrt(dx * dx + dy * dy)
            }
            metrics.motionDisplacement = sumDist / 21.0f
        } else {
            metrics.motionDisplacement = 0.0f
            hasPrevious = true
        }
        System.arraycopy(landmarks42, 0, previousLandmarks, 0, 42)
        metrics.isSteadinessOk = metrics.motionDisplacement <= thresholds.maxMotionDisplacement

        // Prioritized state transitions
        if (!metrics.isCenteringOk) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.CenterHand(), metrics)
        }
        if (metrics.scale < thresholds.minPalmScale) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.MoveCloser(), metrics)
        }
        if (metrics.scale > thresholds.maxPalmScale) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.MoveBack(), metrics)
        }
        if (!metrics.isAngleOk) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.HoldFlat(), metrics)
        }
        if (metrics.meanLuminance < thresholds.minLuminance) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.TooDark(), metrics)
        }
        if (metrics.glareRatio > thresholds.maxGlareRatio) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.GlareDetected(), metrics)
        }
        if (!metrics.isSteadinessOk) {
            steadyStartTimeSec = null
            return Pair(PalmGuidanceState.HoldSteady(), metrics)
        }

        // All 5 gates PASS: Start or advance steady countdown
        val start = steadyStartTimeSec
        return if (start != null) {
            val elapsed = (timestampSec - start).toFloat()
            val progress = min(1.0f, elapsed / thresholds.steadyDurationSec)
            metrics.isReadyForCapture = (progress >= 1.0f)
            Pair(PalmGuidanceState.Ready(progress), metrics)
        } else {
            steadyStartTimeSec = timestampSec
            Pair(PalmGuidanceState.Ready(0.0f), metrics)
        }
    }

    /**
     * Fast sub-sampled photometric evaluation on CameraX Y-plane ByteBuffer.
     */
    private fun evaluatePhotometrics(
        yBuffer: ByteBuffer,
        width: Int,
        height: Int,
        centerX: Float,
        centerY: Float,
        radius: Float
    ): Pair<Float, Float> {
        val cx = (centerX * width).toInt()
        val cy = (centerY * height).toInt()
        val rPx = (radius * min(width, height)).toInt()

        val x0 = max(0, cx - rPx)
        val x1 = min(width - 1, cx + rPx)
        val y0 = max(0, cy - rPx)
        val y1 = min(height - 1, cy + rPx)

        if (x1 <= x0 || y1 <= y0) return Pair(128.0f, 0.0f)

        var totalLum = 0.0f
        var glareCount = 0
        var sampleCount = 0
        val step = 4

        for (y in y0..y1 step step) {
            val rowOffset = y * width
            for (x in x0..x1 step step) {
                val idx = rowOffset + x
                if (idx < yBuffer.limit()) {
                    val yVal = (yBuffer.get(idx).toInt() and 0xFF).toFloat()
                    totalLum += yVal
                    if (yVal > 245.0f) glareCount++
                    sampleCount++
                }
            }
        }

        if (sampleCount == 0) return Pair(128.0f, 0.0f)
        val avgLum = totalLum / sampleCount
        val glareRatio = glareCount.toFloat() / sampleCount
        return Pair(avgLum, glareRatio)
    }
}
