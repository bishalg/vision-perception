package org.curv.palmapp.view

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size

/**
 * Maps normalized image coordinates (0..1, top-left origin) into a view that
 * displays the image with aspect-fit or aspect-fill. Parity with iOS ImageCoordinateMapper.swift.
 */
object ImageCoordinateMapper {
    enum class ContentMode { FIT, FILL }

    fun imageRect(imageSize: Size, viewSize: Size, mode: ContentMode = ContentMode.FIT): Rect {
        if (imageSize.width <= 1f || imageSize.height <= 1f || viewSize.width <= 1f || viewSize.height <= 1f) {
            return Rect(Offset.Zero, viewSize)
        }
        val imageAspect = imageSize.width / imageSize.height
        val viewAspect = viewSize.width / viewSize.height
        return when (mode) {
            ContentMode.FIT -> if (imageAspect > viewAspect) {
                val w = viewSize.width
                val h = w / imageAspect
                Rect(Offset(0f, (viewSize.height - h) * 0.5f), Size(w, h))
            } else {
                val h = viewSize.height
                val w = h * imageAspect
                Rect(Offset((viewSize.width - w) * 0.5f, 0f), Size(w, h))
            }
            ContentMode.FILL -> if (imageAspect > viewAspect) {
                val h = viewSize.height
                val w = h * imageAspect
                Rect(Offset((viewSize.width - w) * 0.5f, 0f), Size(w, h))
            } else {
                val w = viewSize.width
                val h = w / imageAspect
                Rect(Offset(0f, (viewSize.height - h) * 0.5f), Size(w, h))
            }
        }
    }

    fun map(point: Offset, into: Rect): Offset =
        Offset(into.left + point.x * into.width, into.top + point.y * into.height)

    fun unmap(point: Offset, from: Rect): Offset {
        if (from.width <= 1f || from.height <= 1f) return Offset.Zero
        val nx = ((point.x - from.left) / from.width).coerceIn(0f, 1f)
        val ny = ((point.y - from.top) / from.height).coerceIn(0f, 1f)
        return Offset(nx, ny)
    }
}
