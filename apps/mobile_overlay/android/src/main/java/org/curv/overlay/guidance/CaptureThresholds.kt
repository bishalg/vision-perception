package org.curv.overlay.guidance

data class CaptureThresholds(
    val requireDetectedSubject: Boolean = true,
    val minScale: Float = 0.35f,
    val maxScale: Float = 0.60f,
    val maxCenterOffset: Float = 0.18f,
    val maxRollAngleDeg: Float = 22.0f,
    val minFlatnessRatio: Float = 0.70f,
    val minLuminance: Float = 70.0f,
    val maxGlareRatio: Float = 0.04f,
    val maxMotionDisplacement: Float = 0.025f,
    val steadyDurationSec: Float = 0.30f,
    val skipGeometryGates: Boolean = false
)
