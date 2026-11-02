package org.curv.asd.spike

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.SystemClock
import android.view.View
import kotlin.math.cos
import kotlin.math.sin

/**
 * The bench target (as the iOS bench dot): a white disc with a small red centre at a
 * screen-normalised position over the full display (origin top-left, y down).
 */
class TargetView(context: Context) : View(context) {
    private val density = resources.displayMetrics.density
    private val outer = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.WHITE }
    private val inner = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.RED }
    private var target: Pair<Double, Double>? = null

    fun show(xNorm: Double, yNorm: Double) {
        target = xNorm to yNorm
        invalidate()
    }

    fun clear() {
        target = null
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        val (x, y) = target ?: return
        val cx = (x * width).toFloat()
        val cy = (y * height).toFloat()
        canvas.drawCircle(cx, cy, 14f * density, outer)
        canvas.drawCircle(cx, cy, 3f * density, inner)
    }
}

/**
 * The free-viewing "clip": a few coloured discs drifting on slow Lissajous paths for 60 s.
 * Synthetic and content-free on purpose (no stimulus licensing, no AOIs); it exists so the
 * log has clip frames for valid-frame ratio and frame rate under continuous viewing.
 */
class FreeViewView(context: Context) : View(context) {
    private val density = resources.displayMetrics.density
    private val paints = listOf(0xFFFFC107, 0xFF03A9F4, 0xFF8BC34A, 0xFFE91E63, 0xFFFFFFFF)
        .map { c -> Paint(Paint.ANTI_ALIAS_FLAG).apply { color = c.toInt() } }
    private var startMs = 0L
    var running = false
        private set

    fun start() {
        startMs = SystemClock.uptimeMillis()
        running = true
        postInvalidateOnAnimation()
    }

    fun stop() {
        running = false
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        if (!running) return
        val t = (SystemClock.uptimeMillis() - startMs) / 1000.0
        paints.forEachIndexed { i, p ->
            val a = 0.11 + 0.037 * i
            val b = 0.07 + 0.029 * i
            val x = 0.5 + 0.38 * sin(a * 2 * Math.PI * t + i)
            val y = 0.5 + 0.40 * cos(b * 2 * Math.PI * t + 2 * i)
            canvas.drawCircle((x * width).toFloat(), (y * height).toFloat(), (22 + 6 * i) * density, p)
        }
        postInvalidateOnAnimation()
    }
}
