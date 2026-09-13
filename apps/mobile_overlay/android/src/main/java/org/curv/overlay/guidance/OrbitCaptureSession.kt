package org.curv.overlay.guidance

import org.curv.overlay.camera.CameraIntrinsics
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

enum class OrbitBin(val key: String, val displayName: String) {
    FLAT_CENTER("flat_center", "Hold palm flat, centered"),
    TILT_LEFT("tilt_left", "Tilt hand left ~20°"),
    TILT_RIGHT("tilt_right", "Tilt hand right ~20°"),
    THUMB_TOWARD("thumb_toward", "Rotate thumb toward camera"),
    THUMB_AWAY("thumb_away", "Rotate thumb away from camera"),
    TILT_FORWARD("tilt_forward", "Tilt fingers toward camera");

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
    val maxJointDrift: Float
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
    }
}

class OrbitCaptureSession(val handSide: String) {
    val schema = "curv_orbit_session_v1"
    val startedAt: Double = System.currentTimeMillis() / 1000.0
    var completedAt: Double? = null
    val frames = mutableListOf<OrbitFrame>()
    var referenceLandmarks: List<List<Float>>? = null

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
    }
}
