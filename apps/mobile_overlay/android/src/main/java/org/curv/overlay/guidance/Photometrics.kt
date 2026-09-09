package org.curv.overlay.guidance

import java.nio.ByteBuffer
import kotlin.math.max
import kotlin.math.min

/**
 * Fast sub-sampled luminance + specular glare probe on a CameraX Y plane.
 */
object Photometrics {
    fun sample(
        yBuffer: ByteBuffer,
        width: Int,
        height: Int,
        rowStride: Int,
        centerX: Float,
        centerY: Float,
        radius: Float
    ): PhotometricSample {
        val cx = (centerX * width).toInt()
        val cy = (centerY * height).toInt()
        val rPx = (radius * min(width, height)).toInt()

        val x0 = max(0, cx - rPx)
        val x1 = min(width - 1, cx + rPx)
        val y0 = max(0, cy - rPx)
        val y1 = min(height - 1, cy + rPx)
        if (x1 <= x0 || y1 <= y0) return PhotometricSample.unknown

        var totalLum = 0f
        var glareCount = 0
        var sampleCount = 0
        val step = 4
        val stride = if (rowStride > 0) rowStride else width

        for (y in y0..y1 step step) {
            val rowOffset = y * stride
            for (x in x0..x1 step step) {
                val idx = rowOffset + x
                if (idx < yBuffer.limit()) {
                    val yVal = (yBuffer.get(idx).toInt() and 0xFF).toFloat()
                    totalLum += yVal
                    if (yVal > 245f) glareCount++
                    sampleCount++
                }
            }
        }

        if (sampleCount == 0) return PhotometricSample.unknown
        return PhotometricSample(
            meanLuminance = totalLum / sampleCount,
            glareRatio = glareCount.toFloat() / sampleCount
        )
    }
}
