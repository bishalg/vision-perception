package org.curv.palmapp.controller

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
import org.curv.overlay.palm.PalmOverlay
import org.curv.palmapp.model.*
import java.nio.ByteBuffer

/**
 * Palm-domain ViewModel. Camera + guidance live in the shared overlay kit;
 * this class only maps landmarks and hands off to CurvPipeline.
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

    private val _isAnalyzing = MutableStateFlow(false)
    val isAnalyzing: StateFlow<Boolean> = _isAnalyzing.asStateFlow()

    private val _analysisResult = MutableStateFlow<PalmAnalysisResponse?>(null)
    val analysisResult: StateFlow<PalmAnalysisResponse?> = _analysisResult.asStateFlow()

    private val _detectedSplines = MutableStateFlow<List<CreaseSpline>>(emptyList())
    val detectedSplines: StateFlow<List<CreaseSpline>> = _detectedSplines.asStateFlow()

    private val _showResultSheet = MutableStateFlow(false)
    val showResultSheet: StateFlow<Boolean> = _showResultSheet.asStateFlow()

    private val _currentLanguage = MutableStateFlow(AppLanguage.ENGLISH)
    val currentLanguage: StateFlow<AppLanguage> = _currentLanguage.asStateFlow()

    private var isCapturing = false

    init {
        try {
            curvPipeline = CurvPipeline(domainId = PalmOverlay.profile.domainId)
            Log.i(tag, "Successfully initialized CurvPipeline native C++ engine")
        } catch (e: Throwable) {
            Log.e(tag, "CurvPipeline native initialization failed", e)
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
        if (isCapturing || _isAnalyzing.value) {
            imageProxy.close()
            return
        }

        val yPlane = imageProxy.planes[0]
        val yBuffer = yPlane.buffer
        val rowStride = yPlane.rowStride
        val width = imageProxy.width
        val height = imageProxy.height
        val nowSec = SystemClock.elapsedRealtimeNanos() / 1_000_000_000.0

        handTracker.processImage(
            imageProxy = imageProxy,
            onSuccess = { landmarks ->
                if (landmarks.isEmpty()) {
                    _guidanceState.value = GuidanceState.Searching
                    _detectedLandmarks.value = emptyList()
                    _qualityMetrics.value = QualityMetrics()
                    return@processImage
                }

                val pose = HandLandmarkPose.from(landmarks)
                val photometrics = if (pose != null) {
                    Photometrics.sample(
                        yBuffer = yBuffer,
                        width = width,
                        height = height,
                        rowStride = rowStride,
                        centerX = pose.centroid.x,
                        centerY = pose.centroid.y,
                        radius = pose.scale * 0.3f
                    )
                } else {
                    org.curv.overlay.guidance.PhotometricSample.unknown
                }

                val evaluation = guidanceEngine.evaluate(pose, photometrics, nowSec)
                _detectedLandmarks.value = landmarks
                _qualityMetrics.value = evaluation.metrics
                _guidanceState.value = evaluation.state

                if (evaluation.state is GuidanceState.Ready &&
                    evaluation.state.countdownProgress >= 1.0f
                ) {
                    onTriggerHaptic()
                    onPerformCapture()
                }
            },
            onError = { }
        )
    }

    fun triggerCapture(
        cameraSession: CameraSession,
        onTriggerHaptic: () -> Unit
    ) {
        if (isCapturing || _isAnalyzing.value || _detectedLandmarks.value.isEmpty()) return
        isCapturing = true
        onTriggerHaptic()
        _isAnalyzing.value = true
        _guidanceState.value = GuidanceState.Ready(1.0f)

        val landmarks = _detectedLandmarks.value
        cameraSession.capturePhoto(
            onSuccess = { buffer, width, height, stride ->
                executeDeepPerception(buffer, width, height, stride, landmarks)
            },
            onError = { err ->
                Log.e(tag, "Capture failed", err)
                isCapturing = false
                _isAnalyzing.value = false
            }
        )
    }

    private fun executeDeepPerception(
        buffer: ByteBuffer,
        width: Int,
        height: Int,
        stride: Int,
        landmarks: List<PointF>
    ) {
        viewModelScope.launch {
            val flatLandmarks = MLKitHandTracker.toFlatLandmarksArray(landmarks)

            val (response, splines) = withContext(Dispatchers.Default) {
                var resp: PalmAnalysisResponse? = null
                var lines = emptyList<CreaseSpline>()

                curvPipeline?.let { pipeline ->
                    try {
                        val jsonStr = pipeline.processLandmarksRoi(
                            buffer = buffer,
                            width = width,
                            height = height,
                            stride = stride,
                            format = CurvPipeline.FORMAT_GRAYSCALE,
                            landmarks = flatLandmarks
                        )
                        resp = PalmAnalysisResponse.fromJson(jsonStr)
                        lines = generateSplinesFromAnalysis(resp, landmarks)
                    } catch (e: Exception) {
                        Log.e(tag, "CurvPipeline execution error", e)
                    }
                }
                Pair(resp, lines)
            }

            _analysisResult.value = response
            _detectedSplines.value = splines
            _isAnalyzing.value = false
            _showResultSheet.value = true
            isCapturing = false
        }
    }

    fun retake() {
        isCapturing = false
        _isAnalyzing.value = false
        _analysisResult.value = null
        _detectedSplines.value = emptyList()
        _showResultSheet.value = false
        guidanceEngine.reset()
    }

    private fun generateSplinesFromAnalysis(
        response: PalmAnalysisResponse?,
        landmarks: List<PointF>
    ): List<CreaseSpline> {
        if (landmarks.size < 21) return emptyList()
        val result = mutableListOf<CreaseSpline>()

        // MediaPipe: tips 4/8/12/16/20; MCP knuckle row 5/9/13/17; wrist 0.
        // Depth t=0 at MCP, t=1 at wrist — heart stays just under knuckles.
        val wrist = landmarks[0]
        val thumbCmc = landmarks[1]
        val thumbMcp = landmarks[2]
        val thumbIp = landmarks[3]
        val indexMcp = landmarks[5]
        val middleMcp = landmarks[9]
        val middleTip = landmarks[12]
        val ringMcp = landmarks[13]
        val pinkyMcp = landmarks[17]

        fun lerp(a: PointF, b: PointF, t: Float) =
            PointF(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
        fun palmDepth(mcp: PointF, t: Float) = lerp(mcp, wrist, t)
        val lifeOrigin = lerp(indexMcp, thumbMcp, 0.45f)

        result.add(CreaseSpline(
            lineType = CreaseLineType.HEART,
            points = listOf(
                palmDepth(pinkyMcp, 0.06f),
                palmDepth(ringMcp, 0.10f),
                palmDepth(middleMcp, 0.12f),
                palmDepth(indexMcp, 0.10f),
                lerp(palmDepth(indexMcp, 0.08f), palmDepth(middleMcp, 0.08f), 0.35f)
            ),
            confidence = response?.domainPayload?.majorLines?.get("heart")?.confidence ?: 0.88f,
            tortuosity = response?.domainPayload?.majorLines?.get("heart")?.tortuosity ?: 1.12f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("heart")?.arcLengthPx ?: 184.5f
        ))

        result.add(CreaseSpline(
            lineType = CreaseLineType.HEAD,
            points = listOf(
                lerp(lifeOrigin, wrist, 0.18f),
                palmDepth(indexMcp, 0.32f),
                palmDepth(middleMcp, 0.36f),
                palmDepth(ringMcp, 0.34f),
                palmDepth(pinkyMcp, 0.30f)
            ),
            confidence = response?.domainPayload?.majorLines?.get("head")?.confidence ?: 0.91f,
            tortuosity = response?.domainPayload?.majorLines?.get("head")?.tortuosity ?: 1.08f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("head")?.arcLengthPx ?: 192.0f
        ))

        result.add(CreaseSpline(
            lineType = CreaseLineType.LIFE,
            points = listOf(
                lerp(lifeOrigin, wrist, 0.08f),
                lerp(thumbMcp, indexMcp, 0.25f),
                lerp(thumbMcp, wrist, 0.35f),
                lerp(thumbIp, wrist, 0.55f),
                lerp(thumbCmc, wrist, 0.72f),
                lerp(wrist, thumbCmc, 0.15f)
            ),
            confidence = response?.domainPayload?.majorLines?.get("life")?.confidence ?: 0.94f,
            tortuosity = response?.domainPayload?.majorLines?.get("life")?.tortuosity ?: 1.25f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("life")?.arcLengthPx ?: 245.2f
        ))

        result.add(CreaseSpline(
            lineType = CreaseLineType.FATE,
            points = listOf(
                lerp(wrist, middleTip, 0.12f),
                lerp(wrist, middleTip, 0.38f),
                lerp(wrist, middleTip, 0.58f),
                palmDepth(middleMcp, 0.22f)
            ),
            confidence = response?.domainPayload?.majorLines?.get("fate")?.confidence ?: 0.79f,
            tortuosity = response?.domainPayload?.majorLines?.get("fate")?.tortuosity ?: 1.03f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("fate")?.arcLengthPx ?: 142.8f
        ))

        return result
    }

    override fun onCleared() {
        super.onCleared()
        curvPipeline?.close()
        curvPipeline = null
    }
}
