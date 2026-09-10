package org.curv.overlay.palm

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import org.curv.overlay.hud.OverlayEmerald

@Composable
fun PalmSilhouette(
    isReady: Boolean,
    handSide: HandSide = HandSide.AUTO,
    detectedIsLeft: Boolean? = null
) {
    val mirror = when (handSide) {
        HandSide.LEFT -> true
        HandSide.RIGHT -> false
        HandSide.AUTO -> detectedIsLeft ?: false
    }
    val fillBase = if (isReady) OverlayEmerald.copy(alpha = 0.13f) else Color.White.copy(alpha = 0.05f)
    val strokeC = if (isReady) OverlayEmerald else Color.White.copy(alpha = 0.38f)

    Canvas(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 14.dp, vertical = 10.dp)
            .graphicsLayer(scaleX = if (mirror) -1f else 1f)
    ) {
        val w = size.width
        val h = size.height
        val path = fullHandPath(w, h)
        // outer glow
        drawPath(path, brush = Brush.radialGradient(listOf(fillBase, Color.Transparent)), style = Fill)
        // crisp fill with subtle top highlight
        drawPath(path, brush = Brush.linearGradient(listOf(Color.White.copy(alpha = if (isReady) 0.14f else 0.08f), fillBase)), style = Fill)
        // faint crease hints
        drawPath(hintCreases(w, h), color = Color.White.copy(alpha = 0.14f), style = Stroke(width = 1f, pathEffect = androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(10f, 10f), 0f)))
        // stroke
        drawPath(path, color = strokeC, style = Stroke(width = if (isReady) 6f else 4.5f))
        drawPath(path, color = Color.White.copy(alpha = 0.72f), style = Stroke(width = 1.2f))
        // wrist tick
        drawPath(wristBrace(w, h), color = Color.White.copy(alpha = 0.24f), style = Stroke(width = 2f, pathEffect = androidx.compose.ui.graphics.PathEffect.dashPathEffect(floatArrayOf(6f, 8f), 0f)))
        // fingertip dots
        val glowC = if (isReady) Color.White.copy(alpha = 0.92f) else Color.White.copy(alpha = 0.62f)
        for (pt in fingertipPoints(w, h) + mcpPoints(w, h)) {
            drawCircle(color = glowC, radius = 3.4f, center = pt)
        }
        drawCircle(color = glowC, radius = 4.2f, center = Offset(w*0.5f, h*0.56f))
    }
}

private fun fingertipPoints(w: Float, h: Float): List<Offset> = listOf(
    Offset(0.12f*w, 0.17f*h), Offset(0.30f*w, 0.09f*h), Offset(0.50f*w, 0.04f*h), Offset(0.70f*w, 0.08f*h), Offset(0.87f*w, 0.18f*h)
)
private fun mcpPoints(w: Float, h: Float): List<Offset> = listOf(
    Offset(0.32f*w, 0.42f*h), Offset(0.42f*w, 0.36f*h), Offset(0.54f*w, 0.34f*h), Offset(0.66f*w, 0.36f*h), Offset(0.76f*w, 0.42f*h)
)
private fun hintCreases(w: Float, h: Float): Path = Path().apply {
    moveTo(0.22f*w, 0.42f*h); cubicTo(0.32f*w,0.34f*h, 0.64f*w,0.34f*h, 0.78f*w,0.40f*h)
    moveTo(0.20f*w, 0.52f*h); cubicTo(0.34f*w,0.48f*h, 0.62f*w,0.48f*h, 0.76f*w,0.51f*h)
    moveTo(0.40f*w, 0.46f*h); cubicTo(0.32f*w,0.56f*h, 0.30f*w,0.68f*h, 0.42f*w,0.74f*h)
}
private fun wristBrace(w: Float, h: Float): Path = Path().apply {
    moveTo(0.28f*w, 0.88f*h); cubicTo(0.38f*w,0.90f*h, 0.62f*w,0.90f*h, 0.72f*w,0.88f*h)
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
