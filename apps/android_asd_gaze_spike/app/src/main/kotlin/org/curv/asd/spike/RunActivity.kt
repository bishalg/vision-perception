package org.curv.asd.spike

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.Button
import android.widget.FrameLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.curv.asd.core.GazeBenchCondition
import org.curv.asd.core.GazeScheduledTarget
import org.curv.asd.core.coreVersion
import org.curv.asd.core.gazeBenchConditionName
import org.curv.asd.spike.backend.ArCoreBackend
import org.curv.asd.spike.backend.CaptureBackend
import org.curv.asd.spike.backend.MediaPipeBackend
import org.json.JSONObject
import java.io.BufferedWriter
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * One spike run: positioning (face check, small preview for MediaPipe) -> 2 s lead-in ->
 * 9-point calibration -> 13-point validation grid -> 60 s free viewing -> files written.
 * Fixed timing, no gaze feedback (BEN-2). Live frame rate and detected-frame ratio at the top.
 * Triple-tap cancels and discards the run.
 */
class RunActivity : ComponentActivity() {

    private lateinit var root: FrameLayout
    private lateinit var backendLayer: FrameLayout
    private lateinit var targetView: TargetView
    private lateinit var freeView: FreeViewView
    private lateinit var stats: TextView
    private lateinit var message: TextView
    private lateinit var startButton: Button

    private lateinit var backend: CaptureBackend
    private lateinit var recorder: SpikeRecorder
    private lateinit var sidecarWriter: BufferedWriter
    private lateinit var base: File
    private lateinit var condition: GazeBenchCondition
    private lateinit var variant: String
    private val clock = FrameClock()
    private var participant = 0
    private var distanceCm = 0
    private var runJob: Job? = null
    private var finished = false
    private var failed = false
    private var thermalAtStart: JSONObject? = null

