package org.curv.overlay.hud

import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.camera.CameraPreview
import org.curv.overlay.guidance.GuidanceState
import org.curv.overlay.guidance.QualityMetrics
import org.curv.overlay.profile.OverlayProfile

@Composable
fun CameraOverlayHost(
    profile: OverlayProfile,
    guidanceState: GuidanceState,
    metrics: QualityMetrics,
    isAnalyzing: Boolean,
    onPreviewViewCreated: (PreviewView) -> Unit,
    onManualCapture: () -> Unit,
    statusMessageOverride: String? = null,
    deviceRollDeg: Float? = null,
    bottomLeading: @Composable () -> Unit = {},
    bottomTrailing: @Composable () -> Unit = {},
    overlayContent: @Composable () -> Unit
) {
    Box(modifier = Modifier.fillMaxSize().background(Color.Black)) {
        CameraPreview(onPreviewViewCreated = onPreviewViewCreated)

        LiquidGlassHUD(
            profile = profile,
            guidanceState = guidanceState,
            metrics = metrics,
            onManualCapture = onManualCapture,
            statusMessageOverride = statusMessageOverride,
            deviceRollDeg = deviceRollDeg,
            bottomLeading = bottomLeading,
            bottomTrailing = bottomTrailing,
            overlayContent = overlayContent
        )

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
                imageVector = Icons.Filled.AutoAwesome,
                contentDescription = null,
                tint = OverlayEmerald,
                modifier = Modifier.size(16.dp)
            )
            Text(text = profile.brand, color = Color.White.copy(alpha = 0.7f), fontSize = 13.sp)
            Text(text = "/", color = Color.White.copy(alpha = 0.4f), fontSize = 13.sp)
            Text(
                text = profile.productTitle,
                color = Color.White,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold
            )
        }

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
                            text = profile.copy.processing,
                            color = Color.White,
                            fontSize = 15.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                    }
                }
            }
        }
    }
}
