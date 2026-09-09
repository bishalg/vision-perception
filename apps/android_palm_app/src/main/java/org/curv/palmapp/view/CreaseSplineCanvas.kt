package org.curv.palmapp.view

import android.graphics.PointF
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import org.curv.palmapp.model.CreaseSpline

/**
 * Jetpack Compose Canvas rendering anti-aliased Catmull-Rom vector crease curves.
 */
@Composable
fun CreaseSplineCanvas(
    splines: List<CreaseSpline>,
    modifier: Modifier = Modifier
) {
    Canvas(modifier = modifier.fillMaxSize()) {
        val canvasWidth = size.width
        val canvasHeight = size.height

        for (spline in splines) {
            if (spline.points.size < 2) continue

            val canvasPoints = spline.points.map { pt ->
                PointF(pt.x * canvasWidth, pt.y * canvasHeight)
            }

            val path = buildCatmullRomPath(canvasPoints)
            val color = parseHexColor(spline.lineType.hexColor)

            // 1. Outer neon glow layer
            drawPath(
                path = path,
                color = color.copy(alpha = 0.4f),
                style = Stroke(
                    width = 14f,
                    cap = StrokeCap.Round,
                    join = StrokeJoin.Round
                )
            )

            // 2. Crisp core vector spline
            drawPath(
                path = path,
                color = color,
                style = Stroke(
                    width = 5f,
                    cap = StrokeCap.Round,
                    join = StrokeJoin.Round
                )
            )
        }
    }
}

private fun buildCatmullRomPath(points: List<PointF>): Path {
    val path = Path()
    if (points.size < 2) return path

    path.moveTo(points[0].x, points[0].y)
    if (points.size == 2) {
        path.lineTo(points[1].x, points[1].y)
        return path
    }

    for (i in 0 until points.size - 1) {
        val p0 = if (i > 0) points[i - 1] else points[i]
        val p1 = points[i]
        val p2 = points[i + 1]
        val p3 = if (i + 2 < points.size) points[i + 2] else p2

        val cp1x = p1.x + (p2.x - p0.x) / 6f
        val cp1y = p1.y + (p2.y - p0.y) / 6f
        val cp2x = p2.x - (p3.x - p1.x) / 6f
        val cp2y = p2.y - (p3.y - p1.y) / 6f

        path.cubicTo(cp1x, cp1y, cp2x, cp2y, p2.x, p2.y)
    }

    return path
}

private fun parseHexColor(hex: String): Color {
    val clean = hex.removePrefix("#")
    val colorLong = clean.toLongOrNull(16) ?: 0xFFFFFFFF
    return if (clean.length == 6) {
        Color(0xFF000000 or colorLong)
    } else {
        Color(colorLong)
    }
}
