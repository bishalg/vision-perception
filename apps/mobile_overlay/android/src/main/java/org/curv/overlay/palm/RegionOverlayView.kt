package org.curv.overlay.palm

import android.graphics.PointF
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import org.curv.overlay.hud.OverlayEmerald

/**
 * Live sci-fi region overlay: dotted mount outlines + optional line-corridor
 * guides, drawn from the 21 ML Kit landmarks. Dim by design — UI feel only,
 * never a detection claim.
 *
 * Direct port of iOS CurvPalmOverlay/RegionOverlayView.swift.
 */
@Composable
fun RegionOverlayView(
    landmarks: List<PointF>,
    isReady: Boolean,
    showCorridors: Boolean = false,
    mirror: Boolean = false,
    imageRect: Rect? = null,
    modifier: Modifier = Modifier
) {
    // EMA smoothing against landmark jitter (matches iOS 0.35 factor).
    var smoothed by remember { mutableStateOf<List<PointF>>(emptyList()) }
    LaunchedEffect(landmarks) {
        if (landmarks.size < 21) {
            if (smoothed.size >= 21) smoothed = emptyList()
        } else if (smoothed.size == landmarks.size) {
            val a = 0.35f
            smoothed = smoothed.zip(landmarks) { prev, cur ->
                PointF(prev.x + (cur.x - prev.x) * a, prev.y + (cur.y - prev.y) * a)
            }
        } else {
            smoothed = landmarks
        }
    }

    Canvas(modifier = modifier.fillMaxSize()) {
        val pts = if (smoothed.size >= 21) smoothed else landmarks
        val regions = PalmRegionGeometry.regions(from = pts) ?: return@Canvas
        val rect = imageRect ?: Rect(Offset.Zero, size)
        fun map(p: PointF): Offset {
            var x = rect.left + p.x * rect.width
            val y = rect.top + p.y * rect.height
            if (mirror) x = rect.right - (x - rect.left)
            return Offset(x, y)
        }

        val base: Color = if (isReady) OverlayEmerald else Color.White
        val mountStroke = base.copy(alpha = if (isReady) 0.55f else 0.34f)
        val mountFill = base.copy(alpha = 0.07f)
        val corridorStroke = base.copy(alpha = if (isReady) 0.5f else 0.3f)
        val echoStroke = base.copy(alpha = if (isReady) 0.22f else 0.14f)

        for ((_, poly) in regions.mounts) {
            val mapped = poly.map(::map)
            if (mapped.size < 3) continue
            val path = Path().apply {
                moveTo(mapped[0].x, mapped[0].y)
                for (pt in mapped.drop(1)) lineTo(pt.x, pt.y)
                close()
            }
            drawPath(path, mountFill)
            drawPath(
                path, mountStroke,
                style = Stroke(
                    width = 1.5f, cap = StrokeCap.Round, join = StrokeJoin.Round,
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(4f, 10f), 0f)
                )
            )
        }

        if (showCorridors) {
            for ((_, center, echoes) in regions.corridors) {
                for (echo in echoes) {
                    drawPath(
                        polyline(echo.map(::map)), echoStroke,
                        style = Stroke(
                            width = 1f, cap = StrokeCap.Round,
                            pathEffect = PathEffect.dashPathEffect(floatArrayOf(2f, 12f), 0f)
                        )
                    )
                }
                drawPath(
                    polyline(center.map(::map)), corridorStroke,
                    style = Stroke(
                        width = 2f, cap = StrokeCap.Round,
                        pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f)
                    )
                )
            }
        }
    }
}

private fun polyline(pts: List<Offset>): Path {
    val path = Path()
    if (pts.isEmpty()) return path
    path.moveTo(pts[0].x, pts[0].y)
    for (pt in pts.drop(1)) path.lineTo(pt.x, pt.y)
    return path
}
