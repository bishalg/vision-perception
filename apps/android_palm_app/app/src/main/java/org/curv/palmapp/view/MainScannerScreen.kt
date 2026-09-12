package org.curv.palmapp.view

import android.view.HapticFeedbackConstants
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import org.curv.overlay.camera.CameraSession
import org.curv.overlay.hud.CameraOverlayHost
import org.curv.overlay.palm.HandSide
import org.curv.overlay.palm.HandSideToggle
import org.curv.overlay.palm.PalmSilhouette
import org.curv.palmapp.controller.PalmAnalysisViewModel

/**
 * Palm scanner screen — parity with iOS MainScannerView.swift.
 * Live guidance + thumbnail (like iOS Camera) + full-screen editable review.
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
    val detectedLandmarks by viewModel.detectedLandmarks.collectAsState()
    val capturedBitmap by viewModel.capturedBitmap.collectAsState()
    val captureLandmarks by viewModel.captureLandmarks.collectAsState()
    val detectedSplines by viewModel.detectedSplines.collectAsState()
    val overlaySource by viewModel.overlaySource.collectAsState()
    val showResultViewer by viewModel.showResultViewer.collectAsState()
    val showResultSheet by viewModel.showResultSheet.collectAsState()
    val analysisResult by viewModel.analysisResult.collectAsState()
    val currentLanguage by viewModel.currentLanguage.collectAsState()
    val statusOverride by viewModel.statusMessageOverride.collectAsState()
    val selectedHandSide by viewModel.selectedHandSide.collectAsState()
    val detectedHandSide by viewModel.detectedHandSide.collectAsState()

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
            statusMessageOverride = statusOverride,
            onPreviewViewCreated = { previewView = it },
            onManualCapture = {
                viewModel.triggerCapture(cameraSession) {
                    view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                }
            }
        ) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
                PalmSilhouette(
                    isReady = guidanceState.isReady,
                    handSide = selectedHandSide,
                    detectedIsLeft = detectedHandSide == HandSide.LEFT,
                    isHandDetected = detectedLandmarks.isNotEmpty()
                )

                // Center-top inside the camera box reticle frame
                HandSideToggle(
                    selection = selectedHandSide,
                    autoDetected = detectedHandSide,
                    onSelect = { viewModel.selectHandSide(it) },
                    modifier = Modifier.padding(top = 14.dp)
                )
            }
        }

        // Live landmark skeleton (same dots as result view)
        if (detectedLandmarks.isNotEmpty() && !showResultViewer) {
            HandOutlineOverlay(landmarks = detectedLandmarks, modifier = Modifier.fillMaxSize())
        }

        // Bottom-left thumbnail (iOS Camera style)
        if (capturedBitmap != null && !showResultViewer) {
            Box(modifier = Modifier.align(Alignment.BottomStart).padding(start = 22.dp, bottom = 46.dp)) {
                Image(
                    bitmap = capturedBitmap!!.asImageBitmap(),
                    contentDescription = null,
                    modifier = Modifier
                        .size(54.dp)
                        .clip(RoundedCornerShape(8.dp))
                        .border(1.5.dp, Color.White.copy(alpha = 0.85f), RoundedCornerShape(8.dp))
                        .clickable { viewModel.openLastCapture() },
                    contentScale = ContentScale.Crop
                )
            }
        }

        // Full-screen editable review
        if (showResultViewer) {
            CaptureResultView(
                bitmap = capturedBitmap,
                landmarks = captureLandmarks,
                splines = detectedSplines,
                selectedLanguage = currentLanguage,
                overlaySource = overlaySource,
                handSide = if (selectedHandSide != HandSide.AUTO) selectedHandSide else detectedHandSide,
                onSplinesChange = { viewModel.updateSplines(it) },
                onLanguageChange = { viewModel.setLanguage(it) },
                onRetake = { viewModel.retake() },
                onConfirmAdjustedLines = { viewModel.confirmAdjustedLinesForPrediction() },
                onDismiss = { viewModel.dismissResultViewer() },
                modifier = Modifier.fillMaxSize()
            )
        } else if (showResultSheet) {
            // Fallback bottom sheet when not in manual-edit viewer (native success still shows sheet)
            // Kept for backwards compat; viewer already covers manual case.
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
