package org.curv.overlay.palm

import kotlin.math.acos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt

data class DigitMeasurement(
    val name: String,
    val totalLengthMm: Float,
    val proximalPhalanxMm: Float,
    val intermediatePhalanxMm: Float,
    val distalPhalanxMm: Float,
    val lengthRelativeToHand: Float
)

data class DigitRatios(
    val ratio2D4D: Float,
    val ratio2D3D: Float,
    val ratio3D4D: Float,
    val ratio4D5D: Float,
    val thumbToIndex: Float
)

data class PalmDimensions(
    val metacarpalBreadthMm: Float,
    val palmLengthMm: Float,
    val totalHandLengthMm: Float,
    val aspectRatio: Float,
    val palmSurfaceAreaCm2: Float,
    val handShapeType: String
)

data class InterdigitalSpan(
    val thumbIndexAngleDeg: Float,
    val indexMiddleAngleDeg: Float,
    val middleRingAngleDeg: Float,
    val ringPinkyAngleDeg: Float,
    val thumbIndexDistanceMm: Float,
    val indexMiddleDistanceMm: Float,
    val middleRingDistanceMm: Float,
    val ringPinkyDistanceMm: Float
)

data class AnatomicalPadMetric(
    val padName: String,
    val heightMm: Float,
    val relativeProminence: Float,
    val areaMm2: Float,
    val curvature: Float
) {
    val anatomicalPad: AnatomicalPad?
        get() = AnatomicalPad.fromKey(padName) ?: AnatomicalPad.fromAstrologicalMount(padName)
}

data class PalmarCreaseMetric(
    val anatomicalName: String,
    val traditionalAlias: String,
    val arcLengthMm: Float,
    val chordLengthMm: Float,
    val tortuosity: Float,
    val isSimianVariant: Boolean = false
) {
    val scientificDisplayName: String
        get() = when (anatomicalName) {
            "distal_transverse_crease" -> "Distal Transverse Crease"
            "proximal_transverse_crease" -> "Proximal Transverse Crease"
            "thenar_longitudinal_crease" -> "Thenar Longitudinal Crease"
            "median_longitudinal_crease" -> "Median Longitudinal Crease"
            else -> anatomicalName
        }
}

data class HandBiometricsReport(
    val schemaVersion: String = "curv_hand_anthropometry_v1",
    val palm: PalmDimensions,
    val digits: List<DigitMeasurement>,
    val ratios: DigitRatios,
    val spans: InterdigitalSpan,
    val pads: List<AnatomicalPadMetric>,
    val creases: List<PalmarCreaseMetric> = emptyList(),
    val mmPerPixel: Float,
    val handSide: String
)

object HandBiometricsCalculator {
    private fun dist(p1: List<Float>, p2: List<Float>): Float {
        val dx = p1[0] - p2[0]
        val dy = p1[1] - p2[1]
        return sqrt(dx * dx + dy * dy)
    }

    private fun angleBetween(v1: Pair<Float, Float>, v2: Pair<Float, Float>): Float {
        val mag1 = sqrt(v1.first * v1.first + v1.second * v1.second)
        val mag2 = sqrt(v2.first * v2.first + v2.second * v2.second)
        if (mag1 < 1e-6f || mag2 < 1e-6f) return 0.0f
        val dot = v1.first * v2.first + v1.second * v2.second
        val cosTheta = max(-1.0f, min(1.0f, dot / (mag1 * mag2)))
        return (acos(cosTheta) * 180.0f / Math.PI.toFloat())
    }

