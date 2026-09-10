package org.curv.palmapp.controller

import android.graphics.Bitmap
import android.graphics.PointF
import android.os.SystemClock
import android.util.Log
import androidx.camera.core.ImageProxy
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.curv.CurvPipeline
import org.curv.overlay.camera.CameraSession
import org.curv.overlay.guidance.CaptureGuidance
import org.curv.overlay.guidance.GuidanceState
import org.curv.overlay.guidance.Photometrics
import org.curv.overlay.guidance.QualityMetrics
import org.curv.overlay.palm.HandLandmarkPose
import org.curv.overlay.palm.HandSide
import org.curv.overlay.palm.PalmOverlay
import org.curv.palmapp.model.*
import java.nio.ByteBuffer

/**
 * Palm-domain ViewModel — parity with iOS PalmAnalysisViewModel.swift.
 * Camera/HUD from overlay kit; post-capture uses CurvEngine Steger polylines when
 * available, never invents fake creases. Falls back to editable scaffolds.
 */
class PalmAnalysisViewModel : ViewModel() {
    private val tag = "PalmAnalysisViewModel"

    val profile = PalmOverlay.profile
    private val guidanceEngine = CaptureGuidance(PalmOverlay.profile.thresholds)
    private val handTracker = MLKitHandTracker()
    private var curvPipeline: CurvPipeline? = null

    private val _guidanceState = MutableStateFlow<GuidanceState>(GuidanceState.Searching)
    val guidanceState: StateFlow<GuidanceState> = _guidanceState.asStateFlow()

    private val _qualityMetrics = MutableStateFlow(QualityMetrics())
    val qualityMetrics: StateFlow<QualityMetrics> = _qualityMetrics.asStateFlow()

    private val _detectedLandmarks = MutableStateFlow<List<PointF>>(emptyList())
    val detectedLandmarks: StateFlow<List<PointF>> = _detectedLandmarks.asStateFlow()

    private val _statusOverride = MutableStateFlow<String?>(null)
    val statusMessageOverride: StateFlow<String?> = _statusOverride.asStateFlow()

    private val _isAnalyzing = MutableStateFlow(false)
    val isAnalyzing: StateFlow<Boolean> = _isAnalyzing.asStateFlow()

    private val _analysisResult = MutableStateFlow<PalmAnalysisResponse?>(null)
    val analysisResult: StateFlow<PalmAnalysisResponse?> = _analysisResult.asStateFlow()

    private val _detectedSplines = MutableStateFlow<List<CreaseSpline>>(emptyList())
    val detectedSplines: StateFlow<List<CreaseSpline>> = _detectedSplines.asStateFlow()

    private val _captureLandmarks = MutableStateFlow<List<PointF>>(emptyList())
    val captureLandmarks: StateFlow<List<PointF>> = _captureLandmarks.asStateFlow()

    private val _capturedBitmap = MutableStateFlow<Bitmap?>(null)
    val capturedBitmap: StateFlow<Bitmap?> = _capturedBitmap.asStateFlow()

    private val _showResultSheet = MutableStateFlow(false)
    val showResultSheet: StateFlow<Boolean> = _showResultSheet.asStateFlow()

    private val _showResultViewer = MutableStateFlow(false)
    val showResultViewer: StateFlow<Boolean> = _showResultViewer.asStateFlow()

    private val _overlaySource = MutableStateFlow(CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT)
    val overlaySource: StateFlow<CreaseOverlaySource> = _overlaySource.asStateFlow()

    private val _usedNativeEngine = MutableStateFlow(false)
    val usedNativeEngine: StateFlow<Boolean> = _usedNativeEngine.asStateFlow()

    private val _predictionReadySplines = MutableStateFlow<List<CreaseSpline>>(emptyList())
    val predictionReadySplines: StateFlow<List<CreaseSpline>> = _predictionReadySplines.asStateFlow()

    private val _currentLanguage = MutableStateFlow(AppLanguage.ENGLISH)
    val currentLanguage: StateFlow<AppLanguage> = _currentLanguage.asStateFlow()

    private val _selectedHandSide = MutableStateFlow(HandSide.AUTO)
    val selectedHandSide: StateFlow<HandSide> = _selectedHandSide.asStateFlow()

