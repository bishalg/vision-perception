package org.curv.vastuapp.view

import android.view.HapticFeedbackConstants
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import org.curv.overlay.camera.CameraSession
import org.curv.overlay.hud.CameraOverlayHost
import org.curv.overlay.vastu.VastuMandala
import org.curv.vastuapp.controller.VastuAnalysisViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScannerScreen(
    viewModel: VastuAnalysisViewModel = viewModel()
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val view = LocalView.current

    val cameraSession = remember { CameraSession(context) }
    var previewView by remember { mutableStateOf<PreviewView?>(null) }

    val guidanceState by viewModel.guidanceState.collectAsState()
    val metrics by viewModel.qualityMetrics.collectAsState()
    val isAnalyzing by viewModel.isAnalyzing.collectAsState()
    val analysisJSON by viewModel.analysisJSON.collectAsState()
    val showResultSheet by viewModel.showResultSheet.collectAsState()

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
            surfaceProvider = pv.surfaceProvider
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
            VastuMandala(isReady = guidanceState.isReady)
        }

        if (showResultSheet) {
            ModalBottomSheet(onDismissRequest = { viewModel.retake() }) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(20.dp)
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    Text(
                        text = "Vastu Mandala Analysis",
                        color = Color.White,
                        fontSize = 20.sp,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = analysisJSON ?: """{"status":"no_evidence"}""",
                        color = Color.White.copy(alpha = 0.85f),
                        fontSize = 11.sp,
                        fontFamily = FontFamily.Monospace,
                        modifier = Modifier
                            .clip(RoundedCornerShape(16.dp))
                            .background(Color.White.copy(alpha = 0.06f))
                            .padding(12.dp)
                    )
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(18.dp))
                            .background(Color.White.copy(alpha = 0.12f))
                            .border(1.dp, Color.White.copy(alpha = 0.25f), RoundedCornerShape(18.dp))
                            .clickable { viewModel.retake() }
                            .padding(vertical = 14.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("Retake", color = Color.White, fontWeight = FontWeight.SemiBold)
                    }
                    Spacer(modifier = Modifier.height(24.dp))
                }
            }
        }
    }
}
