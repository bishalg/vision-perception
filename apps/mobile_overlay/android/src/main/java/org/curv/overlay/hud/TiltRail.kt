package org.curv.overlay.hud

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.Capsule
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Small vertical tick on left/right edge.
 * left  = CAM (device gyro roll)
 * right = Δ hand-to-camera (handRoll - deviceRoll)
 */
@Composable
fun TiltRail(
    deviceRollDeg: Float,
    handRollDeg: Float,
    isLeft: Boolean,
    modifier: Modifier = Modifier
) {
    val angleDeg = if (isLeft) deviceRollDeg else {
        var d = handRollDeg - deviceRollDeg
        while (d > 180f) d -= 360f
        while (d < -180f) d += 360f
        d
    }
    val degreeText = "%.0f°".format(kotlin.math.abs(angleDeg))
    val isOk = kotlin.math.abs(angleDeg) <= if (isLeft) 8f else 10f
    val accent = if (isOk) OverlayEmerald else OverlayAmber

    Column(
        modifier = modifier.padding(horizontal = 4.dp, vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        // Icon indicator: phone/cam symbol on left, Delta (Δ) on right
        Text(
            text = if (isLeft) "⌁" else "Δ",
            color = if (isOk) Color.White.copy(alpha = 0.8f) else OverlayAmber,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold
        )
        Box(modifier = Modifier.size(width = 20.dp, height = 32.dp), contentAlignment = Alignment.Center) {
            Box(
                modifier = Modifier
                    .size(width = 2.dp, height = 30.dp)
                    .clip(Capsule)
                    .background(Color.White.copy(alpha = 0.20f))
            )
            Box(
                modifier = Modifier
                    .size(width = 2.5.dp, height = 24.dp)
                    .clip(Capsule)
                    .background(accent)
                    .rotate(angleDeg)
            )
            Box(
                modifier = Modifier
                    .size(3.dp)
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.95f))
            )
        }
        Text(
            text = degreeText,
            color = accent,
            fontSize = 9.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace
        )
    }
}