    private val _detectedHandSide = MutableStateFlow(HandSide.RIGHT)
    val detectedHandSide: StateFlow<HandSide> = _detectedHandSide.asStateFlow()

    fun selectHandSide(side: HandSide) { _selectedHandSide.value = side }

    private var isCapturing = false
    private var suppressAutoCaptureUntilNs: Long = 0L
    private var lastCountdownDigit = 0
    private var lastMetricsPublishNs: Long = 0L

    init {
        try {
            curvPipeline = CurvPipeline(domainId = PalmOverlay.profile.domainId)
            Log.i(tag, "CurvPipeline ready (domain=palm)")
        } catch (e: Throwable) {
            Log.e(tag, "CurvPipeline init failed", e)
        }
    }

    fun setLanguage(lang: AppLanguage) {
        _currentLanguage.value = lang
        LocalizedStrings.currentLanguage = lang
    }

    fun processFrame(
        imageProxy: ImageProxy,
        onTriggerHaptic: () -> Unit,
        onPerformCapture: () -> Unit
    ) {
        if (isCapturing || _isAnalyzing.value || _showResultViewer.value) {
            imageProxy.close()
            return
        }
        val yPlane = imageProxy.planes[0]
        val yBuffer = yPlane.buffer
        val rowStride = yPlane.rowStride
        val width = imageProxy.width
        val height = imageProxy.height
        val nowSec = SystemClock.elapsedRealtimeNanos() / 1_000_000_000.0
        val nowNs = SystemClock.elapsedRealtimeNanos()

        handTracker.processImage(
            imageProxy = imageProxy,
            onSuccess = { landmarks ->
                if (landmarks.isEmpty()) {
                    _guidanceState.value = GuidanceState.Searching
                    _detectedLandmarks.value = emptyList()
                    _qualityMetrics.value = QualityMetrics()
                    _statusOverride.value = null
                    return@processImage
                }
                val pose = HandLandmarkPose.from(landmarks)
                val tipsOK = pose != null
                val photometrics = if (pose != null) {
                    Photometrics.sample(yBuffer, width, height, rowStride, pose.centroid.x, pose.centroid.y, pose.scale * 0.30f)
                } else org.curv.overlay.guidance.PhotometricSample.unknown

                var evaluation = guidanceEngine.evaluate(if (tipsOK) pose else null, photometrics, nowSec)
                var statusOverride: String? = null

                if (pose != null) {
                    if (!tipsOK) {
                        statusOverride = "Move ~2 cm farther — keep fingertips inside"
                    } else {
                        // distance hint when scale outside range
                        val scale = pose.scale
                        val minS = profile.thresholds.minScale
                        val maxS = profile.thresholds.maxScale
                        if (scale < minS || scale > maxS) {
                            val cm = ((kotlin.math.abs(scale - (if (scale < minS) minS else maxS)) / 0.03f).toInt().coerceAtLeast(1))
                            statusOverride = if (scale < minS) "Move ~${cm} cm closer" else "Move ~${cm} cm farther"
                        } else if (evaluation.state is GuidanceState.CenterSubject) {
                            statusOverride = centerHint(pose)
                        } else if (evaluation.state is GuidanceState.Ready) {
                            val p = (evaluation.state as GuidanceState.Ready).countdownProgress
                            if (p < 1f) statusOverride = profile.copy.message(evaluation.state)
                        }
                    }
                }
                if (!tipsOK) {
                    // Force moveBack gate like iOS
                    var m = evaluation.metrics
                    if (pose != null) {
                        m = m.withOverrides(
                            scale = pose.scale,
                            centerOffset = kotlin.math.sqrt(((pose.centroid.x - 0.5f)*(pose.centroid.x - 0.5f) + (pose.centroid.y - 0.5f)*(pose.centroid.y - 0.5f)).toDouble()).toFloat(),
                            rollAngleDeg = kotlin.math.abs(pose.rollDeg),
                            flatnessRatio = pose.aspectRatio,
                            meanLuminance = photometrics.meanLuminance,
                            glareRatio = photometrics.glareRatio,
                            isReadyForCapture = false
                        )
                    }
                    evaluation = org.curv.overlay.guidance.GuidanceEvaluation(GuidanceState.MoveBack, m)
                }

                val shouldPublish = nowNs - lastMetricsPublishNs >= 66_000_000L // ~15 Hz
                if (shouldPublish) lastMetricsPublishNs = nowNs

                val autoArmed = nowNs >= suppressAutoCaptureUntilNs
                val shouldCapture = autoArmed && tipsOK && evaluation.state is GuidanceState.Ready &&
                    (evaluation.state as GuidanceState.Ready).countdownProgress >= 1f && evaluation.metrics.isReadyForCapture

                _detectedLandmarks.value = landmarks
                _detectedHandSide.value = if (HandLandmarkPose.isLeftHand(landmarks)) HandSide.LEFT else HandSide.RIGHT
                if (!tipsOK) guidanceEngine.reset()
                _guidanceState.value = evaluation.state
                _statusOverride.value = statusOverride
                if (shouldPublish) _qualityMetrics.value = evaluation.metrics

                val digit = profile.copy.countdownDigit(evaluation.state) ?: 0
                if (digit != 0 && digit != lastCountdownDigit) {
                    lastCountdownDigit = digit
                    // light haptic handled by caller via onTrigger? we use onTrigger for capture only
                }
                if (evaluation.state !is GuidanceState.Ready) lastCountdownDigit = 0

                if (shouldCapture) {
                    onTriggerHaptic()
                    onPerformCapture()
                }
            },
            onError = { }
        )
    }

