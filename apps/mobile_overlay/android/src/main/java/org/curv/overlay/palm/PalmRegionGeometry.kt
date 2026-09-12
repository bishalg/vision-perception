package org.curv.overlay.palm

import android.graphics.PointF
import kotlin.math.max
import kotlin.math.sqrt

/**
 * Landmark-anchored palm region geometry for the live overlay.
 * Pure math, no UI: 21 MediaPipe-order points in normalized space
 * (y-down, origin top-left) → 7 mount polygons + 3 line-corridor guides.
 *
 * Direct port of iOS CurvPalmOverlay/PalmRegionGeometry.swift.
 * Anchors mirror HandAnatomyDecomposer (digit rays + basal MCP crease).
 */
enum class PalmMount { JUPITER, SATURN, APOLLO, MERCURY, VENUS, LUNA, MARS }

enum class PalmCorridor { HEART, HEAD, LIFE }

data class PalmRegions(
    val mounts: List<Pair<PalmMount, List<PointF>>>,
    val corridors: List<Triple<PalmCorridor, List<PointF>, List<List<PointF>>>>
) {
    fun mountPolygon(mount: PalmMount): List<PointF> =
        mounts.firstOrNull { it.first == mount }?.second ?: emptyList()
}

object PalmRegionGeometry {

    private fun lerp(a: PointF, b: PointF, t: Float): PointF =
        PointF(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)

    private fun mid(a: PointF, b: PointF): PointF = lerp(a, b, 0.5f)

    /** Point dropped from an MCP toward the wrist: unit palm-depth. */
    private fun depth(mcp: PointF, wrist: PointF, t: Float): PointF = lerp(mcp, wrist, t)

    /** Builds regions from 21 normalized landmarks. Null when input is incomplete. */
    fun regions(from: List<PointF>): PalmRegions? {
        if (from.size < 21) return null
        val wrist = from[0]
        val thumbCmc = from[1]
        val thumbMcp = from[2]
        val indexMcp = from[5]
        val middleMcp = from[9]
        val ringMcp = from[13]
        val pinkyMcp = from[17]

        val idxMid = mid(indexMcp, middleMcp)
        val midMid = mid(middleMcp, ringMcp)
        val ringMid = mid(ringMcp, pinkyMcp)

        // ulnar-outward unit (pinky side, away from palm center)
        val ux = pinkyMcp.x - indexMcp.x
        val uy = pinkyMcp.y - indexMcp.y
        val ulen = max(1e-4f, sqrt(ux * ux + uy * uy))
        val uw = ulen * 0.38f
        val ox = ux / ulen * uw
        val oy = uy / ulen * uw
        fun push(p: PointF): PointF = PointF(p.x + ox, p.y + oy)

        // Four finger mounts: MCP row on top, 0.22 palm-depth foot.
        fun fingerMount(topA: PointF, topB: PointF, outward: Boolean = false): List<PointF> {
            val a = topA
            val b = if (outward) push(topB) else topB
            return listOf(a, b, depth(b, wrist, 0.22f), depth(a, wrist, 0.22f))
        }

        val venus: List<PointF> = listOf(
            thumbCmc,
            thumbMcp,
            lerp(indexMcp, thumbMcp, 0.45f),
            lerp(wrist, indexMcp, 0.35f),
            lerp(thumbMcp, wrist, 0.35f)
        )
        val lunaTop = lerp(wrist, pinkyMcp, 0.55f)
        val luna: List<PointF> = listOf(lunaTop, push(lunaTop), push(wrist), wrist)
        val mars: List<PointF> = listOf(
            depth(middleMcp, wrist, 0.40f), depth(pinkyMcp, wrist, 0.40f),
            push(depth(pinkyMcp, wrist, 0.58f)), push(depth(middleMcp, wrist, 0.58f))
        )

        val mounts: List<Pair<PalmMount, List<PointF>>> = listOf(
            PalmMount.JUPITER to fingerMount(topA = indexMcp, topB = idxMid),
            PalmMount.SATURN to fingerMount(topA = idxMid, topB = midMid),
            PalmMount.APOLLO to fingerMount(topA = midMid, topB = ringMid),
            PalmMount.MERCURY to fingerMount(topA = ringMid, topB = pinkyMcp, outward = true),
            PalmMount.VENUS to venus,
            PalmMount.LUNA to luna,
            PalmMount.MARS to mars
        )

        val lifeOrigin = lerp(indexMcp, thumbMcp, 0.45f)
        val heart: List<PointF> = listOf(
            depth(pinkyMcp, wrist, 0.06f), depth(ringMcp, wrist, 0.10f),
            depth(middleMcp, wrist, 0.12f), depth(indexMcp, wrist, 0.10f)
        )
        val head: List<PointF> = listOf(
            lerp(lifeOrigin, wrist, 0.18f),
            depth(indexMcp, wrist, 0.32f), depth(middleMcp, wrist, 0.36f),
            depth(ringMcp, wrist, 0.34f), depth(pinkyMcp, wrist, 0.30f)
        )
        val life: List<PointF> = listOf(
            lerp(lifeOrigin, wrist, 0.08f),
            lerp(thumbMcp, indexMcp, 0.25f),
            lerp(thumbMcp, wrist, 0.35f),
            lerp(from[3], wrist, 0.55f),
            lerp(thumbCmc, wrist, 0.72f)
        )

        fun echoes(pts: List<PointF>): List<List<PointF>> {
            val inEcho = pts.map { lerp(it, wrist, 0.05f) }
            val outEcho = pts.map { p ->
                val dx = p.x - wrist.x
                val dy = p.y - wrist.y
                val len = max(1e-4f, sqrt(dx * dx + dy * dy))
                val step = len * 0.05f
                PointF(p.x + dx / len * step, p.y + dy / len * step)
            }
            return listOf(inEcho, outEcho)
        }

        return PalmRegions(
            mounts = mounts,
            corridors = listOf(
                Triple(PalmCorridor.HEART, heart, echoes(heart)),
                Triple(PalmCorridor.HEAD, head, echoes(head)),
                Triple(PalmCorridor.LIFE, life, echoes(life))
            )
        )
    }

    /** Synthetic centered right hand (normalized) for previews / tests. */
    fun scaffoldLandmarks(): List<PointF> {
        val cx = 0.5f
        fun p(x: Float, y: Float) = PointF(cx + x, y)
        return listOf(
            p(0.00f, 0.86f),                       // 0 wrist
            p(-0.20f, 0.72f), p(-0.24f, 0.60f), p(-0.30f, 0.50f), p(-0.34f, 0.42f), // 1-4 thumb
            p(-0.13f, 0.44f), p(-0.14f, 0.30f), p(-0.14f, 0.20f), p(-0.14f, 0.12f), // 5-8 index
            p(-0.01f, 0.42f), p(-0.01f, 0.26f), p(-0.01f, 0.15f), p(-0.01f, 0.06f), // 9-12 middle
            p(0.11f, 0.44f), p(0.12f, 0.29f), p(0.12f, 0.19f), p(0.13f, 0.11f),     // 13-16 ring
            p(0.21f, 0.48f), p(0.24f, 0.37f), p(0.26f, 0.29f), p(0.27f, 0.23f)      // 17-20 pinky
        )
    }
}
