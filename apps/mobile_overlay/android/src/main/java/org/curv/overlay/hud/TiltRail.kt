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
    val label = if (isLeft) "CAM %.0f°".format(deviceRollDeg) else "Δ %.0f°".format(angleDeg)
    val isOk = kotlin.math.abs(angleDeg) <= if (isLeft) 8f else 10f
    val accent = if (isOk) OverlayEmerald else OverlayAmber

    Column(
        modifier = modifier
            .clip(Capsule)
            .background(Color.Black.copy(alpha = 0.38f))
            .border(1.dp, Color.White.copy(alpha = 0.14f), Capsule)
            .padding(horizontal = 6.dp, vertical = 7.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        Text(
            text = if (isLeft) "CAM" else "HAND",
            color = Color.White.copy(alpha = 0.55f),
            fontSize = 7.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace
        )
        Box(modifier = Modifier.size(width = 22.dp, height = 38.dp), contentAlignment = Alignment.Center) {
            Box(
                modifier = Modifier
                    .size(width = 2.dp, height = 34.dp)
                    .clip(Capsule)
                    .background(Color.White.copy(alpha = 0.12f))
            )
            Box(
                modifier = Modifier
                    .size(width = 2.5.dp, height = 28.dp)
                    .clip(Capsule)
                    .background(accent)
                    .rotate(angleDeg)
            )
            Box(
                modifier = Modifier
                    .size(3.dp)
                    .clip(CircleShape)
                    .background(Color.White.copy(alpha = 0.9f))
            )
        }
        Text(
            text = label,
            color = accent,
            fontSize = 8.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace
        )
    }
}