    private fun centerHint(pose: org.curv.overlay.guidance.SubjectPose): String {
        val dx = pose.centroid.x - 0.5f
        val dy = pose.centroid.y - 0.5f
        val perCm = 0.03f
        return if (kotlin.math.abs(dx) >= kotlin.math.abs(dy)) {
            val cm = (kotlin.math.abs(dx) / perCm).toInt().coerceAtLeast(1)
            if (dx > 0) "Move ~${cm} cm left" else "Move ~${cm} cm right"
        } else {
            val cm = (kotlin.math.abs(dy) / perCm).toInt().coerceAtLeast(1)
            if (dy > 0) "Move ~${cm} cm up" else "Move ~${cm} cm down"
        }
    }

    fun triggerCapture(cameraSession: CameraSession, onTriggerHaptic: () -> Unit) {
        if (isCapturing || _isAnalyzing.value || _detectedLandmarks.value.isEmpty()) return
        val pose = HandLandmarkPose.from(_detectedLandmarks.value) ?: return
        // Require in-frame
        if (pose.samplePoints.any { it.x < 0.04f || it.x > 0.96f || it.y < 0.04f || it.y > 0.96f }) return
        isCapturing = true
        suppressAutoCaptureUntilNs = SystemClock.elapsedRealtimeNanos() + 10_000_000_000L
        onTriggerHaptic()
        _isAnalyzing.value = true
        _guidanceState.value = GuidanceState.Ready(1f)

        val landmarks = _detectedLandmarks.value.toList()
        cameraSession.capturePhoto(
            onSuccess = { buffer, width, height, stride ->
                // Build bitmap for thumbnail + re-detect
                val bmp = yuvGrayscaleToBitmap(buffer, width, height)
                // Try re-detect on still; fallback to video landmarks
                if (bmp != null) {
                    handTracker.processBitmap(bmp,
                        onSuccess = { stillLandmarks ->
                            val photoLandmarks = if (stillLandmarks.size == 21) stillLandmarks else landmarks
                            if (stillLandmarks.size == 21) Log.i(tag, "Re-detected ${stillLandmarks.size} landmarks on still")
                            else Log.w(tag, "Still re-detect failed — using video landmarks")
                            executeDeepPerception(buffer, width, height, stride, photoLandmarks, bmp)
                        },
                        onError = {
                            executeDeepPerception(buffer, width, height, stride, landmarks, bmp)
                        }
                    )
                } else {
                    executeDeepPerception(buffer, width, height, stride, landmarks, null)
                }
            },
            onError = { err ->
                Log.e(tag, "Capture failed", err)
                isCapturing = false
                _isAnalyzing.value = false
                suppressAutoCaptureUntilNs = SystemClock.elapsedRealtimeNanos() + 1_500_000_000L
                guidanceEngine.reset()
            }
        )
    }

