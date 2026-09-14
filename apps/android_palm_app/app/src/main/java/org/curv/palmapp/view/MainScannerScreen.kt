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

    val scanMode by viewModel.scanMode.collectAsState()
    val orbitSession by viewModel.orbitSession.collectAsState()
    val orbitState by viewModel.orbitState.collectAsState()
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
            },
            bottomLeading = {
                org.curv.overlay.palm.LeftHandSideButton(
                    selection = selectedHandSide,
                    autoDetected = detectedHandSide,
                    onSelect = { viewModel.selectHandSide(it) }
                )
            },
            bottomTrailing = {
                org.curv.overlay.palm.RightHandSideButton(
                    selection = selectedHandSide,
                    autoDetected = detectedHandSide,
                    onSelect = { viewModel.selectHandSide(it) }
                )
            }
        ) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
                PalmSilhouette(
                    isReady = guidanceState.isReady,
                    handSide = selectedHandSide,
                    detectedIsLeft = detectedHandSide == HandSide.LEFT,
                    isHandDetected = detectedLandmarks.isNotEmpty()
                )

                // Center-top inside the camera box reticle frame: Camera Mode Selector
                CameraModeSelector(
                    selectedMode = scanMode,
                    onSelect = { viewModel.setScanMode(it, cameraSession) },
                    modifier = Modifier.padding(top = 14.dp)
                )
            }
        }

        // 3D Orbit Scan Guidance HUD (animated arrows, % progress, angle pips, coaching)
        if (scanMode == PalmScanMode.ORBIT_SCAN && !showResultViewer) {
            org.curv.overlay.guidance.OrbitGuidanceOverlayView(
                session = orbitSession,
                state = orbitState
            )
        }

        // Live landmark skeleton while hand is detected.
        if (detectedLandmarks.isNotEmpty() && !showResultViewer) {
            HandOutlineOverlay(landmarks = detectedLandmarks, modifier = Modifier.fillMaxSize())
        }

        // Live sci-fi region overlay (mounts).
        // Live landmarks are already in image space and anatomically invariant:
        // no mirroring (parity with iOS MainScannerView shouldMirror).
        if (detectedLandmarks.isNotEmpty() && !showResultViewer) {
            RegionOverlayView(
                landmarks = detectedLandmarks,
                isReady = guidanceState.isReady,
                showCorridors = false,
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

/**
 * Top Camera Mode Selector capsule for Android.
 * Replaces the old "AUTO - L R" reticle header.
 */
@Composable
fun CameraModeSelector(
    selectedMode: PalmScanMode,
    onSelect: (PalmScanMode) -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(20.dp))
            .background(Color.Black.copy(alpha = 0.65f))
            .border(0.8.dp, Color.White.copy(alpha = 0.24f), RoundedCornerShape(20.dp))
            .padding(3.dp)
    ) {
        androidx.compose.foundation.layout.Row(
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            PalmScanMode.all.forEach { mode ->
                val isSelected = (mode == selectedMode)
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(16.dp))
                        .background(
                            if (isSelected) {
                                when (mode) {
                                    PalmScanMode.ORBIT_SCAN -> Color.Cyan.copy(alpha = 0.35f)
                                    PalmScanMode.STILL_PHOTO -> Color(0xFF33D64A).copy(alpha = 0.35f)
                                    else -> Color.White.copy(alpha = 0.25f)
                                }
                            } else Color.Transparent
                        )
                        .border(
                            1.dp,
                            if (isSelected) {
                                when (mode) {
                                    PalmScanMode.ORBIT_SCAN -> Color.Cyan
                                    PalmScanMode.STILL_PHOTO -> Color(0xFF33D64A)
                                    else -> Color.White.copy(alpha = 0.7f)
                                }
                            } else Color.Transparent,
                            RoundedCornerShape(16.dp)
                        )
                        .clickable { onSelect(mode) }
                        .padding(horizontal = 9.dp, vertical = 6.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = when (mode) {
                            PalmScanMode.STILL_PHOTO -> "Still"
                            PalmScanMode.ORBIT_SCAN -> "3D Orbit"
                            PalmScanMode.CLOSE_UP_MOSAIC -> "Close-Up"
                            PalmScanMode.FINGERTIP_SCAN -> "Fingertips"
                        },
                        color = if (isSelected) Color.White else Color.White.copy(alpha = 0.70f),
                        fontSize = 11.sp,
                        fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium
                    )
                }
            }
        }
    }
}

