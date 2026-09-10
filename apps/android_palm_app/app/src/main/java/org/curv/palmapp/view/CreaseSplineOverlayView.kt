package org.curv.palmapp.view

import android.graphics.PointF
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import org.curv.palmapp.model.CreaseSpline
import kotlin.math.hypot

/**
 * Editable crease overlay — parity with iOS CreaseSplineOverlayView.swift.
 * Renders Catmull-Rom splines mapped into [imageRect] (aspect-fit), supports
 * tap-to-select and drag handles when editing.
 */
@Composable
fun CreaseSplineOverlayView(
    splines: List<CreaseSpline>,
    imageRect: Rect? = null,
    selectedSplineId: String? = null,
    isEditing: Boolean = false,
    useScaffoldStyle: Boolean = false,
    onSelect: ((String) -> Unit)? = null,
    onMoveVertex: ((String, Int, Offset) -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val viewSize = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
        // Fallback rect is full view when imageRect not yet known.
        val rect = imageRect ?: Rect(Offset.Zero, viewSize)

        // Tap to select line (separate gesture)
        Canvas(
            modifier = Modifier
                .fillMaxSize()
                .pointerInput(splines, selectedSplineId, isEditing, rect) {
                    if (!isEditing) return@pointerInput
                    detectTapGestures { tap ->
                        nearestSplineId(tap, splines, rect)?.let { onSelect?.invoke(it) }
                    }
                }
                .pointerInput(splines, selectedSplineId, isEditing, rect) {
                    if (!isEditing || selectedSplineId == null) return@pointerInput
                    val spline = splines.firstOrNull { it.id == selectedSplineId } ?: return@pointerInput
                    var dragIdx = -1
                    detectDragGestures(
                        onDragStart = { start ->
                            var bestIdx = -1
                            var bestDist = Float.MAX_VALUE
                            for ((idx, pt) in spline.points.withIndex()) {
                                val canvasPt = pointToCanvas(PointF(pt.x, pt.y), rect)
                                val d = hypot(canvasPt.x - start.x, canvasPt.y - start.y)
                                if (d < bestDist) { bestDist = d; bestIdx = idx }
                            }
                            if (bestDist < 68f) dragIdx = bestIdx
                        },
                        onDrag = { change, _ ->
                            if (dragIdx >= 0) {
                                val norm = canvasToNorm(change.position, rect)
                                onMoveVertex?.invoke(selectedSplineId, dragIdx, norm)
                            }
                            change.consume()
                        },
                        onDragEnd = { dragIdx = -1 },
                        onDragCancel = { dragIdx = -1 }
                    )
                }
        ) {
            for (spline in splines) {
                if (spline.points.size < 2) continue
                val canvasPoints = spline.points.map { pt -> pointToCanvas(pt, rect) }
                val path = buildCatmullRomPathCanvas(canvasPoints)
                val color = parseHexColor(spline.lineType.hexColor)
                val selected = spline.id == selectedSplineId
                val dim = isEditing && selectedSplineId != null && !selected
                val scaffold = useScaffoldStyle && !spline.isUserAdjusted

                if (scaffold) {
                    drawPath(
                        path = path,
                        color = color.copy(alpha = if (dim) 0.2f else 0.55f),
                        style = Stroke(
                            width = if (selected) 3.0f.dp.toPx() else 2.2f.dp.toPx(),
                            cap = StrokeCap.Round,
                            join = StrokeJoin.Round,
                            pathEffect = PathEffect.dashPathEffect(floatArrayOf(14f, 12f), 0f)
                        )
                    )
                } else {
                    val outerAlpha = if (dim) 0.15f else 0.4f
                    drawPath(
                        path = path,
                        color = color.copy(alpha = outerAlpha),
                        style = Stroke(width = if (selected) 16f else 14f, cap = StrokeCap.Round, join = StrokeJoin.Round)
                    )
                    drawPath(
                        path = path,
                        color = color.copy(alpha = if (dim) 0.35f else 1f),
                        style = Stroke(width = if (selected) 6f else 5f, cap = StrokeCap.Round, join = StrokeJoin.Round)
                    )
                }
            }

            // Vertex handles for selected spline.
            if (isEditing && selectedSplineId != null) {
                val spline = splines.firstOrNull { it.id == selectedSplineId }
                if (spline != null) {
                    val dotColor = parseHexColor(spline.lineType.hexColor)
                    for ((idx, pt) in spline.points.withIndex()) {
                        val canvasPt = pointToCanvas(pt, rect)
                        val isEnd = idx == 0 || idx == spline.points.size - 1
                        val r = if (isEnd) 11.dp.toPx() else 9.dp.toPx()
                        drawCircle(color = Color.White, radius = r + 2f, center = canvasPt)
                        drawCircle(color = dotColor, radius = r, center = canvasPt)
                    }
                }
            }
        }
    }
}

/** Legacy simple canvas — wraps overlay with full-size rect for backwards compat. */
@Composable
fun CreaseSplineCanvas(splines: List<CreaseSpline>, modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val rect = Rect(Offset.Zero, Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat()))
        CreaseSplineOverlayView(splines = splines, imageRect = rect, isEditing = false)
    }
}

private fun pointToCanvas(pt: PointF, rect: Rect): Offset =
    Offset(rect.left + pt.x * rect.width, rect.top + pt.y * rect.height)

private fun canvasToNorm(pos: Offset, rect: Rect): Offset {
    if (rect.width <= 1f || rect.height <= 1f) return Offset.Zero
    return Offset(
        ((pos.x - rect.left) / rect.width).coerceIn(0f, 1f),
        ((pos.y - rect.top) / rect.height).coerceIn(0f, 1f)
    )
}

private fun buildCatmullRomPathCanvas(points: List<Offset>): Path {
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

private fun nearestSplineId(tap: Offset, splines: List<CreaseSpline>, rect: Rect): String? {
    var best: Pair<String, Float>? = null
    for (spline in splines) {
        for (pt in spline.points) {
            val mapped = pointToCanvas(pt, rect)
            val d = hypot(mapped.x - tap.x, mapped.y - tap.y)
            if (d < 72f && (best == null || d < best.second)) best = spline.id to d
        }
    }
    return best?.first
}

private fun parseHexColor(hex: String): Color {
    val clean = hex.removePrefix("#")
    val v = clean.toLongOrNull(16) ?: 0xFFFFFF
    return if (clean.length == 6) Color(0xFF000000 or v) else Color(v)
}