    private fun yuvGrayscaleToBitmap(buffer: ByteBuffer, width: Int, height: Int): Bitmap? = try {
        val arr = ByteArray(buffer.remaining())
        buffer.duplicate().get(arr)
        val bmp = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val pixels = IntArray(width * height)
        for (i in pixels.indices) {
            val v = arr[i].toInt() and 0xFF
            pixels[i] = (0xFF shl 24) or (v shl 16) or (v shl 8) or v
        }
        bmp.setPixels(pixels, 0, width, 0, 0, width, height)
        bmp
    } catch (e: Exception) { Log.e(tag, "Bitmap conversion failed", e); null }

    private fun executeDeepPerception(
        buffer: ByteBuffer, width: Int, height: Int, stride: Int,
        landmarks: List<PointF>, bitmap: Bitmap?
    ) {
        viewModelScope.launch {
            val flat = MLKitHandTracker.toFlatLandmarksArray(landmarks)
            val (response, splines, usedNative, source) = withContext(Dispatchers.Default) {
                var resp: PalmAnalysisResponse? = null
                var lines: List<CreaseSpline> = emptyList()
                var native = false
                var src = CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT
                curvPipeline?.let { pipeline ->
                    try {
                        val jsonStr = pipeline.processLandmarksRoi(buffer, width, height, stride, CurvPipeline.FORMAT_GRAYSCALE, flat)
                        resp = PalmAnalysisResponse.fromJson(jsonStr)
                        val nativeSplines = resp?.creaseSplinesFromPolylines()
                        if (nativeSplines != null) {
                            lines = nativeSplines
                            native = true
                            src = CreaseOverlaySource.NATIVE_ENGINE
                            Log.i(tag, "Native Steger returned ${nativeSplines.size} polylines")
                        } else {
                            Log.w(tag, "No native polylines — manual alignment (is_usable=${resp?.isUsable})")
                        }
                    } catch (e: Exception) {
                        Log.e(tag, "CurvPipeline error", e)
                    }
                }
                if (src == CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT) {
                    lines = manualSeedCreases(landmarks)
                }
                Quad(resp, lines, native, src)
            }
            _analysisResult.value = response
            _detectedSplines.value = splines
            _captureLandmarks.value = landmarks
            _capturedBitmap.value = bitmap
            _usedNativeEngine.value = usedNative
            _overlaySource.value = source
            _isAnalyzing.value = false
            _showResultViewer.value = (source == CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT)
            _showResultSheet.value = true
            isCapturing = false
            suppressAutoCaptureUntilNs = SystemClock.elapsedRealtimeNanos() + 3_000_000_000L
            guidanceEngine.reset()
        }
    }

    fun openLastCapture() { if (_capturedBitmap.value != null) _showResultViewer.value = true }
    fun dismissResultViewer() { _showResultViewer.value = false }
    fun updateSplines(newList: List<CreaseSpline>) { _detectedSplines.value = newList }
    fun confirmAdjustedLinesForPrediction() {
        _predictionReadySplines.value = _detectedSplines.value
        Log.i(tag, "Prediction lines locked (${_predictionReadySplines.value.size})")
    }
    fun predictionPayloadJSON(): String? {
        val lines = if (_predictionReadySplines.value.isEmpty()) _detectedSplines.value else _predictionReadySplines.value
        if (lines.isEmpty()) return null
        val obj = org.json.JSONObject()
        val payload = org.json.JSONObject()
        val majors = org.json.JSONObject()
        for (l in lines) {
            val entry = org.json.JSONObject()
            entry.put("name", l.engineKey)
            entry.put("present", true)
            entry.put("confidence", l.confidence)
            entry.put("user_adjusted", l.isUserAdjusted)
            val pts = org.json.JSONArray()
            for (pt in l.predictionPointsImageNorm) { val a = org.json.JSONArray(); a.put(pt[0]); a.put(pt[1]); pts.put(a) }
            entry.put("points_image_norm", pts)
            majors.put(l.engineKey, entry)
        }
        payload.put("major_lines", majors)
        payload.put("source", _overlaySource.value.name.lowercase())
        obj.put("domain_payload", payload)
        return obj.toString()
    }

    fun clearLastCapture() {
        _analysisResult.value = null
        _detectedSplines.value = emptyList()
        _predictionReadySplines.value = emptyList()
        _captureLandmarks.value = emptyList()
        _capturedBitmap.value = null
        _showResultViewer.value = false
        _showResultSheet.value = false
        _usedNativeEngine.value = false
        _overlaySource.value = CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT
    }

