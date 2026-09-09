package org.curv.vastuapp.controller

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
import org.curv.overlay.guidance.SubjectPose
import org.curv.overlay.vastu.VastuOverlay
import java.nio.ByteBuffer

class VastuAnalysisViewModel : ViewModel() {
    private val tag = "VastuAnalysisViewModel"

    val profile = VastuOverlay.profile
    private val guidanceEngine = CaptureGuidance(VastuOverlay.profile.thresholds)
    private var curvPipeline: CurvPipeline? = null

    private val _guidanceState = MutableStateFlow<GuidanceState>(GuidanceState.Searching)
    val guidanceState: StateFlow<GuidanceState> = _guidanceState.asStateFlow()

    private val _qualityMetrics = MutableStateFlow(QualityMetrics())
    val qualityMetrics: StateFlow<QualityMetrics> = _qualityMetrics.asStateFlow()

    private val _isAnalyzing = MutableStateFlow(false)
    val isAnalyzing: StateFlow<Boolean> = _isAnalyzing.asStateFlow()

    private val _analysisJSON = MutableStateFlow<String?>(null)
    val analysisJSON: StateFlow<String?> = _analysisJSON.asStateFlow()

    private val _showResultSheet = MutableStateFlow(false)
    val showResultSheet: StateFlow<Boolean> = _showResultSheet.asStateFlow()

    private var isCapturing = false

    init {
        try {
            curvPipeline = CurvPipeline(domainId = VastuOverlay.profile.domainId)
            Log.i(tag, "Initialized CurvPipeline floor_plan domain")
        } catch (e: Throwable) {
            Log.e(tag, "CurvPipeline init failed", e)
        }
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
        val nowSec = SystemClock.elapsedRealtimeNanos() / 1_000_000_000.0
        val pose = SubjectPose.reticleFill(
            profile.reticleWidthFraction,
            profile.reticleHeightFraction
        )
        val photometrics = Photometrics.sample(
            yBuffer = yPlane.buffer,
            width = imageProxy.width,
            height = imageProxy.height,
            rowStride = yPlane.rowStride,
            centerX = pose.centroid.x,
            centerY = pose.centroid.y,
            radius = pose.scale * 0.35f
        )

        val evaluation = guidanceEngine.evaluate(pose, photometrics, nowSec)
        _qualityMetrics.value = evaluation.metrics
        _guidanceState.value = evaluation.state
        imageProxy.close()

        if (evaluation.state is GuidanceState.Ready && evaluation.state.countdownProgress >= 1f) {
            onTriggerHaptic()
            onPerformCapture()
        }
    }

    fun triggerCapture(cameraSession: CameraSession, onTriggerHaptic: () -> Unit) {
        if (isCapturing || _isAnalyzing.value) return
        isCapturing = true
        onTriggerHaptic()
        _isAnalyzing.value = true
        _guidanceState.value = GuidanceState.Ready(1f)

        cameraSession.capturePhoto(
            onSuccess = { buffer, width, height, stride ->
                executeDeepPerception(buffer, width, height, stride)
            },
            onError = { err ->
                Log.e(tag, "Capture failed", err)
                isCapturing = false
                _isAnalyzing.value = false
            }
        )
    }

    private fun executeDeepPerception(buffer: ByteBuffer, width: Int, height: Int, stride: Int) {
        viewModelScope.launch {
            val json = withContext(Dispatchers.Default) {
                try {
                    curvPipeline?.processQuadRoi(
                        buffer = buffer,
                        width = width,
                        height = height,
                        stride = stride,
                        format = CurvPipeline.FORMAT_GRAYSCALE,
                        corners = VastuOverlay.reticleQuad()
                    )
                } catch (e: Exception) {
                    Log.e(tag, "CurvPipeline quad ROI error", e)
                    null
                }
            }
            _analysisJSON.value = json
            _isAnalyzing.value = false
            _showResultSheet.value = true
            isCapturing = false
        }
    }

    fun retake() {
        isCapturing = false
        _isAnalyzing.value = false
        _analysisJSON.value = null
        _showResultSheet.value = false
        guidanceEngine.reset()
    }

    override fun onCleared() {
        super.onCleared()
        curvPipeline?.close()
        curvPipeline = null
    }
}
