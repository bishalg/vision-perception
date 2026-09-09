package org.curv.palmapp.view

import android.view.HapticFeedbackConstants
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import org.curv.palmapp.controller.CameraXManager
import org.curv.palmapp.controller.PalmAnalysisViewModel
import org.curv.palmapp.model.LocalizedStrings

/**
 * Main Scanner screen orchestrating the camera preview, HUD guidance overlay,
 * spline drawing layer, and Samudrika reading presentation.
 */
@Composable
fun MainScannerScreen(
    viewModel: PalmAnalysisViewModel = viewModel()
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val view = LocalView.current

    val cameraManager = remember { CameraXManager(context) }
    var previewView by remember { mutableStateOf<PreviewView?>(null) }

    val guidanceState by viewModel.guidanceState.collectAsState()
    val metrics by viewModel.qualityMetrics.collectAsState()
    val isAnalyzing by viewModel.isAnalyzing.collectAsState()
    val analysisResult by viewModel.analysisResult.collectAsState()
    val detectedSplines by viewModel.detectedSplines.collectAsState()
    val showResultSheet by viewModel.showResultSheet.collectAsState()
    val currentLanguage by viewModel.currentLanguage.collectAsState()

    // Setup CameraX pipeline when PreviewView is created
    LaunchedEffect(previewView) {
        val pv = previewView ?: return@LaunchedEffect
        cameraManager.onFrameAnalysis = { imageProxy ->
            viewModel.processFrame(
                imageProxy = imageProxy,
                onTriggerHaptic = {
                    view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                },
                onPerformCapture = {
                    viewModel.triggerCapture(cameraManager) {
                        view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                    }
                }
            )
        }
        cameraManager.startCamera(
            lifecycleOwner = lifecycleOwner,
            surfaceProvider = pv.surfaceProvider,
            onSuccess = {},
            onError = {}
        )
    }

    DisposableEffect(Unit) {
        onDispose {
            cameraManager.shutdown()
        }
    }

    Box(modifier = Modifier.fillMaxSize().background(Color.Black)) {
        // 1. Fullscreen Camera Preview
        CameraPreview(
            onPreviewViewCreated = { pv ->
                previewView = pv
            }
        )

        // 2. Vector Crease Splines Canvas
        if (detectedSplines.isNotEmpty()) {
            CreaseSplineCanvas(splines = detectedSplines)
        }

        // 3. Liquid Glass HUD Overlay
        LiquidGlassHUD(
            guidanceState = guidanceState,
            metrics = metrics,
            onManualCapture = {
                viewModel.triggerCapture(cameraManager) {
                    view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                }
            }
        )

        // 4. Breadcrumb Header (Liquid Glass style)
        Row(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 16.dp)
                .clip(CircleShape)
                .background(Color.Black.copy(alpha = 0.4f))
                .border(1.dp, Color.White.copy(alpha = 0.2f), CircleShape)
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Icon(
                imageVector = Icons.Default.AutoAwesome,
                contentDescription = null,
                tint = Color(0xFF32D74B),
                modifier = Modifier.size(16.dp)
            )
            Text(
                text = "AstroFusion",
                color = Color.White.copy(alpha = 0.7f),
                fontSize = 13.sp
            )
            Text(
                text = "/",
                color = Color.White.copy(alpha = 0.4f),
                fontSize = 13.sp
            )
            Text(
                text = "Palm Perception",
                color = Color.White,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold
            )
        }

        // 5. Analyzing Progress Overlay
        if (isAnalyzing) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.6f)),
                contentAlignment = Alignment.Center
            ) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(24.dp))
                        .background(Color.White.copy(alpha = 0.1f))
                        .border(1.dp, Color.White.copy(alpha = 0.25f), RoundedCornerShape(24.dp))
                        .padding(28.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        CircularProgressIndicator(color = Color.White)
                        Text(
                            text = LocalizedStrings.tr("status.processing", currentLanguage),
                            color = Color.White,
                            fontSize = 15.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                    }
                }
            }
        }

        // 6. Bottom Sheet Results Presentation
        if (showResultSheet) {
            ReadingBottomSheet(
                result = analysisResult,
                splines = detectedSplines,
                selectedLanguage = currentLanguage,
                onLanguageChange = { newLang ->
                    viewModel.setLanguage(newLang)
                },
                onRetake = {
                    viewModel.retake()
                },
                onDismiss = {
                    viewModel.retake()
                }
            )
        }
    }
}