    fun retake() {
        isCapturing = false
        _isAnalyzing.value = false
        _analysisResult.value = null
        _detectedSplines.value = emptyList()
        _predictionReadySplines.value = emptyList()
        _captureLandmarks.value = emptyList()
        _capturedBitmap.value = null
        _showResultViewer.value = false
        _showResultSheet.value = false
        _usedNativeEngine.value = false
        _overlaySource.value = CreaseOverlaySource.NEEDS_MANUAL_ALIGNMENT
        guidanceEngine.reset()
    }

    /** Editable 4-point scaffolds — parity with iOS manualAlignmentSeedCreases. */
    private fun manualSeedCreases(landmarks: List<PointF>): List<CreaseSpline> {
        fun lerp(a: PointF, b: PointF, t: Float) = PointF(a.x + (b.x - a.x)*t, a.y + (b.y - a.y)*t)
        val wrist: PointF; val indexMcp: PointF; val middleMcp: PointF; val pinkyMcp: PointF; val thumbMcp: PointF; val middleTip: PointF
        if (landmarks.size >= 21) {
            wrist = landmarks[0]; thumbMcp = landmarks[2]; indexMcp = landmarks[5]; middleMcp = landmarks[9]; middleTip = landmarks[12]; pinkyMcp = landmarks[17]
        } else {
            wrist = PointF(0.50f,0.82f); thumbMcp = PointF(0.28f,0.55f); indexMcp = PointF(0.38f,0.28f); middleMcp = PointF(0.50f,0.26f); middleTip = PointF(0.50f,0.08f); pinkyMcp = PointF(0.68f,0.32f)
        }
        val palmTop = lerp(indexMcp, pinkyMcp, 0.5f)
        val palmMid = lerp(palmTop, wrist, 0.45f)
        val heart = listOf(lerp(pinkyMcp,wrist,0.12f), lerp(pinkyMcp,indexMcp,0.35f), lerp(pinkyMcp,indexMcp,0.70f), lerp(indexMcp,wrist,0.15f))
        val head = listOf(lerp(indexMcp,thumbMcp,0.35f), lerp(indexMcp,pinkyMcp,0.25f), lerp(middleMcp,wrist,0.35f), lerp(pinkyMcp,wrist,0.28f))
        val life = listOf(lerp(indexMcp,thumbMcp,0.40f), lerp(thumbMcp,wrist,0.25f), lerp(thumbMcp,wrist,0.55f), lerp(wrist,thumbMcp,0.15f))
        val fate = listOf(lerp(wrist,middleTip,0.15f), lerp(wrist,middleTip,0.40f), lerp(palmMid,middleMcp,0.35f), lerp(middleMcp,wrist,0.20f))
        fun seed(id: String, type: CreaseLineType, pts: List<PointF>) = CreaseSpline(id= id, lineType= type, points= pts, confidence=0f, tortuosity=1f, arcLengthPx=0f, isUserAdjusted=false)
        return listOf(seed("heart_line", CreaseLineType.HEART, heart), seed("head_line", CreaseLineType.HEAD, head), seed("life_line", CreaseLineType.LIFE, life), seed("fate_line", CreaseLineType.FATE, fate))
    }

    override fun onCleared() { super.onCleared(); curvPipeline?.close(); curvPipeline = null }

    private data class Quad(val resp: PalmAnalysisResponse?, val splines: List<CreaseSpline>, val native: Boolean, val src: CreaseOverlaySource)
    private fun QualityMetrics.withOverrides(scale: Float = this.scale, centerOffset: Float = this.centerOffset, rollAngleDeg: Float = this.rollAngleDeg, flatnessRatio: Float = this.flatnessRatio, meanLuminance: Float = this.meanLuminance, glareRatio: Float = this.glareRatio, isReadyForCapture: Boolean = this.isReadyForCapture) = copy(scale=scale, centerOffset=centerOffset, rollAngleDeg=rollAngleDeg, flatnessRatio=flatnessRatio, meanLuminance=meanLuminance, glareRatio=glareRatio, isReadyForCapture=isReadyForCapture)
}

