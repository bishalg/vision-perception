package org.curv.overlay.hud

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.curv.overlay.guidance.GuidanceState
import org.curv.overlay.guidance.QualityMetrics
import org.curv.overlay.profile.OverlayProfile
import org.curv.overlay.profile.TelemetryChannel

val OverlayEmerald = Color(0xFF32D74B)
val OverlayAmber = Color(0xFFFFCC00)

@Composable
fun LiquidGlassHUD(
    profile: OverlayProfile,
    guidanceState: GuidanceState,
    metrics: QualityMetrics,
    onManualCapture: () -> Unit,
    modifier: Modifier = Modifier,
    overlayContent: @Composable () -> Unit
) {
    val isReady = guidanceState.isReady
    val statusColor by animateColorAsState(
        targetValue = if (isReady) OverlayEmerald else OverlayAmber,
        label = "statusColor"
    )
    val corner = RoundedCornerShape(profile.reticleCornerRadiusDp.dp)

    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        Box(
            modifier = Modifier
                .size(width = maxWidth * profile.reticleWidthFraction, height = maxHeight * profile.reticleHeightFraction)
                .align(Alignment.Center)
                .shadow(
                    elevation = if (isReady) 24.dp else 0.dp,
                    shape = corner,
                    ambientColor = OverlayEmerald,
                    spotColor = OverlayEmerald
                )
                .border(
                    width = if (isReady) 3.5.dp else 2.dp,
                    brush = if (isReady) {
                        Brush.linearGradient(listOf(OverlayEmerald, Color(0xFF22C55E)))
                    } else {
                        Brush.linearGradient(listOf(Color.White.copy(alpha = 0.6f), Color.White.copy(alpha = 0.2f)))
                    },
                    shape = corner
                ),
            contentAlignment = Alignment.Center
        ) {
            overlayContent()
        }

        Box(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 56.dp)
                .clip(CircleShape)
                .background(Color.Black.copy(alpha = 0.45f))
                .border(1.dp, Color.White.copy(alpha = 0.25f), CircleShape)
                .padding(horizontal = 20.dp, vertical = 10.dp)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(statusColor)
                )
                Text(
                    text = profile.copy.message(guidanceState),
                    color = Color.White,
                    fontSize = 15.sp,
                    fontWeight = FontWeight.SemiBold
                )
            }
        }

        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 36.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                profile.telemetry.forEach { channel ->
                    when (channel) {
                        TelemetryChannel.LUX -> TelemetryChip(
                            "Lux",
                            String.format("%.0f", metrics.meanLuminance),
                            metrics.isIlluminationOk
                        )
                        TelemetryChannel.GLARE -> TelemetryChip(
                            "Glare",
                            String.format("%.1f%%", metrics.glareRatio * 100f),
                            metrics.glareRatio <= profile.thresholds.maxGlareRatio
                        )
                        TelemetryChannel.TILT -> TelemetryChip(
                            "Tilt",
                            String.format("%.0f°", metrics.rollAngleDeg),
                            metrics.isAngleOk
                        )
                    }
                }
            }

            Box(
                modifier = Modifier
                    .size(76.dp)
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.15f))
                    .border(2.dp, Color.White.copy(alpha = 0.6f), CircleShape)
                    .clickable { onManualCapture() },
                contentAlignment = Alignment.Center
            ) {
                Box(
                    modifier = Modifier
                        .size(60.dp)
                        .clip(CircleShape)
                        .background(if (isReady) OverlayEmerald else Color.White)
                        .shadow(
                            elevation = 8.dp,
                            shape = CircleShape,
                            ambientColor = if (isReady) OverlayEmerald else Color.White
                        )
                )
            }
        }
    }
}

@Composable
private fun TelemetryChip(title: String, value: String, isValid: Boolean) {
    Row(
        modifier = Modifier
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.5f))
            .border(0.5.dp, Color.White.copy(alpha = 0.15f), CircleShape)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(text = title, color = Color.White.copy(alpha = 0.7f), fontSize = 11.sp)
        Text(
            text = value,
            color = if (isValid) OverlayEmerald else OverlayAmber,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace
        )
    }
}
