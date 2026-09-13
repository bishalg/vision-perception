package org.curv.overlay.palm

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import org.json.JSONObject

/**
 * Shared shape-spec runtime for the native overlays (Android side).
 *
 * Anatomy-free loading of segment lists from
 * `apps/mobile_overlay/shared/shapes/*.json` (single source of truth):
 * normalized 0..1 paths become Compose [Path]s. Mirrors iOS
 * CurvCameraOverlay/GuideSpec.swift behavior exactly (same chaining,
 * same mirror rule, same hex parsing).
 *
 * Rules (see shared/shapes/README.md):
 * - Normalized 0..1 space, y down. Map with x * w / y * h.
 * - Mirror x for the opposite hand; never mirror labels, only geometry.
 * - Fill the closed silhouette loop once; stroke finger segments individually.
 *
 * NOTE: not yet wired into PalmSilhouette.kt (tracked next step); the JSON
 * and this loader land first so the art source is single before views move.
 */
data class GuideSegment(val move: List<Float>, val curves: List<List<Float>>) {
    val isWellFormed: Boolean
        get() = move.size == 2 && curves.all { it.size == 6 }

    val startPoint: Offset?
        get() = if (move.size == 2) Offset(move[0], move[1]) else null

    val endPoint: Offset?
        get() {
            val last = curves.lastOrNull()
            return if (last != null && last.size == 6) Offset(last[4], last[5]) else startPoint
        }
}

object GuidePaths {

    /** Decodes the `segments` subtree of silhouette.json into dotted keys. */
    fun loadSegments(doc: JSONObject): Map<String, GuideSegment> {
        val out = mutableMapOf<String, GuideSegment>()
        val segments = doc.optJSONObject("segments")
            ?: throw IllegalArgumentException("shape spec has no segments object")
        flatten(segments, "", out)
        require(out.isNotEmpty()) { "shape spec segments are empty" }
        for ((key, seg) in out) {
            require(seg.isWellFormed) { "malformed segment: $key" }
        }
        return out
    }

    private fun flatten(node: JSONObject, prefix: String, out: MutableMap<String, GuideSegment>) {
        for (key in node.keys()) {
            val dotted = if (prefix.isEmpty()) key else "$prefix.$key"
            val child = node.optJSONObject(key) ?: continue
            val move = child.optJSONArray("move")
            val curves = child.optJSONArray("curves")
            if (move != null && curves != null) {
                out[dotted] = GuideSegment(
                    move = List(move.length()) { i -> move.getDouble(i).toFloat() },
                    curves = List(curves.length()) { i ->
                        val c = curves.getJSONArray(i)
                        List(c.length()) { j -> c.getDouble(j).toFloat() }
                    }
                )
            } else {
                flatten(child, dotted, out)
            }
        }
    }

    /** Builds a strokable/fillable path for one segment in a w*h pixel rect. */
    fun buildPath(seg: GuideSegment, w: Float, h: Float, mirror: Boolean = false): Path {
        fun px(x: Float) = (if (mirror) 1f - x else x) * w
        fun py(y: Float) = y * h
        return Path().apply {
            if (seg.move.size != 2) return@apply
            moveTo(px(seg.move[0]), py(seg.move[1]))
            for (c in seg.curves) {
                if (c.size != 6) continue
                cubicTo(px(c[0]), py(c[1]), px(c[2]), py(c[3]), px(c[4]), py(c[5]))
            }
        }
    }

    /** Parses "#RRGGBB" (as stored in overlay_theme.json). */
    fun hexToColor(hex: String, alpha: Float = 1f): Color {
        val cleaned = hex.trimStart().removePrefix("#")
        require(cleaned.length == 6) { "theme color must be #RRGGBB: $hex" }
        val value = cleaned.toLong(16)
        return Color(
            red = ((value shr 16) and 0xFF).toInt() / 255f,
            green = ((value shr 8) and 0xFF).toInt() / 255f,
            blue = (value and 0xFF).toInt() / 255f,
            alpha = alpha
        )
    }
}
