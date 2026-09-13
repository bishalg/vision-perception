package org.curv.overlay.camera

import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.max
import kotlin.math.min

/**
 * Downsampled depth snapshot in meters, safe to pass across threads/coroutines.
 *
 * Conversion policy (kept in one place so capture code stays thin):
 * - Depth values are in meters, invalid samples (<= 0, NaN, infinite) are dropped (Float.NaN).
 * - Long edge is capped at [maxEdge] (default 256) by stride sampling.
 * - Supports direct float meters or Camera2 `DEPTH16` buffers (16-bit: lower 13 bits mm, upper 3 bits confidence).
 */
data class PhotoDepthMap(
    val width: Int,
    val height: Int,
    val meters: FloatArray,
    val source: String,
    val quality: Int
) {
    val isValid: Boolean
        get() = width > 0 && height > 0 && meters.size == width * height

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false
        other as PhotoDepthMap
        if (width != other.width) return false
        if (height != other.height) return false
        if (!meters.contentEquals(other.meters)) return false
        if (source != other.source) return false
        if (quality != other.quality) return false
        return true
    }

    override fun hashCode(): Int {
        var result = width
        result = 31 * result + height
        result = 31 * result + meters.contentHashCode()
        result = 31 * result + source.hashCode()
        result = 31 * result + quality
        return result
    }

    companion object {
        /**
         * Converts Camera2 DEPTH16 buffer to a downsampled PhotoDepthMap.
         * DEPTH16: 16-bit sample where lower 13 bits are depth in mm, upper 3 bits are confidence.
         */
        fun fromDepth16(
            buffer: ByteBuffer,
            width: Int,
            height: Int,
            stride: Int = width * 2,
            source: String = "tof_camera2_v1",
            maxEdge: Int = 256
        ): PhotoDepthMap? {
            if (width <= 0 || height <= 0 || buffer.remaining() < height * stride) return null
            val step = max(1, max(width, height) / max(1, maxEdge))
            val outW = (width + step - 1) / step
            val outH = (height + step - 1) / step
            val out = FloatArray(outW * outH)

            val shortBuffer = buffer.duplicate().order(ByteOrder.nativeOrder()).asShortBuffer()
            val rowStrideShorts = stride / 2

            var outIdx = 0
            for (y in 0 until height step step) {
                for (x in 0 until width step step) {
                    val raw = shortBuffer.get(y * rowStrideShorts + x).toInt()
                    val depthMm = (raw and 0x1FFF).toFloat()
                    val confidence = (raw ushr 13) and 0x7
                    // Samples with zero depth or zero confidence are invalid.
                    if (confidence > 0 && depthMm > 0f) {
                        val m = depthMm / 1000.0f
                        out[outIdx] = if (m in 0.05f..3.0f) m else Float.NaN
                    } else {
                        out[outIdx] = Float.NaN
                    }
                    outIdx++
                }
            }
            return PhotoDepthMap(width = outW, height = outH, meters = out, source = source, quality = 1)
        }
    }
}
