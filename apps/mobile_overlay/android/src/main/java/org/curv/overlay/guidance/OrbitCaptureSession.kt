package org.curv.overlay.guidance

import org.curv.overlay.camera.CameraIntrinsics
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

enum class OrbitBin(
    val key: String,
    val displayName: String,
    val arrowSymbol: String,
    val shortLabel: String,
    val coachingTip: String
) {
    FLAT_CENTER("flat_center", "Hold palm flat, centered", "✋", "Flat", "Align palm flat to establish base 3D hand reference plane"),
    TILT_LEFT("tilt_left", "Tilt hand left ~20°", "⇦", "Tilt Left", "Tilt hand left ~20° to expose Thenar & Jupiter elevation contours"),
    TILT_RIGHT("tilt_right", "Tilt hand right ~20°", "⇨", "Tilt Right", "Tilt hand right ~20° to expose Hypothenar & Mercury relief"),
    THUMB_TOWARD("thumb_toward", "Rotate thumb toward camera", "↻", "Thumb In", "Rotate thumb slightly toward camera to map thenar muscle pad"),
    THUMB_AWAY("thumb_away", "Rotate thumb away from camera", "↺", "Thumb Out", "Rotate thumb slightly outward to capture hypothenar slope"),
    TILT_FORWARD("tilt_forward", "Tilt fingers toward camera", "⇧", "Fingers Up", "Tilt fingers forward to map interdigital mount summits");

    companion object {
        val all = entries.toList()
    }
}

data class OrbitFrame(
    val id: String = UUID.randomUUID().toString(),
    val bin: OrbitBin,
    val timestamp: Double,
    val landmarks: List<List<Float>>,
    val gravity: List<Float>,
    val deviceRollDeg: Float,
    val intrinsics: CameraIntrinsics,
    val imageWidth: Int,
    val imageHeight: Int,
    val digitSpreadOk: Boolean,
    val maxJointDrift: Float,
    /** Measured per-mount depth in mm from hardware depth. Null until wired. */
    val depthMm: Map<String, Float>? = null,
    val depthSource: String? = null
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("id", id)
        put("bin", bin.key)
        put("timestamp", timestamp)
        put("landmarks", JSONArray(landmarks.map { JSONArray(it) }))
        put("gravity", JSONArray(gravity))
        put("device_roll_deg", deviceRollDeg.toDouble())
        put("intrinsics", intrinsics.toJson())
        put("image_width", imageWidth)
        put("image_height", imageHeight)
        put("digit_spread_ok", digitSpreadOk)
        put("max_joint_drift", maxJointDrift.toDouble())
        depthMm?.let { put("depth_mm", JSONObject(it as Map<*, *>)) }
        depthSource?.let { put("depth_source", it) }
    }
}

class OrbitCaptureSession(val handSide: String) {
    val schema = "curv_orbit_session_v1"
    val startedAt: Double = System.currentTimeMillis() / 1000.0
    var completedAt: Double? = null
    val frames = mutableListOf<OrbitFrame>()
    var referenceLandmarks: List<List<Float>>? = null
    /** Device gravity at the flat reference view; set from the first frame. */
    var referenceGravity: List<Float>? = null
    /** Anatomical scale prior in mm (metacarpal breadth). Null = 80 mm default. */
    var metricPalmWidthMm: Float? = null
    var metricSource: String? = null

    fun filledBins(): Set<OrbitBin> = frames.map { it.bin }.toSet()

    fun coverageFraction(): Float = filledBins().size.toFloat() / OrbitBin.all.size

    fun toJson(): JSONObject = JSONObject().apply {
        put("schema", schema)
        put("hand_side", handSide)
        put("started_at", startedAt)
        completedAt?.let { put("completed_at", it) }
        put("frames", JSONArray(frames.map { it.toJson() }))
        referenceLandmarks?.let { ref ->
            put("reference_landmarks", JSONArray(ref.map { JSONArray(it) }))
        }
        referenceGravity?.let { put("reference_gravity", JSONArray(it)) }
        metricPalmWidthMm?.let { put("metric_palm_width_mm", it.toDouble()) }
        metricSource?.let { put("metric_source", it) }
    }
}
