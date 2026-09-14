package org.curv.palmapp.view

import android.view.HapticFeedbackConstants
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
import org.curv.overlay.palm.RegionOverlayView
import org.curv.palmapp.controller.PalmAnalysisViewModel
import org.curv.palmapp.model.LocalizedStrings

/**
 * Palm scanner screen — 1:1 parity with iOS MainScannerView.swift.
 * Live guidance + sci-fi region overlay + corridors toggle + thumbnail
 * (like iOS Camera) + full-screen editable review + Samudrika reading sheet.
 * Camera feed pauses during review & reading to conserve battery.
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

    // Corridor guides ship default-off (spec §4); persists across launches.
    val prefs = remember(context) {
        context.getSharedPreferences("palm_prefs", android.content.Context.MODE_PRIVATE)
    }
    var showCorridors by remember {
        mutableStateOf(prefs.getBoolean("showPalmCorridors", false))
    }

    val guidanceState by viewModel.guidanceState.collectAsState()
    val metrics by viewModel.qualityMetrics.collectAsState()
    val isAnalyzing by viewModel.isAnalyzing.collectAsState()
    val detectedLandmarks by viewModel.detectedLandmarks.collectAsState()
    val capturedBitmap by viewModel.capturedBitmap.collectAsState()
    val captureLandmarks by viewModel.captureLandmarks.collectAsState()
    val detectedSplines by viewModel.detectedSplines.collectAsState()
    val predictionReadySplines by viewModel.predictionReadySplines.collectAsState()
    val overlaySource by viewModel.overlaySource.collectAsState()
    val showResultViewer by viewModel.showResultViewer.collectAsState()
    val showAnalysisReading by viewModel.showAnalysisReading.collectAsState()
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

    // Camera session lifecycle: stop feed during review & reading (battery save).
    LaunchedEffect(showResultViewer, showAnalysisReading) {
        if (showResultViewer || showAnalysisReading) cameraSession.pause()
        else cameraSession.resume()
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
                    isHandDetected = detectedLandmarks.isNotEmpty(),
                    onSelect = { viewModel.selectHandSide(it) },
                    modifier = Modifier.padding(top = 14.dp)
                )
            }
        }

        // Optional corridors toggle (bottom-left, above telemetry chips).
        // Native toggle chip: selected state + checkmark come from Material3.
        if (!showResultViewer && detectedLandmarks.isNotEmpty()) {
            FilterChip(
                selected = showCorridors,
                onClick = {
                    showCorridors = !showCorridors
                    prefs.edit().putBoolean("showPalmCorridors", showCorridors).apply()
                },
                label = { Text(LocalizedStrings.tr("camera.corridors", currentLanguage)) },
                leadingIcon = {
                    if (showCorridors) {
                        Icon(
                            Icons.Filled.Check,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp)
                        )
                    }
                },
                modifier = Modifier.align(Alignment.BottomStart)
                    .padding(start = 22.dp, bottom = 134.dp)
            )
        }

        // Live landmark skeleton while hand is detected.
        if (detectedLandmarks.isNotEmpty() && !showResultViewer) {
            HandOutlineOverlay(landmarks = detectedLandmarks, modifier = Modifier.fillMaxSize())
        }

        // Live sci-fi region overlay (mounts + optional corridors).
        // Live landmarks are already in image space and anatomically invariant:
        // no mirroring (parity with iOS MainScannerView shouldMirror).
        if (detectedLandmarks.isNotEmpty() && !showResultViewer) {
            RegionOverlayView(
                landmarks = detectedLandmarks,
                isReady = guidanceState.isReady,
                showCorridors = showCorridors,
                mirror = false,
                imageRect = null,
                modifier = Modifier.fillMaxSize()
            )
        }

        // Bottom-left thumbnail (iOS Camera style).
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

        // Phase 2: crease-line verification screen.
        if (showResultViewer) {
            CaptureResultView(
                bitmap = capturedBitmap,
                landmarks = captureLandmarks,
                splines = detectedSplines,
                selectedLanguage = currentLanguage,
                overlaySource = overlaySource,
                handSide = if (selectedHandSide != HandSide.AUTO) selectedHandSide else detectedHandSide,
                padDepths = viewModel.padDepths,
                onSplinesChange = { viewModel.updateSplines(it) },
                onLanguageChange = { viewModel.setLanguage(it) },
                onRetake = { viewModel.retake() },
                onConfirmAdjustedLines = { viewModel.confirmAdjustedLinesForPrediction() },
                onDismiss = { viewModel.dismissResultViewer() },
                onProceedToAnalysis = { viewModel.proceedToAnalysis() },
                modifier = Modifier.fillMaxSize()
            )
        }

        // Phase 3: dedicated Samudrika reading sheet.
        if (showAnalysisReading) {
            ReadingBottomSheet(
                result = analysisResult,
                splines = if (predictionReadySplines.isEmpty()) detectedSplines else predictionReadySplines,
                selectedLanguage = currentLanguage,
                handSide = if (selectedHandSide != HandSide.AUTO) selectedHandSide else detectedHandSide,
                onLanguageChange = { viewModel.setLanguage(it) },
                onRetake = { viewModel.retake() },
                onDismiss = { viewModel.dismissAnalysisReading() }
            )
        }
    }
}