    private val statsTicker = object : Runnable {
        override fun run() {
            if (::recorder.isInitialized) {
                val fps = recorder.liveFps()
                val ratio = recorder.liveDetectedRatio()
                stats.text = String.format(
                    Locale.ROOT, "%s  %.1f fps  detected %.0f%%%s",
                    backend.id, fps, ratio * 100, if (recorder.recording) "  REC" else "",
                )
            }
            stats.postDelayed(this, 250)
        }
    }

    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) startBackend() else fail("Camera permission denied.")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        if (Build.VERSION.SDK_INT >= 28) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }

        val backendId = intent.getStringExtra(EXTRA_BACKEND) ?: "mediapipe"
        val gpu = intent.getBooleanExtra(EXTRA_GPU, false)
        condition = GazeBenchCondition.valueOf(intent.getStringExtra(EXTRA_CONDITION) ?: "BASELINE")
        participant = intent.getIntExtra(EXTRA_PARTICIPANT, 0)
        distanceCm = intent.getIntExtra(EXTRA_DISTANCE_CM, 50)
        backend = if (backendId == "arcore") ArCoreBackend() else MediaPipeBackend(gpu)

        buildUi()

        val stamp = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.ROOT).format(Date())
        variant = if (backend.id == "mediapipe") (if (gpu) "mediapipe-gpu" else "mediapipe-cpu") else backend.id
        val name = "spike-$variant-${gazeBenchConditionName(condition)}-p%02d-${distanceCm}cm-$stamp".format(participant)
        base = File(DeviceFacts.logDir(this), name)
        sidecarWriter = File(base.path + ".landmarks.jsonl.part").bufferedWriter()
        sidecarWriter.write(sidecarHeader())
        sidecarWriter.write("\n")
        recorder = SpikeRecorder(clock, sidecarWriter)

        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            startBackend()
        } else {
            permission.launch(Manifest.permission.CAMERA)
        }
        stats.post(statsTicker)
    }

    private fun sidecarHeader(): String = JSONObject()
        .put("format", "android-gaze-spike-landmarks/1")
        .put("backend", variant)
        .put("note", "Raw tracker output per frame, uninterpreted; t_sec is on the same clock as the raw log. Biometric data: never commit.")
        .toString()

    private fun buildUi() {
        root = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        backendLayer = FrameLayout(this)
        targetView = TargetView(this)
        freeView = FreeViewView(this)
        stats = TextView(this).apply {
            setTextColor(0x99FFFFFF.toInt())
            textSize = 12f
            gravity = Gravity.CENTER_HORIZONTAL
        }
        message = TextView(this).apply {
            setTextColor(Color.WHITE)
            textSize = 17f
            gravity = Gravity.CENTER
            setPadding(48, 48, 48, 48)
        }
        startButton = Button(this).apply {
            text = "Start run"
            setOnClickListener { beginRun() }
        }
        val match = FrameLayout.LayoutParams.MATCH_PARENT
        root.addView(backendLayer, FrameLayout.LayoutParams(match, match))
        root.addView(freeView, FrameLayout.LayoutParams(match, match))
        root.addView(targetView, FrameLayout.LayoutParams(match, match))
        root.addView(stats, FrameLayout.LayoutParams(match, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.TOP).apply {
            topMargin = (6 * resources.displayMetrics.density).toInt()
        })
        root.addView(message, FrameLayout.LayoutParams(match, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.TOP).apply {
            topMargin = (64 * resources.displayMetrics.density).toInt()
        })
        root.addView(startButton, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL,
        ).apply { bottomMargin = (72 * resources.displayMetrics.density).toInt() })
        setContentView(root)

        message.text = "Positioning\n\nVolunteer (adult) at ${distanceCm} cm from the screen, measured with a ruler " +
            "from the glass to the outer eye corner. Phone on a stand, portrait, screen vertical.\n\n" +
            "Wait until 'detected' stays near 100 %, then tap Start run. During the run the " +
            "volunteer looks at each dot, then watches the moving shapes for 60 s.\n\nTriple-tap to cancel."

        val detector = GestureDetector(this, object : GestureDetector.SimpleOnGestureListener() {})
        var taps = 0
        var lastTap = 0L
        root.setOnTouchListener { _, ev ->
            detector.onTouchEvent(ev)
            if (ev.actionMasked == MotionEvent.ACTION_UP) {
                val now = ev.eventTime
                taps = if (now - lastTap < 400) taps + 1 else 1
                lastTap = now
                if (taps >= 3) cancelRun()
            }
            true
        }
    }

    private fun startBackend() {
        backend.start(this, backendLayer, recorder) { msg -> fail(msg) }
    }

    private fun beginRun() {
        if (!clock.ready) {
            message.text = "No camera frames yet. Wait for the camera (or check the error above)."
            return
        }
        startButton.visibility = View.GONE
        message.visibility = View.GONE
        backend.hidePreview()
        backendLayer.visibility = if (backend.id == "arcore") View.VISIBLE else View.INVISIBLE
        val plan = try {
            RunPlan.load(DeviceFacts.modelIdentifier)
        } catch (e: Exception) {
            fail("Could not read the Rust gaze config: ${e.message}")
            return
        }
        thermalAtStart = DeviceFacts.thermal(this)
        recorder.startRecording()
        runJob = lifecycleScope.launch {
            delay((RunPlan.LEAD_IN_SEC * 1000).toLong())
            for (t in plan.targets) {
                if (!isActive) return@launch
                targetView.show(t.xNorm, t.yNorm)
                val start = clock.nowSec()
                delay((t.dwellSec * 1000).toLong())
                val end = clock.nowSec()
                targetView.clear()
                recorder.addTarget(GazeScheduledTarget(t.kind, t.xNorm, t.yNorm, start, end, t.label))
                delay((RunPlan.GAP_SEC * 1000).toLong())
            }
            freeView.start()
            recorder.beginClip(clock.nowSec())
            delay((RunPlan.FREE_VIEW_SEC * 1000).toLong())
            recorder.endClip(clock.nowSec())
            freeView.stop()
            delay(300)
            finishRun(plan)
        }
    }

    private suspend fun finishRun(plan: RunPlan) {
        recorder.stopRecording()
        backend.stop()
        val facts = backend.describe()
        val runStats = recorder.runStats()
        val thermalEnd = DeviceFacts.thermal(this)
        val result = withContext(Dispatchers.IO) {
            runCatching {
                sidecarWriter.close()
                File(base.path + ".landmarks.jsonl.part").renameTo(File(base.path + ".landmarks.jsonl"))
                val appVersion = "android_asd_gaze_spike/0.1 backend=$variant asd-core=${coreVersion()}"
                val (written, dropped) = recorder.writeRawLog(base, DeviceFacts.modelIdentifier, appVersion, condition)
                val meta = JSONObject()
                    .put("format", "android-gaze-spike-meta/1")
                    .put("rawLog", base.name + ".jsonl")
                    .put("appVersion", appVersion)
                    .put("asdCoreVersion", coreVersion())
                    .put("condition", gazeBenchConditionName(condition))
                    .put("participantNumber", participant)
                    .put("distanceCm", distanceCm)
                    .put("frameClock", clock.mode)
                    .put("samplesWritten", written)
                    .put("samplesDropped", dropped)
                    .put("live", runStats)
                    .put("thermalStart", thermalAtStart)
                    .put("thermalEnd", thermalEnd)
                    .put("device", DeviceFacts.device())
                    .put("display", DeviceFacts.display(this@RunActivity, root.width, root.height))
                    .put("capture", facts)
                    .put("rustGeometrySource", plan.config.opt("geometrySource"))
                    .put("bindingGap", "raw log has no face_transform/look_at/eye transforms; see README.md")
                File(base.path + ".meta.json").writeText(meta.toString(2))
                Triple(written, dropped, runStats)
            }
        }
        finished = true
        stats.removeCallbacks(statsTicker)
        stats.text = ""
        result.onSuccess { (written, dropped, s) ->
            message.visibility = View.VISIBLE
            message.text = String.format(
                Locale.ROOT,
                "Saved %s\n\n%d samples (%d dropped)\nmean %.1f fps, detected %.0f%%\nCPU %.2f cores\n\n" +
                    "adb pull /sdcard/Android/data/%s/files/spike_logs/\n\nTap Back to return.",
                base.name, written, dropped, s.optDouble("meanFps", 0.0),
                s.optDouble("detectedRatio", 0.0) * 100, s.optDouble("processCpuCores", 0.0), packageName,
            )
        }.onFailure { fail("Could not write the log: ${it.message}") }
    }

    private fun cancelRun() {
        if (finished) return
        runJob?.cancel()
        discard()
        finish()
    }

    private fun discard() {
        if (::recorder.isInitialized) recorder.stopRecording()
        backend.stop()
        runCatching { sidecarWriter.close() }
        File(base.path + ".landmarks.jsonl.part").delete()
    }

    private fun fail(msg: String) {
        if (failed) return
        failed = true
        runJob?.cancel()
        stats.removeCallbacks(statsTicker)
        startButton.visibility = View.GONE
        message.visibility = View.VISIBLE
        message.setTextColor(0xFFFF8A80.toInt())
        message.text = "$msg\n\nNothing was saved. Tap Back."
        if (!finished) discard()
        finished = true
    }

    override fun onResume() {
        super.onResume()
        if (!finished) backend.onResume()
    }

    override fun onPause() {
        super.onPause()
        // Leaving the screen mid-run breaks the fixed timing: discard rather than log a gap.
        if (runJob?.isActive == true) {
            runJob?.cancel()
            fail("Run interrupted (app left the screen).")
        } else if (!finished) {
            backend.onPause()
        }
    }

    override fun onDestroy() {
        stats.removeCallbacks(statsTicker)
        if (!finished) discard()
        super.onDestroy()
    }

    companion object {
        const val EXTRA_BACKEND = "backend"
        const val EXTRA_GPU = "gpu"
        const val EXTRA_CONDITION = "condition"
        const val EXTRA_PARTICIPANT = "participant"
        const val EXTRA_DISTANCE_CM = "distanceCm"

        fun intent(context: Context, backend: String, gpu: Boolean, condition: GazeBenchCondition, participant: Int, distanceCm: Int) =
            Intent(context, RunActivity::class.java)
                .putExtra(EXTRA_BACKEND, backend)
                .putExtra(EXTRA_GPU, gpu)
                .putExtra(EXTRA_CONDITION, condition.name)
                .putExtra(EXTRA_PARTICIPANT, participant)
                .putExtra(EXTRA_DISTANCE_CM, distanceCm)
    }
}
