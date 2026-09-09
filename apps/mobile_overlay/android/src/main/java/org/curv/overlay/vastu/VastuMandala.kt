package org.curv.overlay.vastu

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.unit.dp
import org.curv.overlay.hud.OverlayEmerald
import androidx.compose.ui.graphics.Color

@Composable
fun VastuMandala(isReady: Boolean) {
    val stroke = if (isReady) OverlayEmerald.copy(alpha = 0.85f) else Color.White.copy(alpha = 0.45f)
    val label = if (isReady) OverlayEmerald.copy(alpha = 0.9f) else Color.White.copy(alpha = 0.7f)

    Canvas(modifier = Modifier.fillMaxSize().padding(12.dp)) {
        val inset = 8.dp.toPx()
        val gridW = size.width - inset * 2
        val gridH = size.height - inset * 2
        val cellW = gridW / 3f
        val cellH = gridH / 3f

        for (row in 0 until 3) {
            for (col in 0 until 3) {
                val zone = VastuZoneInfo.mandalaGrid[row][col]
                val left = inset + col * cellW
                val top = inset + row * cellH
                drawRoundRect(
                    color = stroke,
                    topLeft = Offset(left + 4, top + 4),
                    size = Size(cellW - 8, cellH - 8),
                    cornerRadius = androidx.compose.ui.geometry.CornerRadius(8f, 8f),
                    style = Stroke(width = if (zone.key == "brahmasthan") 3f else 1.5f)
                )
                drawContext.canvas.nativeCanvas.apply {
                    val paint = android.graphics.Paint().apply {
                        color = android.graphics.Color.argb(
                            (label.alpha * 255).toInt(),
                            (label.red * 255).toInt(),
                            (label.green * 255).toInt(),
                            (label.blue * 255).toInt()
                        )
                        textAlign = android.graphics.Paint.Align.CENTER
                        isAntiAlias = true
                        textSize = 22f
                    }
                    drawText(zone.name, left + cellW / 2f, top + cellH / 2f, paint)
                }
            }
        }
    }
}
