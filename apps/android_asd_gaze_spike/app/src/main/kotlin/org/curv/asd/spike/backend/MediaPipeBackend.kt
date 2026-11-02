package org.curv.asd.spike.backend

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Matrix
import android.hardware.camera2.CameraCharacteristics
import android.os.Build
import android.util.Size
import android.widget.FrameLayout
import androidx.camera.camera2.interop.Camera2CameraInfo
import androidx.camera.camera2.interop.ExperimentalCamera2Interop
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import androidx.lifecycle.LifecycleOwner
import com.google.mediapipe.framework.image.BitmapImageBuilder
import com.google.mediapipe.tasks.core.BaseOptions
import com.google.mediapipe.tasks.core.Delegate
import com.google.mediapipe.tasks.vision.core.RunningMode
import com.google.mediapipe.tasks.vision.facelandmarker.FaceLandmarker
import com.google.mediapipe.tasks.vision.facelandmarker.FaceLandmarkerResult
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * Back-end 1: CameraX front camera + MediaPipe Face Landmarker (478 landmarks with iris,
 * ARKit-named blend shapes, facial transformation matrix). Apache-2.0 library and model.
 *
 * Frames are rotated upright (the model expects an upright face) and **not** mirrored.
 * Landmarks are written as MediaPipe returns them: x, y normalised to the upright image,
 * z relative depth on MediaPipe's own scale.
 */
class MediaPipeBackend(private val useGpu: Boolean) : CaptureBackend {
    override val id = "mediapipe"

    private var landmarker: FaceLandmarker? = null
    private var provider: ProcessCameraProvider? = null
    private var preview: Preview? = null
    private var camera: Camera? = null
    private var analysisSize: Size? = null
    private var uprightSize: Size? = null
    private var rotationDegrees: Int? = null
    private val executor: ExecutorService = Executors.newSingleThreadExecutor()
    private val pendingNs = ConcurrentHashMap<Long, Long>()
    @Volatile private var lastMs = Long.MIN_VALUE
    @Volatile private var sink: FrameSink? = null
    @Volatile private var stopped = false

