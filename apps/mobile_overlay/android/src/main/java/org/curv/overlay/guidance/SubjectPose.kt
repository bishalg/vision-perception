package org.curv.overlay.guidance

import android.graphics.PointF

/**
 * Domain-agnostic subject geometry in normalized frame coordinates (origin top-left).
 */
data class SubjectPose(
    val centroid: PointF,
    val scale: Float,
    val rollDeg: Float,
    val aspectRatio: Float,
    val samplePoints: List<PointF> = emptyList(),
    val confidence: Float = 1.0f
) {
    companion object {
        fun reticleFill(widthFraction: Float, heightFraction: Float): SubjectPose {
            val cx = 0.5f
            val cy = 0.5f
            val corners = listOf(
                PointF(cx - widthFraction * 0.5f, cy - heightFraction * 0.5f),
                PointF(cx + widthFraction * 0.5f, cy - heightFraction * 0.5f),
                PointF(cx + widthFraction * 0.5f, cy + heightFraction * 0.5f),
                PointF(cx - widthFraction * 0.5f, cy + heightFraction * 0.5f)
            )
            return SubjectPose(
                centroid = PointF(cx, cy),
                scale = heightFraction,
                rollDeg = 0f,
                aspectRatio = if (heightFraction > 0.001f) widthFraction / heightFraction else 1f,
                samplePoints = corners,
                confidence = 1f
            )
        }
    }
}
