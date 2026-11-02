package org.curv.asd.spike

import android.os.Process
import android.os.SystemClock
import org.curv.asd.core.GazeBenchCondition
import org.curv.asd.core.GazeOrientation
import org.curv.asd.core.GazeRawLogHeaderInput
import org.curv.asd.core.GazeScheduledTarget
import org.curv.asd.core.RawSampleRecord
import org.curv.asd.core.gazeRawLogDocument
import org.curv.asd.core.gazeScheduledTargetsJson
import org.curv.asd.spike.backend.FrameSink
import org.curv.asd.spike.backend.Observation
import org.json.JSONObject
import java.io.File
import java.io.Writer

/**
 * Collects one run and writes it.
 *
 * - `<base>.jsonl`: the `asd-raw-tracking/1` log, encoded by Rust (`gazeRawLogDocument`, the
 *   same encoder the iOS bench uses), so `asd-cli gaze replay --check` accepts it.
 * - `<base>.targets.json`: the schedule (`gazeScheduledTargetsJson`), as on iOS.
 * - `<base>.landmarks.jsonl`: what the tracker returned per frame (sidecar; see README).
 * - `<base>.meta.json`: device, display, camera, clock and live-stat facts for the sheet.
 *
 * The raw-log samples carry only what the trackers give directly: `t_sec`, `face_tracked`,
 * ARKit-named eye-blink blend shapes (MediaPipe), orientation and clip time. `face_transform`,
 * `look_at` and the eye transforms are left absent: producing them from landmarks is gaze
 * maths that has no Rust function yet (binding gap, README.md).
 */
class SpikeRecorder(
    private val clock: FrameClock,
    private val sidecar: Writer,
) : FrameSink {

    private val lock = Any()
    private val records = ArrayList<RawSampleRecord>(8192)
    private val targets = ArrayList<GazeScheduledTarget>()
    private val recentFrames = ArrayDeque<Long>()

    @Volatile var recording = false
        private set
    @Volatile private var clipStartSec = Double.NaN
    @Volatile private var clipEndSec = Double.NaN

    // Live stats (positioning and run are counted separately).
    private var framesAll = 0L
    private var detectedAll = 0L
    private var framesRun = 0L
    private var detectedRun = 0L
    private var faceRun = 0L
    private var lastFrameNs = Long.MIN_VALUE
    private var firstRunFrameNs = Long.MIN_VALUE
    private var lastRunFrameNs = Long.MIN_VALUE
    private var cpuAtStartMs = 0L
    private var wallAtStartMs = 0L

    override fun onFrame(observation: Observation) {
        clock.onFrame(observation.frameTimeNs)
        synchronized(lock) {
            if (observation.frameTimeNs <= lastFrameNs) return
            lastFrameNs = observation.frameTimeNs
            framesAll++
            if (observation.detected) detectedAll++
            recentFrames.addLast(observation.frameTimeNs)
            while (recentFrames.isNotEmpty() && observation.frameTimeNs - recentFrames.first() > 2_000_000_000L) {
                recentFrames.removeFirst()
            }
            if (!recording) return
            framesRun++
            if (observation.detected) detectedRun++
            if (observation.faceTracked) faceRun++
            if (firstRunFrameNs == Long.MIN_VALUE) firstRunFrameNs = observation.frameTimeNs
            lastRunFrameNs = observation.frameTimeNs

            val tSec = observation.frameTimeNs / 1e9
            val inClip = tSec >= clipStartSec && !(tSec >= clipEndSec)
            records += RawSampleRecord(
                tSec = tSec,
                faceTracked = observation.faceTracked,
                faceTransform = null,
                lookAt = null,
                leftEyeTransform = null,
                rightEyeTransform = null,
                eyeBlinkLeft = observation.eyeBlinkLeft,
                eyeBlinkRight = observation.eyeBlinkRight,
                ambientIntensity = null,
                ambientColorTemp = null,
                lightDirection = null,
                orientation = GazeOrientation.PORTRAIT,
                clipMediaTimeSec = if (inClip) tSec - clipStartSec else null,
                stimulusId = if (inClip) FREE_VIEW_STIMULUS else null,
            )
            observation.sidecarJson?.let {
                sidecar.write(it)
                sidecar.write("\n")
            }
        }
    }

    fun startRecording() {
        synchronized(lock) {
            recording = true
            cpuAtStartMs = Process.getElapsedCpuTime()
            wallAtStartMs = SystemClock.elapsedRealtime()
        }
    }

    fun stopRecording() {
        synchronized(lock) { recording = false }
    }

    fun addTarget(target: GazeScheduledTarget) {
        synchronized(lock) { targets += target }
    }

    fun beginClip(startSec: Double) { clipStartSec = startSec }
    fun endClip(endSec: Double) { clipEndSec = endSec }

    /** Frames per second over the last 2 s of delivered tracker results. */
    fun liveFps(): Double = synchronized(lock) {
        if (recentFrames.size < 2) 0.0
        else (recentFrames.size - 1) / ((recentFrames.last() - recentFrames.first()) / 1e9)
    }

    /** Detected-frame ratio: of the run once recording, else since the camera started. */
    fun liveDetectedRatio(): Double = synchronized(lock) {
        val (d, n) = if (recording || framesRun > 0) detectedRun to framesRun else detectedAll to framesAll
        if (n == 0L) 0.0 else d.toDouble() / n
    }

    fun runStats(): JSONObject = synchronized(lock) {
        val durationSec = if (framesRun > 1) (lastRunFrameNs - firstRunFrameNs) / 1e9 else 0.0
        val wallMs = SystemClock.elapsedRealtime() - wallAtStartMs
        val cpuMs = Process.getElapsedCpuTime() - cpuAtStartMs
        JSONObject()
            .put("frames", framesRun)
            .put("faceTrackedFrames", faceRun)
            .put("detectedFrames", detectedRun)
            .put("detectedRatio", if (framesRun == 0L) JSONObject.NULL else detectedRun.toDouble() / framesRun)
            .put("durationSec", durationSec)
            .put("meanFps", if (durationSec > 0) (framesRun - 1) / durationSec else JSONObject.NULL)
            // Process CPU time over wall time; 1.0 = one core fully busy.
            .put("processCpuCores", if (wallMs > 0) cpuMs.toDouble() / wallMs else JSONObject.NULL)
    }

    /** Writes the raw log and the targets file; returns (samples written, samples dropped). */
    fun writeRawLog(
        base: File,
        modelIdentifier: String,
        appVersion: String,
        condition: GazeBenchCondition,
    ): Pair<Long, Long> {
        val (recs, tgts) = synchronized(lock) { records.toList() to targets.toList() }
        val header = GazeRawLogHeaderInput(
            modelIdentifier = modelIdentifier,
            orientation = GazeOrientation.PORTRAIT,
            targets = tgts,
            appVersion = appVersion,
            condition = condition,
        )
        val doc = gazeRawLogDocument(header, recs)
        File(base.path + ".jsonl").writeText(doc.jsonl)
        File(base.path + ".targets.json").writeText(gazeScheduledTargetsJson(tgts))
        return doc.sampleCount.toLong() to doc.droppedSampleCount.toLong()
    }

    companion object {
        const val FREE_VIEW_STIMULUS = "spike-free-view"
    }
}
