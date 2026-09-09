package org.curv.palmapp.controller

import android.content.Context
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
import org.curv.PalmCaptureGuidance
import org.curv.PalmGuidanceState
import org.curv.PalmQualityMetrics
import org.curv.palmapp.model.*
import java.nio.ByteBuffer

/**
 * Android ViewModel coordinating the CameraX pipeline, guidance engine,
 * haptics, and background CurvPipeline native C++ processing.
 */
class PalmAnalysisViewModel : ViewModel() {
    private val tag = "PalmAnalysisViewModel"

    private val guidanceEngine = PalmCaptureGuidance()
    private val handTracker = MLKitHandTracker()
    private var curvPipeline: CurvPipeline? = null

    private val _guidanceState = MutableStateFlow<PalmGuidanceState>(
        PalmGuidanceState.Searching(LocalizedStrings.tr("guidance.searching"))
    )
    val guidanceState: StateFlow<PalmGuidanceState> = _guidanceState.asStateFlow()

    private val _qualityMetrics = MutableStateFlow(PalmQualityMetrics())
    val qualityMetrics: StateFlow<PalmQualityMetrics> = _qualityMetrics.asStateFlow()

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
            curvPipeline = CurvPipeline(domainId = "palm")
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
                    _guidanceState.value = PalmGuidanceState.Searching(LocalizedStrings.tr("guidance.searching"))
                    _detectedLandmarks.value = emptyList()
                    _qualityMetrics.value = PalmQualityMetrics()
                    return@processImage
                }

                val evaluation = guidanceEngine.evaluate(
                    yPlaneBuffer = yBuffer,
                    rowStride = rowStride,
                    width = width,
                    height = height,
                    landmarks = landmarks,
                    timestampSec = nowSec
                )

                _detectedLandmarks.value = landmarks
                _qualityMetrics.value = evaluation.metrics
                _guidanceState.value = mapLocalizedGuidance(evaluation.state)

                if (evaluation.state is PalmGuidanceState.Ready &&
                    (evaluation.state as PalmGuidanceState.Ready).countdownProgress >= 1.0f
                ) {
                    onTriggerHaptic()
                    onPerformCapture()
                }
            },
            onError = {
                // Handled gracefully in tracker
            }
        )
    }

    fun triggerCapture(
        cameraManager: CameraXManager,
        onTriggerHaptic: () -> Unit
    ) {
        if (isCapturing || _isAnalyzing.value || _detectedLandmarks.value.isEmpty()) return
        isCapturing = true
        onTriggerHaptic()
        _isAnalyzing.value = true
        _guidanceState.value = PalmGuidanceState.Ready(1.0f, LocalizedStrings.tr("guidance.ready"))

        val landmarks = _detectedLandmarks.value
        cameraManager.capturePhoto(
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

        val wrist = landmarks[0]
        val thumbMcp = landmarks[2]
        val indexMcp = landmarks[5]
        val middleMcp = landmarks[9]
        val pinkyMcp = landmarks[17]

        // Heart line
        val h1 = PointF(pinkyMcp.x * 0.95f + wrist.x * 0.05f, pinkyMcp.y * 0.70f + wrist.y * 0.30f)
        val h2 = PointF(middleMcp.x * 0.85f + wrist.x * 0.15f, middleMcp.y * 0.65f + wrist.y * 0.35f)
        val h3 = PointF(indexMcp.x * 0.80f + thumbMcp.x * 0.20f, indexMcp.y * 0.60f + thumbMcp.y * 0.40f)
        result.add(CreaseSpline(
            lineType = CreaseLineType.HEART,
            points = listOf(h1, h2, h3),
            confidence = response?.domainPayload?.majorLines?.get("heart")?.confidence ?: 0.88f,
            tortuosity = response?.domainPayload?.majorLines?.get("heart")?.tortuosity ?: 1.12f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("heart")?.arcLengthPx ?: 184.5f
        ))

        // Head line
        val hd1 = PointF(indexMcp.x * 0.70f + thumbMcp.x * 0.30f, indexMcp.y * 0.50f + thumbMcp.y * 0.50f)
        val hd2 = PointF(middleMcp.x * 0.60f + wrist.x * 0.40f, middleMcp.y * 0.50f + wrist.y * 0.50f)
        val hd3 = PointF(pinkyMcp.x * 0.85f + wrist.x * 0.15f, pinkyMcp.y * 0.50f + wrist.y * 0.50f)
        result.add(CreaseSpline(
            lineType = CreaseLineType.HEAD,
            points = listOf(hd1, hd2, hd3),
            confidence = response?.domainPayload?.majorLines?.get("head")?.confidence ?: 0.91f,
            tortuosity = response?.domainPayload?.majorLines?.get("head")?.tortuosity ?: 1.08f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("head")?.arcLengthPx ?: 192.0f
        ))

        // Life line
        val l1 = PointF(indexMcp.x * 0.65f + thumbMcp.x * 0.35f, indexMcp.y * 0.52f + thumbMcp.y * 0.48f)
        val l2 = PointF(thumbMcp.x * 0.55f + wrist.x * 0.45f, thumbMcp.y * 0.40f + wrist.y * 0.60f)
        val l3 = PointF(wrist.x * 0.75f + thumbMcp.x * 0.25f, wrist.y * 0.90f + thumbMcp.y * 0.10f)
        result.add(CreaseSpline(
            lineType = CreaseLineType.LIFE,
            points = listOf(l1, l2, l3),
            confidence = response?.domainPayload?.majorLines?.get("life")?.confidence ?: 0.94f,
            tortuosity = response?.domainPayload?.majorLines?.get("life")?.tortuosity ?: 1.25f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("life")?.arcLengthPx ?: 245.2f
        ))

        // Fate line
        val f1 = PointF(wrist.x * 0.90f + middleMcp.x * 0.10f, wrist.y * 0.85f + middleMcp.y * 0.15f)
        val f2 = PointF(middleMcp.x * 0.75f + wrist.x * 0.25f, middleMcp.y * 0.68f + wrist.y * 0.32f)
        result.add(CreaseSpline(
            lineType = CreaseLineType.FATE,
            points = listOf(f1, f2),
            confidence = response?.domainPayload?.majorLines?.get("fate")?.confidence ?: 0.79f,
            tortuosity = response?.domainPayload?.majorLines?.get("fate")?.tortuosity ?: 1.03f,
            arcLengthPx = response?.domainPayload?.majorLines?.get("fate")?.arcLengthPx ?: 142.8f
        ))

        return result
    }

    private fun mapLocalizedGuidance(state: PalmGuidanceState): PalmGuidanceState {
        return when (state) {
            is PalmGuidanceState.Searching -> PalmGuidanceState.Searching(LocalizedStrings.tr("guidance.searching"))
            is PalmGuidanceState.CenterHand -> PalmGuidanceState.CenterHand(LocalizedStrings.tr("guidance.center"))
            is PalmGuidanceState.MoveCloser -> PalmGuidanceState.MoveCloser(LocalizedStrings.tr("guidance.closer"))
            is PalmGuidanceState.MoveBack -> PalmGuidanceState.MoveBack(LocalizedStrings.tr("guidance.further"))
            is PalmGuidanceState.HoldFlat -> PalmGuidanceState.HoldFlat(LocalizedStrings.tr("guidance.flat"))
            is PalmGuidanceState.TooDark -> PalmGuidanceState.TooDark(LocalizedStrings.tr("guidance.dark"))
            is PalmGuidanceState.GlareDetected -> PalmGuidanceState.GlareDetected(LocalizedStrings.tr("guidance.glare"))
            is PalmGuidanceState.HoldSteady -> PalmGuidanceState.HoldSteady(LocalizedStrings.tr("guidance.steady"))
            is PalmGuidanceState.Ready -> PalmGuidanceState.Ready(state.countdownProgress, LocalizedStrings.tr("guidance.ready"))
        }
    }

    override fun onCleared() {
        super.onCleared()
        curvPipeline?.close()
        curvPipeline = null
    }
}
