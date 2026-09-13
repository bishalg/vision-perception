package org.curv.overlay.camera

import org.json.JSONObject

/**
 * Pinhole camera intrinsics for captured stills (orbit SfM, mosaic warping).
 */
data class CameraIntrinsics(
    val focalLengthX: Float,
    val focalLengthY: Float,
    val principalPointX: Float,
    val principalPointY: Float,
    val imageWidth: Int,
    val imageHeight: Int,
    val fieldOfViewDeg: Float? = null
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("fx", focalLengthX.toDouble())
        put("fy", focalLengthY.toDouble())
        put("cx", principalPointX.toDouble())
        put("cy", principalPointY.toDouble())
        put("image_width", imageWidth)
        put("image_height", imageHeight)
        fieldOfViewDeg?.let { put("fov_horizontal_deg", it.toDouble()) }
    }

    companion object {
        fun fromFieldOfView(imageWidth: Int, imageHeight: Int, fovHorizontalDeg: Float): CameraIntrinsics {
            val w = imageWidth.coerceAtLeast(1).toFloat()
            val fx = w / (2f * kotlin.math.tan(Math.toRadians(fovHorizontalDeg.toDouble() / 2.0)).toFloat())
            return CameraIntrinsics(
                focalLengthX = fx,
                focalLengthY = fx,
                principalPointX = w * 0.5f,
                principalPointY = imageHeight.coerceAtLeast(1) * 0.5f,
                imageWidth = imageWidth,
                imageHeight = imageHeight,
                fieldOfViewDeg = fovHorizontalDeg
            )
        }
    }
}

data class CapturedPhoto(
    val buffer: java.nio.ByteBuffer,
    val width: Int,
    val height: Int,
    val stride: Int,
    val intrinsics: CameraIntrinsics
)
