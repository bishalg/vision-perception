package org.curv.overlay.guidance

data class QualityMetrics(
    var scale: Float = 0f,
    var centerOffset: Float = 0f,
    var rollAngleDeg: Float = 0f,
    var flatnessRatio: Float = 0f,
    var meanLuminance: Float = 0f,
    var glareRatio: Float = 0f,
    var motionDisplacement: Float = 0f,
    var isCenteringOk: Boolean = false,
    var isScaleOk: Boolean = false,
    var isAngleOk: Boolean = false,
    var isIlluminationOk: Boolean = false,
    var isSteadinessOk: Boolean = false,
    var isReadyForCapture: Boolean = false
)

data class PhotometricSample(
    val meanLuminance: Float,
    val glareRatio: Float
) {
    companion object {
        val unknown = PhotometricSample(meanLuminance = 128f, glareRatio = 0f)
    }
}
