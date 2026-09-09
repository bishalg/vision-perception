package org.curv.overlay.palm

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import org.curv.overlay.hud.OverlayEmerald
import androidx.compose.ui.graphics.Color

@Composable
fun PalmSilhouette(isReady: Boolean) {
    val stroke = if (isReady) OverlayEmerald.copy(alpha = 0.55f) else Color.White.copy(alpha = 0.28f)
    val fill = if (isReady) OverlayEmerald.copy(alpha = 0.12f) else Color.White.copy(alpha = 0.06f)

    Canvas(modifier = Modifier.fillMaxSize().padding(horizontal = 12.dp, vertical = 10.dp)) {
        val path = fullHandPath(size.width, size.height)
        drawPath(path, color = fill, style = Fill)
        drawPath(path, color = stroke, style = Stroke(width = 3f))
    }
}

private fun fullHandPath(w: Float, h: Float): Path {
    fun p(x: Float, y: Float) = Offset(x * w, y * h)
    return Path().apply {
        moveTo(p(0.28f, 0.92f).x, p(0.28f, 0.92f).y)
        cubicTo(p(0.38f, 0.98f).x, p(0.38f, 0.98f).y, p(0.62f, 0.98f).x, p(0.62f, 0.98f).y, p(0.72f, 0.92f).x, p(0.72f, 0.92f).y)
        cubicTo(p(0.80f, 0.86f).x, p(0.80f, 0.86f).y, p(0.82f, 0.72f).x, p(0.82f, 0.72f).y, p(0.78f, 0.62f).x, p(0.78f, 0.62f).y)
        cubicTo(p(0.84f, 0.50f).x, p(0.84f, 0.50f).y, p(0.90f, 0.32f).x, p(0.90f, 0.32f).y, p(0.86f, 0.18f).x, p(0.86f, 0.18f).y)
        cubicTo(p(0.85f, 0.12f).x, p(0.85f, 0.12f).y, p(0.80f, 0.12f).x, p(0.80f, 0.12f).y, p(0.78f, 0.16f).x, p(0.78f, 0.16f).y)
        cubicTo(p(0.76f, 0.24f).x, p(0.76f, 0.24f).y, p(0.74f, 0.34f).x, p(0.74f, 0.34f).y, p(0.74f, 0.42f).x, p(0.74f, 0.42f).y)
        cubicTo(p(0.74f, 0.28f).x, p(0.74f, 0.28f).y, p(0.74f, 0.16f).x, p(0.74f, 0.16f).y, p(0.70f, 0.10f).x, p(0.70f, 0.10f).y)
        cubicTo(p(0.68f, 0.04f).x, p(0.68f, 0.04f).y, p(0.64f, 0.04f).x, p(0.64f, 0.04f).y, p(0.62f, 0.08f).x, p(0.62f, 0.08f).y)
        cubicTo(p(0.60f, 0.16f).x, p(0.60f, 0.16f).y, p(0.60f, 0.28f).x, p(0.60f, 0.28f).y, p(0.60f, 0.38f).x, p(0.60f, 0.38f).y)
        cubicTo(p(0.60f, 0.22f).x, p(0.60f, 0.22f).y, p(0.60f, 0.10f).x, p(0.60f, 0.10f).y, p(0.56f, 0.04f).x, p(0.56f, 0.04f).y)
        cubicTo(p(0.54f, 0.00f).x, p(0.54f, 0.00f).y, p(0.50f, 0.00f).x, p(0.50f, 0.00f).y, p(0.48f, 0.04f).x, p(0.48f, 0.04f).y)
        cubicTo(p(0.46f, 0.12f).x, p(0.46f, 0.12f).y, p(0.46f, 0.24f).x, p(0.46f, 0.24f).y, p(0.46f, 0.36f).x, p(0.46f, 0.36f).y)
        cubicTo(p(0.46f, 0.22f).x, p(0.46f, 0.22f).y, p(0.46f, 0.14f).x, p(0.46f, 0.14f).y, p(0.42f, 0.10f).x, p(0.42f, 0.10f).y)
        cubicTo(p(0.40f, 0.04f).x, p(0.40f, 0.04f).y, p(0.36f, 0.04f).x, p(0.36f, 0.04f).y, p(0.34f, 0.12f).x, p(0.34f, 0.12f).y)
        cubicTo(p(0.32f, 0.22f).x, p(0.32f, 0.22f).y, p(0.32f, 0.34f).x, p(0.32f, 0.34f).y, p(0.34f, 0.44f).x, p(0.34f, 0.44f).y)
        cubicTo(p(0.28f, 0.46f).x, p(0.28f, 0.46f).y, p(0.22f, 0.44f).x, p(0.22f, 0.44f).y, p(0.18f, 0.48f).x, p(0.18f, 0.48f).y)
        cubicTo(p(0.12f, 0.50f).x, p(0.12f, 0.50f).y, p(0.08f, 0.52f).x, p(0.08f, 0.52f).y, p(0.10f, 0.58f).x, p(0.10f, 0.58f).y)
        cubicTo(p(0.10f, 0.64f).x, p(0.10f, 0.64f).y, p(0.14f, 0.68f).x, p(0.14f, 0.68f).y, p(0.16f, 0.66f).x, p(0.16f, 0.66f).y)
        cubicTo(p(0.20f, 0.64f).x, p(0.20f, 0.64f).y, p(0.24f, 0.66f).x, p(0.24f, 0.66f).y, p(0.28f, 0.72f).x, p(0.28f, 0.72f).y)
        cubicTo(p(0.26f, 0.80f).x, p(0.26f, 0.80f).y, p(0.24f, 0.88f).x, p(0.24f, 0.88f).y, p(0.28f, 0.92f).x, p(0.28f, 0.92f).y)
        close()
    }
}
