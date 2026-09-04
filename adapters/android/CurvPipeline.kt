package org.curv

import java.nio.ByteBuffer

/**
 * High-performance Android Kotlin wrapper for CurvEngine native perception core.
 * Uses zero-copy ByteBuffer ingestion directly from CameraX ImageProxy planes.
 */
class CurvPipeline(domainId: String = "palm") : AutoCloseable {

    private var nativeHandle: Long = nativeCreate(domainId)

    companion object {
        init {
            System.loadLibrary("curv_capi")
        }

        const val FORMAT_GRAYSCALE = 1
        const val FORMAT_RGB = 2
        const val FORMAT_RGBA = 3
        const val FORMAT_BGR = 4
        const val FORMAT_BGRA = 5
    }

    /**
     * Processes a direct ByteBuffer camera frame with 21 normalized landmarks.
     * @param buffer Direct ByteBuffer containing pixel data.
     * @param width Frame width in pixels.
     * @param height Frame height in pixels.
     * @param stride Row stride in bytes (pass 0 for tightly packed).
     * @param format Pixel format enum (e.g. FORMAT_RGBA).
     * @param landmarks Array of 42 floats representing 21 normalized (x, y) coordinates.
     * @return Formatted Perception Evidence JSON string.
     */
    fun processLandmarksRoi(
        buffer: ByteBuffer,
        width: Int,
        height: Int,
        stride: Int = 0,
        format: Int = FORMAT_RGBA,
        landmarks: FloatArray
    ): String {
        check(nativeHandle != 0L) { "CurvPipeline has been closed" }
        require(buffer.isDirect) { "Buffer must be allocated as a direct ByteBuffer" }
        require(landmarks.size >= 42) { "Landmarks array must contain at least 42 floats (21 points)" }

        return nativeProcessLandmarksRoi(
            nativeHandle,
            buffer,
            width,
            height,
            stride,
            format,
            landmarks
        )
    }

    override fun close() {
        if (nativeHandle != 0L) {
            nativeDestroy(nativeHandle)
            nativeHandle = 0L
        }
    }

    protected fun finalize() {
        close()
    }

    private external fun nativeCreate(domainId: String?): Long
    private external fun nativeDestroy(handle: Long)
    private external fun nativeProcessLandmarksRoi(
        handle: Long,
        buffer: ByteBuffer,
        width: Int,
        height: Int,
        stride: Int,
        format: Int,
        landmarks: FloatArray
    ): String
}
