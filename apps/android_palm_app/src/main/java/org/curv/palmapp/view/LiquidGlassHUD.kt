package org.curv.palmapp.view

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PanTool
import androidx.compose.material3.Icon
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
import org.curv.PalmGuidanceState
import org.curv.PalmQualityMetrics

/**
 * Liquid Glass HUD overlay displaying palm framing guide,
 * real-time telemetry pills, and glowing emerald auto-capture aura.
 */
@Composable
fun LiquidGlassHUD(
    guidanceState: PalmGuidanceState,
    metrics: PalmQualityMetrics,
    onManualCapture: () -> Unit,
    modifier: Modifier = Modifier
) {
    val isReady = guidanceState is PalmGuidanceState.Ready

    val emeraldGreen = Color(0xFF32D74B)
    val amberYellow = Color(0xFFFFCC00)
    val statusColor by animateColorAsState(
        targetValue = if (isReady) emeraldGreen else amberYellow,
        label = "statusColor"
    )

    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val screenWidth = maxWidth
        val screenHeight = maxHeight

        // Centered Palm Reticle Guide
        Box(
            modifier = Modifier
                .size(width = screenWidth * 0.76f, height = screenHeight * 0.58f)
                .align(Alignment.Center)
                .shadow(
                    elevation = if (isReady) 24.dp else 0.dp,
                    shape = RoundedCornerShape(36.dp),
                    ambientColor = emeraldGreen,
                    spotColor = emeraldGreen
                )
                .border(
                    width = if (isReady) 3.5.dp else 2.dp,
                    brush = if (isReady) {
                        Brush.linearGradient(listOf(emeraldGreen, Color(0xFF22C55E)))
                    } else {
                        Brush.linearGradient(listOf(Color.White.copy(alpha = 0.6f), Color.White.copy(alpha = 0.2f)))
                    },
                    shape = RoundedCornerShape(36.dp)
                ),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                imageVector = Icons.Default.PanTool,
                contentDescription = null,
                modifier = Modifier.size(90.dp),
                tint = if (isReady) emeraldGreen.copy(alpha = 0.35f) else Color.White.copy(alpha = 0.18f)
            )
        }

        // Top Status Pill Badge
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
                    text = guidanceState.message,
                    color = Color.White,
                    fontSize = 15.sp,
                    fontWeight = FontWeight.SemiBold
                )
            }
        }

        // Bottom Telemetry Bar & Manual Capture Button
        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 36.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Telemetry Chips
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TelemetryChip(
                    title = "Lux",
                    value = String.format("%.0f", metrics.meanLuminance),
                    isValid = metrics.isIlluminationOk
                )
                TelemetryChip(
                    title = "Glare",
                    value = String.format("%.1f%%", metrics.glareRatio * 100f),
                    isValid = metrics.glareRatio <= 0.04f
                )
                TelemetryChip(
                    title = "Tilt",
                    value = String.format("%.0f°", metrics.rollAngleDeg),
                    isValid = metrics.isAngleOk
                )
            }

            // Circular Glass Shutter Button
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
                        .background(if (isReady) emeraldGreen else Color.White)
                        .shadow(
                            elevation = 8.dp,
                            shape = CircleShape,
                            ambientColor = if (isReady) emeraldGreen else Color.White
                        )
                )
            }
        }
    }
}

@Composable
private fun TelemetryChip(
    title: String,
    value: String,
    isValid: Boolean
) {
    val emeraldGreen = Color(0xFF32D74B)
    val amberYellow = Color(0xFFFFCC00)

    Row(
        modifier = Modifier
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.5f))
            .border(0.5.dp, Color.White.copy(alpha = 0.15f), CircleShape)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = title,
            color = Color.White.copy(alpha = 0.7f),
            fontSize = 11.sp
        )
        Text(
            text = value,
            color = if (isValid) emeraldGreen else amberYellow,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace
        )
    }
}
