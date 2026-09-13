package org.curv.overlay.camera

import android.content.Context
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.util.Log

/**
 * Runtime probe for hardware depth capture on the back camera.
 *
 * Strictly opportunistic, mirroring the iOS DepthCapability contract:
 * every field gates a progressive enhancement and the orbit pipeline
 * falls back to the parallax/metric estimator whenever depth is
 * unavailable.
 *
 * Current scope is the probe + ready data-model fields only. CameraX
 * stills carry no depth output, so measured depth lights up only once a
 * Camera2 `DEPTH16`/`DEPTH_POINT_CLOUD` still pipeline lands; until then
 * Android stays on the metric path with zero behavior change.
 */
data class DepthCapability(
    val depthOutputSupported: Boolean = false
) {
    val supported: Boolean get() = depthOutputSupported

    /** Diagnostic source string for `OrbitFrame.depthSource`. */
    val depthSource: String get() = "tof_camera2_v1"

    companion object {
        private const val TAG = "DepthCapability"

        /** Safe on emulators and devices without cameras (all false). */
        fun probe(context: Context): DepthCapability {
            return try {
                val manager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
                var found = false
                for (id in manager.cameraIdList) {
                    val chars = manager.getCameraCharacteristics(id)
                    if (chars.get(CameraCharacteristics.LENS_FACING) != CameraCharacteristics.LENS_FACING_BACK) {
                        continue
                    }
                    val caps = chars.get(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES)
                        ?: continue
                    if (caps.contains(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES_DEPTH_OUTPUT)) {
                        found = true
                        break
                    }
                }
                DepthCapability(depthOutputSupported = found)
            } catch (e: Exception) {
                Log.w(TAG, "Depth probe failed; using fallback path", e)
                DepthCapability()
            }
        }
    }
}