    override fun start(activity: Activity, container: FrameLayout, sink: FrameSink, onError: (String) -> Unit) {
        this.sink = sink
        try {
            val base = BaseOptions.builder()
                .setModelAssetPath("face_landmarker.task")
                .setDelegate(if (useGpu) Delegate.GPU else Delegate.CPU)
                .build()
            val options = FaceLandmarker.FaceLandmarkerOptions.builder()
                .setBaseOptions(base)
                .setRunningMode(RunningMode.LIVE_STREAM)
                .setNumFaces(1)
                .setMinFaceDetectionConfidence(0.5f)
                .setMinFacePresenceConfidence(0.5f)
                .setMinTrackingConfidence(0.5f)
                .setOutputFaceBlendshapes(true)
                .setOutputFacialTransformationMatrixes(true)
                .setResultListener { result, _ -> onResult(result) }
                .setErrorListener { e -> activity.runOnUiThread { onError("MediaPipe: ${e.message}") } }
                .build()
            landmarker = FaceLandmarker.createFromOptions(activity, options)
        } catch (e: Exception) {
            onError("MediaPipe Face Landmarker could not start: ${e.message}")
            return
        }

        val previewView = PreviewView(activity).apply {
            implementationMode = PreviewView.ImplementationMode.COMPATIBLE
            scaleType = PreviewView.ScaleType.FIT_CENTER
        }
        val size = (160 * activity.resources.displayMetrics.density).toInt()
        container.addView(
            previewView,
            FrameLayout.LayoutParams(size, (size * 4) / 3).apply {
                gravity = android.view.Gravity.BOTTOM or android.view.Gravity.CENTER_HORIZONTAL
                bottomMargin = (140 * activity.resources.displayMetrics.density).toInt()
            },
        )

        val future = ProcessCameraProvider.getInstance(activity)
        future.addListener({
            try {
                val p = future.get()
                provider = p
                val selector = ResolutionSelector.Builder()
                    .setResolutionStrategy(
                        ResolutionStrategy(Size(640, 480), ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER),
                    )
                    .build()
                val analysis = ImageAnalysis.Builder()
                    .setResolutionSelector(selector)
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_RGBA_8888)
                    .build()
                analysis.setAnalyzer(executor) { proxy -> analyze(proxy) }
                val pv = Preview.Builder().setResolutionSelector(selector).build()
                pv.setSurfaceProvider(previewView.surfaceProvider)
                preview = pv
                p.unbindAll()
                camera = p.bindToLifecycle(
                    activity as LifecycleOwner,
                    CameraSelector.DEFAULT_FRONT_CAMERA,
                    pv,
                    analysis,
                )
            } catch (e: Exception) {
                onError("Front camera could not start: ${e.message}")
            }
        }, ContextCompat.getMainExecutor(activity))
    }

    override fun hidePreview() {
        preview?.let { provider?.unbind(it) }
        preview = null
    }

    private fun analyze(proxy: ImageProxy) {
        val lm = landmarker
        if (lm == null || stopped) {
            proxy.close()
            return
        }
        val ns = proxy.imageInfo.timestamp
        val rotation = proxy.imageInfo.rotationDegrees
        val raw: Bitmap
        try {
            raw = proxy.toBitmap()
        } finally {
            proxy.close()
        }
        analysisSize = Size(raw.width, raw.height)
        rotationDegrees = rotation
        val upright = if (rotation == 0) raw else
            Bitmap.createBitmap(raw, 0, 0, raw.width, raw.height, Matrix().apply { postRotate(rotation.toFloat()) }, false)
        uprightSize = Size(upright.width, upright.height)
        // detectAsync needs strictly increasing millisecond timestamps.
        val ms = ns / 1_000_000L
        if (ms <= lastMs) return
        lastMs = ms
        pendingNs[ms] = ns
        try {
            lm.detectAsync(BitmapImageBuilder(upright).build(), ms)
        } catch (_: Exception) {
            pendingNs.remove(ms)
        }
    }

    private fun onResult(result: FaceLandmarkerResult) {
        val ms = result.timestampMs()
        val ns = pendingNs.remove(ms) ?: (ms * 1_000_000L)
        // Drop bookkeeping of frames the graph skipped.
        if (pendingNs.size > 64) pendingNs.keys.filter { it < ms }.forEach { pendingNs.remove(it) }
        val faces = result.faceLandmarks()
        if (faces.isEmpty()) {
            sink?.onFrame(Observation(ns, faceTracked = false, detected = false, null, null, sidecarLine(ns, null, result)))
            return
        }
        val landmarks = faces[0]
        val hasIris = landmarks.size >= 478
        var blinkL: Double? = null
        var blinkR: Double? = null
        if (result.faceBlendshapes().isPresent) {
            result.faceBlendshapes().get().firstOrNull()?.forEach { c ->
                when (c.categoryName()) {
                    "eyeBlinkLeft" -> blinkL = c.score().toDouble()
                    "eyeBlinkRight" -> blinkR = c.score().toDouble()
                }
            }
        }
        sink?.onFrame(Observation(ns, true, hasIris, blinkL, blinkR, sidecarLine(ns, landmarks, result)))
    }

    private fun sidecarLine(
        ns: Long,
        landmarks: List<com.google.mediapipe.tasks.components.containers.NormalizedLandmark>?,
        result: FaceLandmarkerResult,
    ): String {
        val sb = StringBuilder(2048)
        sb.append("{\"t_sec\":").append(ns / 1e9).append(",\"frame_ns\":").append(ns)
        sb.append(",\"face\":").append(landmarks != null)
        uprightSize?.let { sb.append(",\"image\":[").append(it.width).append(',').append(it.height).append(']') }
        if (landmarks != null) {
            val idx = if (landmarks.size >= 478) EyeLandmarks.WITH_IRIS else EyeLandmarks.MESH_468
            sb.append(",\"landmarks\":[")
            idx.forEachIndexed { i, k ->
                if (i > 0) sb.append(',')
                val p = landmarks[k]
                sb.append('[').num(p.x()).append(',').num(p.y()).append(',').num(p.z()).append(']')
            }
            sb.append(']')
            if (result.faceBlendshapes().isPresent) {
                sb.append(",\"blendshapes\":{")
                var first = true
                result.faceBlendshapes().get().firstOrNull()?.forEach { c ->
                    if (c.categoryName().startsWith("eye")) {
                        if (!first) sb.append(',')
                        first = false
                        sb.append('"').append(c.categoryName()).append("\":").num(c.score())
                    }
                }
                sb.append('}')
            }
            if (result.facialTransformationMatrixes().isPresent) {
                result.facialTransformationMatrixes().get().firstOrNull()?.let {
                    sb.append(",\"face_matrix\":").floats(it)
                }
            }
        }
        return sb.append('}').toString()
    }

    override fun stop() {
        stopped = true
        sink = null
        provider?.unbindAll()
        executor.shutdown()
        landmarker?.close()
        landmarker = null
    }

    @androidx.annotation.OptIn(ExperimentalCamera2Interop::class)
    override fun describe(): JSONObject {
        val o = JSONObject()
            .put("backend", id)
            .put("library", "com.google.mediapipe:tasks-vision:0.10.21")
            .put("model", "face_landmarker.task float16/1 (sha256 64184e22...)")
            .put("delegate", if (useGpu) "GPU" else "CPU")
            .put("cameraApi", "CameraX 1.4.2 ImageAnalysis RGBA_8888, front")
            .put("imageMirrored", false)
            .put("landmarkIndices", JSONArray(EyeLandmarks.WITH_IRIS.toList()))
            .put("landmarkIndicesWithoutIris", JSONArray(EyeLandmarks.MESH_468.toList()))
            .put("landmarkSpace", "x,y normalised to the upright (rotated, unmirrored) analysis image; z MediaPipe relative depth")
            .put("faceMatrixLayout", "16 floats as MediaPipe returns facialTransformationMatrixes (cm), uninterpreted")
        analysisSize?.let { o.put("analysisImage", JSONArray(listOf(it.width, it.height))) }
        uprightSize?.let { o.put("uprightImage", JSONArray(listOf(it.width, it.height))) }
        rotationDegrees?.let { o.put("rotationDegrees", it) }
        camera?.let { cam ->
            try {
                val info = Camera2CameraInfo.from(cam.cameraInfo)
                o.put("camera2Id", info.cameraId)
                info.getCameraCharacteristic(CameraCharacteristics.SENSOR_ORIENTATION)?.let { o.put("sensorOrientation", it) }
                info.getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_TIMESTAMP_SOURCE)?.let {
                    o.put("timestampSource", if (it == CameraCharacteristics.SENSOR_INFO_TIMESTAMP_SOURCE_REALTIME) "realtime" else "unknown")
                }
                info.getCameraCharacteristic(CameraCharacteristics.LENS_INFO_AVAILABLE_FOCAL_LENGTHS)?.let {
                    o.put("focalLengthsMm", JSONArray(it.map { f -> f.toDouble() }))
                }
                info.getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_PHYSICAL_SIZE)?.let {
                    o.put("sensorPhysicalSizeMm", JSONArray(listOf(it.width.toDouble(), it.height.toDouble())))
                }
                info.getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_PIXEL_ARRAY_SIZE)?.let {
                    o.put("sensorPixelArray", JSONArray(listOf(it.width, it.height)))
                }
                info.getCameraCharacteristic(CameraCharacteristics.SENSOR_INFO_ACTIVE_ARRAY_SIZE)?.let {
                    o.put("sensorActiveArray", JSONArray(listOf(it.left, it.top, it.right, it.bottom)))
                }
                if (Build.VERSION.SDK_INT >= 28) {
                    info.getCameraCharacteristic(CameraCharacteristics.LENS_INTRINSIC_CALIBRATION)?.let {
                        o.put("lensIntrinsics_fx_fy_cx_cy_s", JSONArray(it.map { f -> f.toDouble() }))
                    }
                }
                info.getCameraCharacteristic(CameraCharacteristics.LENS_POSE_TRANSLATION)?.let {
                    o.put("lensPoseTranslationM", JSONArray(it.map { f -> f.toDouble() }))
                }
            } catch (_: Exception) {
                // Facts are best effort; the log does not depend on them.
            }
        }
        return o
    }
}

