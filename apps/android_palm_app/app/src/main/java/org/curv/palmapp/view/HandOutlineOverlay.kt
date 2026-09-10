package org.curv.palmapp.view

import android.graphics.PointF
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke

/**
 * Hand landmark skeleton overlay — parity with iOS HandOutlineOverlay.
 */
@Composable
fun HandOutlineOverlay(
    landmarks: List<PointF>,
    imageRect: Rect? = null,
    modifier: Modifier = Modifier
) {
    if (landmarks.size < 21) return
    Canvas(modifier = modifier.fillMaxSize()) {
        val rect = imageRect ?: Rect(Offset.Zero, size)
        fun map(pt: PointF): Offset = Offset(rect.left + pt.x * rect.width, rect.top + pt.y * rect.height)

        val hullIdx = listOf(0, 1, 2, 4, 8, 12, 16, 20, 17, 0)
        val hullPath = Path().apply {
            hullIdx.forEachIndexed { i, idx ->
                val p = map(landmarks[idx])
                if (i == 0) moveTo(p.x, p.y) else lineTo(p.x, p.y)
            }
        }
        drawPath(hullPath, Color.White.copy(alpha = 0.35f), style = Stroke(width = 3f, cap = StrokeCap.Round, join = StrokeJoin.Round))

        val chains = listOf(
            listOf(0, 1, 2, 3, 4),
            listOf(0, 5, 6, 7, 8),
            listOf(0, 9, 10, 11, 12),
            listOf(0, 13, 14, 15, 16),
            listOf(0, 17, 18, 19, 20)
        )
        for (chain in chains) {
            val p = Path().apply {
                chain.forEachIndexed { i, idx ->
                    val pt = map(landmarks[idx])
                    if (i == 0) moveTo(pt.x, pt.y) else lineTo(pt.x, pt.y)
                }
            }
            drawPath(p, Color.White.copy(alpha = 0.18f), style = Stroke(width = 2f, cap = StrokeCap.Round, join = StrokeJoin.Round))
        }
        for (pt in landmarks) {
            val c = map(pt)
            drawCircle(Color.White.copy(alpha = 0.55f), radius = 5f, center = c)
        }
    }
}
