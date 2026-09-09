package org.curv.palmapp.view

import android.view.HapticFeedbackConstants
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.platform.LocalView
import androidx.lifecycle.viewmodel.compose.viewModel
import org.curv.overlay.camera.CameraSession
import org.curv.overlay.hud.CameraOverlayHost
import org.curv.overlay.palm.PalmSilhouette
import org.curv.palmapp.controller.PalmAnalysisViewModel

/**
 * Palm scanner screen built on the shared camera overlay host.
 */
@Composable
fun MainScannerScreen(
    viewModel: PalmAnalysisViewModel = viewModel()
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val view = LocalView.current

    val cameraSession = remember { CameraSession(context) }
    var previewView by remember { mutableStateOf<PreviewView?>(null) }

    val guidanceState by viewModel.guidanceState.collectAsState()
    val metrics by viewModel.qualityMetrics.collectAsState()
    val isAnalyzing by viewModel.isAnalyzing.collectAsState()
    val analysisResult by viewModel.analysisResult.collectAsState()
    val detectedSplines by viewModel.detectedSplines.collectAsState()
    val showResultSheet by viewModel.showResultSheet.collectAsState()
    val currentLanguage by viewModel.currentLanguage.collectAsState()

    LaunchedEffect(previewView) {
        val pv = previewView ?: return@LaunchedEffect
        cameraSession.onFrameAnalysis = { imageProxy ->
            viewModel.processFrame(
                imageProxy = imageProxy,
                onTriggerHaptic = {
                    view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                },
                onPerformCapture = {
                    viewModel.triggerCapture(cameraSession) {
                        view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                    }
                }
            )
        }
        cameraSession.startCamera(
            lifecycleOwner = lifecycleOwner,
            surfaceProvider = pv.surfaceProvider,
            onSuccess = {},
            onError = {}
        )
    }

    DisposableEffect(Unit) {
        onDispose { cameraSession.shutdown() }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        CameraOverlayHost(
            profile = viewModel.profile,
            guidanceState = guidanceState,
            metrics = metrics,
            isAnalyzing = isAnalyzing,
            onPreviewViewCreated = { previewView = it },
            onManualCapture = {
                viewModel.triggerCapture(cameraSession) {
                    view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                }
            }
        ) {
            PalmSilhouette(isReady = guidanceState.isReady)
        }

        if (detectedSplines.isNotEmpty()) {
            CreaseSplineCanvas(splines = detectedSplines)
        }

        if (showResultSheet) {
            ReadingBottomSheet(
                result = analysisResult,
                splines = detectedSplines,
                selectedLanguage = currentLanguage,
                onLanguageChange = { viewModel.setLanguage(it) },
                onRetake = { viewModel.retake() },
                onDismiss = { viewModel.retake() }
            )
        }
    }
}
