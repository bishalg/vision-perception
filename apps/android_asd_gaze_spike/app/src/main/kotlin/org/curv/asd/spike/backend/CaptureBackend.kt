package org.curv.asd.spike.backend

import android.app.Activity
import android.widget.FrameLayout
import org.json.JSONObject

/**
 * One capture back-end of the spike (AND-4). A back-end only captures: it reports what the
 * tracker returned per camera frame, as given, and never turns it into gaze. Gaze maths lives
 * in Rust (`asd-core::gaze`); see the binding-gap section of README.md.
 */
interface CaptureBackend {
    /** Short id used in file names and the log header (`mediapipe`, `arcore`). */
    val id: String

    /** Starts capture; [container] may receive a preview or GL view. Errors go to [onError]. */
    fun start(activity: Activity, container: FrameLayout, sink: FrameSink, onError: (String) -> Unit)

    /** Activity lifecycle hooks (ARCore needs them; CameraX follows the lifecycle itself). */
    fun onResume() {}
    fun onPause() {}

    /** Hides the positioning preview so the run itself costs no preview rendering. */
    fun hidePreview() {}

    fun stop()

    /** Camera and tracker facts for the run's `.meta.json` (intrinsics, resolution, clock). */
    fun describe(): JSONObject
}

/** What a back-end saw in one camera frame. */
class Observation(
    /** Camera frame timestamp in nanoseconds on the back-end's own clock. */
    val frameTimeNs: Long,
    /** A face was found / tracked in this frame. */
    val faceTracked: Boolean,
    /**
     * The frame carries everything this back-end can offer for gaze (MediaPipe: face with
     * the 10 iris landmarks; ARCore: a tracked face, which never includes eyes or iris).
     */
    val detected: Boolean,
    /** Blend shapes named and scaled like ARKit's (0 open, 1 closed), when the tracker has them. */
    val eyeBlinkLeft: Double?,
    val eyeBlinkRight: Double?,
    /** One JSON object (no newline) for the `.landmarks.jsonl` sidecar, or null. */
    val sidecarJson: String?,
)

fun interface FrameSink {
    fun onFrame(observation: Observation)
}

/** Eye-region landmark subset written to the sidecar (MediaPipe canonical face-mesh indices). */
object EyeLandmarks {
    /** Left/right eye contours, iris centres + rings (468..477, MediaPipe only), anchors. */
    val LEFT_EYE = intArrayOf(33, 7, 163, 144, 145, 153, 154, 155, 133, 173, 157, 158, 159, 160, 161, 246)
    val RIGHT_EYE = intArrayOf(263, 249, 390, 373, 374, 380, 381, 382, 362, 398, 384, 385, 386, 387, 388, 466)
    val IRIS = intArrayOf(468, 469, 470, 471, 472, 473, 474, 475, 476, 477)

    /** Nose tip, nose bridge, chin, forehead, face sides: head-pose anchors. */
    val ANCHORS = intArrayOf(1, 4, 6, 10, 152, 234, 454)

    val MESH_468: IntArray = (LEFT_EYE + RIGHT_EYE + ANCHORS)
    val WITH_IRIS: IntArray = MESH_468 + IRIS
}

/** Appends a number with at most 6 significant decimals (keeps the sidecar small). */
internal fun StringBuilder.num(v: Double): StringBuilder =
    if (v.isFinite()) append(String.format(java.util.Locale.ROOT, "%.6g", v)) else append("null")

internal fun StringBuilder.num(v: Float): StringBuilder = num(v.toDouble())

internal fun StringBuilder.floats(values: FloatArray): StringBuilder {
    append('[')
    values.forEachIndexed { i, f ->
        if (i > 0) append(',')
        num(f)
    }
    return append(']')
}