    fun compute(
        landmarks: List<List<Float>>,
        imageWidth: Float = 1000.0f,
        imageHeight: Float = 1000.0f,
        referencePalmWidthMm: Float = 80.0f,
        padDepthMm: Map<String, Float> = emptyMap(),
        handSide: String = "auto"
    ): HandBiometricsReport? {
        if (landmarks.size < 21) return null

        val p5 = listOf(landmarks[5][0] * imageWidth, landmarks[5][1] * imageHeight)
        val p17 = listOf(landmarks[17][0] * imageWidth, landmarks[17][1] * imageHeight)
        val palmBreadthPx = dist(p5, p17)
        if (palmBreadthPx <= 1.0f) return null

        val mmPerPx = referencePalmWidthMm / palmBreadthPx
        val pxToMm = { px: Float -> px * mmPerPx }

        val p0 = listOf(landmarks[0][0] * imageWidth, landmarks[0][1] * imageHeight)
        val p9 = listOf(landmarks[9][0] * imageWidth, landmarks[9][1] * imageHeight)
        val palmLengthMm = pxToMm(dist(p0, p9))

        val p12 = listOf(landmarks[12][0] * imageWidth, landmarks[12][1] * imageHeight)
        val totalHandLengthMm = pxToMm(dist(p0, p12))

        val metacarpalBreadthMm = referencePalmWidthMm
        val aspectRatio = if (palmLengthMm > 1e-3f) metacarpalBreadthMm / palmLengthMm else 1.0f
        val areaCm2 = (metacarpalBreadthMm * palmLengthMm * 0.85f) / 100.0f
        val shapeType = when {
            aspectRatio > 0.9f -> "broad"
            aspectRatio < 0.75f -> "elongated"
            else -> "proportional"
        }

        val palm = PalmDimensions(
            metacarpalBreadthMm = metacarpalBreadthMm,
            palmLengthMm = palmLengthMm,
            totalHandLengthMm = totalHandLengthMm,
            aspectRatio = aspectRatio,
            palmSurfaceAreaCm2 = areaCm2,
            handShapeType = shapeType
        )

        data class Chain(val name: String, val indices: List<Int>)
        val chains = listOf(
            Chain("thumb", listOf(1, 2, 3, 4)),
            Chain("index", listOf(5, 6, 7, 8)),
            Chain("middle", listOf(9, 10, 11, 12)),
            Chain("ring", listOf(13, 14, 15, 16)),
            Chain("pinky", listOf(17, 18, 19, 20))
        )

        val digits = mutableListOf<DigitMeasurement>()
        val digitLengths = mutableMapOf<String, Float>()

        for (c in chains) {
            val pts = c.indices.map { listOf(landmarks[it][0] * imageWidth, landmarks[it][1] * imageHeight) }
            val p1 = pxToMm(dist(pts[0], pts[1]))
            val p2 = pxToMm(dist(pts[1], pts[2]))
            val p3 = pxToMm(dist(pts[2], pts[3]))
            val total = p1 + p2 + p3
            digitLengths[c.name] = total
            val rel = if (totalHandLengthMm > 1e-3f) total / totalHandLengthMm else 0.0f

            digits.add(
                DigitMeasurement(
                    name = c.name,
                    totalLengthMm = total,
                    proximalPhalanxMm = p1,
                    intermediatePhalanxMm = if (c.name == "thumb") 0.0f else p2,
                    distalPhalanxMm = p3,
                    lengthRelativeToHand = rel
                )
            )
        }

        val indexLen = digitLengths["index"] ?: 70.0f
        val middleLen = digitLengths["middle"] ?: 78.0f
        val ringLen = digitLengths["ring"] ?: 72.0f
        val pinkyLen = digitLengths["pinky"] ?: 55.0f
        val thumbLen = digitLengths["thumb"] ?: 50.0f

        val ratios = DigitRatios(
            ratio2D4D = if (ringLen > 1e-3f) indexLen / ringLen else 0.97f,
            ratio2D3D = if (middleLen > 1e-3f) indexLen / middleLen else 0.90f,
            ratio3D4D = if (ringLen > 1e-3f) middleLen / ringLen else 1.08f,
            ratio4D5D = if (pinkyLen > 1e-3f) ringLen / pinkyLen else 1.30f,
            thumbToIndex = if (indexLen > 1e-3f) thumbLen / indexLen else 0.70f
        )

        val tip4 = listOf(landmarks[4][0] * imageWidth, landmarks[4][1] * imageHeight)
        val tip8 = listOf(landmarks[8][0] * imageWidth, landmarks[8][1] * imageHeight)
        val tip16 = listOf(landmarks[16][0] * imageWidth, landmarks[16][1] * imageHeight)
        val tip20 = listOf(landmarks[20][0] * imageWidth, landmarks[20][1] * imageHeight)

        val vThumb = Pair(tip4[0] - p0[0], tip4[1] - p0[1])
        val vIndex = Pair(tip8[0] - p0[0], tip8[1] - p0[1])
        val vMiddle = Pair(p12[0] - p0[0], p12[1] - p0[1])
        val vRing = Pair(tip16[0] - p0[0], tip16[1] - p0[1])
        val vPinky = Pair(tip20[0] - p0[0], tip20[1] - p0[1])

        val spans = InterdigitalSpan(
            thumbIndexAngleDeg = angleBetween(vThumb, vIndex),
            indexMiddleAngleDeg = angleBetween(vIndex, vMiddle),
            middleRingAngleDeg = angleBetween(vMiddle, vRing),
            ringPinkyAngleDeg = angleBetween(vRing, vPinky),
            thumbIndexDistanceMm = pxToMm(dist(tip4, tip8)),
            indexMiddleDistanceMm = pxToMm(dist(tip8, p12)),
            middleRingDistanceMm = pxToMm(dist(p12, tip16)),
            ringPinkyDistanceMm = pxToMm(dist(tip16, tip20))
        )

        val pads = AnatomicalPad.entries.map { pad ->
            val key = pad.key
            val astroKey = pad.astrologicalMountName
            val hMm = padDepthMm[key] ?: padDepthMm[astroKey] ?: 0.0f
            val prominence = max(-1.0f, min(1.0f, hMm / 4.0f))
            AnatomicalPadMetric(
                padName = key,
                heightMm = hMm,
                relativeProminence = prominence,
                areaMm2 = 150.0f,
                curvature = prominence * 0.15f
            )
        }

        // Palmar flexion creases (standard anatomical nomenclature)
        val p6 = listOf(landmarks[6][0] * imageWidth, landmarks[6][1] * imageHeight)
        val p18 = listOf(landmarks[18][0] * imageWidth, landmarks[18][1] * imageHeight)
        val p2 = listOf(landmarks[2][0] * imageWidth, landmarks[2][1] * imageHeight)

        val dtcChord = pxToMm(dist(p17, p6) * 0.90f)
        val ptcChord = pxToMm(dist(p5, p18) * 0.85f)
        val tlcChord = pxToMm(dist(p2, p0) * 0.95f)
        val mlcChord = pxToMm(dist(p0, p9) * 0.75f)

        val creases = listOf(
            PalmarCreaseMetric("distal_transverse_crease", "heart", dtcChord * 1.05f, dtcChord, 1.05f),
            PalmarCreaseMetric("proximal_transverse_crease", "head", ptcChord * 1.04f, ptcChord, 1.04f),
            PalmarCreaseMetric("thenar_longitudinal_crease", "life", tlcChord * 1.15f, tlcChord, 1.15f),
            PalmarCreaseMetric("median_longitudinal_crease", "fate", mlcChord * 1.02f, mlcChord, 1.02f)
        )

        return HandBiometricsReport(
            palm = palm,
            digits = digits,
            ratios = ratios,
            spans = spans,
            pads = pads,
            creases = creases,
            mmPerPixel = mmPerPx,
            handSide = handSide
        )
    }
}
