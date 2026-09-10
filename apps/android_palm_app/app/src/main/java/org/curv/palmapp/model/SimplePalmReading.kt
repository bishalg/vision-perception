package org.curv.palmapp.model

import org.curv.overlay.palm.HandSide

/**
 * Basic local Samudrika palm reading — deterministic, rule-based, no network / no VLM.
 * Mirrors iOS SimplePalmReading.swift templates: confidence, tortuosity, arcLengthPx, points count.
 */
object SimplePalmReading {

    fun reading(splines: List<CreaseSpline>, handSide: HandSide): List<String> =
        splines.map { readingText(it, handSide) }

    fun reading(splines: List<CreaseSpline>): List<String> =
        reading(splines, HandSide.RIGHT)

    fun readingText(spline: CreaseSpline, handSide: HandSide): String {
        val handNote = when (handSide) {
            HandSide.LEFT -> "Left (inherited inclination)"
            HandSide.RIGHT -> "Right (active expression)"
            HandSide.AUTO -> "Active hand"
        }
        val base = baseReading(spline)
        val sideCue = when (spline.lineType) {
            CreaseLineType.HEART, CreaseLineType.LIFE -> " — $handNote"
            else -> ""
        }
        return base + sideCue
    }

    fun readingText(spline: CreaseSpline): String = baseReading(spline)

    private fun baseReading(s: CreaseSpline): String {
        val conf = s.confidence
        val tort = s.tortuosity
        val len = s.arcLengthPx
        val pts = s.points.size

        val clarity = when {
            conf < 0.35f -> "faint / lightly marked"
            conf < 0.55f -> "lightly marked"
            conf < 0.75f -> "moderately clear"
            conf < 0.88f -> "clear"
            else -> "very clear, strongly marked"
        }
        val lengthNote = when {
            len < 85f -> "short"
            len < 130f -> "medium-short"
            len < 180f -> "medium-long"
            else -> "long"
        }
        val detailNote = when {
            pts < 4 -> "few anchor points"
            pts <= 6 -> "well-traced"
            else -> "finely traced ($pts pts)"
        }
        val curveNote = when {
            tort > 1.30f -> "strongly curved / winding"
            tort > 1.22f -> "curved"
            tort > 1.10f -> "gently curved"
            else -> "straighter"
        }

        return when (s.lineType) {
            CreaseLineType.HEART -> {
                val temper = when {
                    tort > 1.25f -> "expressive, emotionally open"
                    tort > 1.12f -> "balanced warmth and reserve"
                    else -> "measured, steadier in affections"
                }
                val lenHint = when {
                    len < 100f -> "selective attachments"
                    len > 175f -> "deep continuity in bonds"
                    else -> "steady warmth"
                }
                "Heart Line — $curveNote, $temper; $lengthNote ($lenHint), $clarity, $detailNote · tort ${"%.2f".format(tort)}"
            }
            CreaseLineType.HEAD -> {
                val focus = when {
                    tort > 1.28f -> "flexible, imaginative thinking"
                    tort > 1.14f -> "balanced logic and imagination"
                    else -> "straighter — focused, linear reasoning"
                }
                val lenHint = when {
                    len < 95f -> "concise, to-the-point focus"
                    len > 170f -> "long — extended analytical reach"
                    else -> "even, sustained focus"
                }
                "Head Line — $curveNote, $focus; $lengthNote ($lenHint), $clarity, $detailNote · tort ${"%.2f".format(tort)}"
            }
            CreaseLineType.LIFE -> {
                val vigor = when {
                    tort > 1.32f -> "winding — adaptive, draws around obstacles"
                    tort > 1.18f -> "gentle arc — resilient"
                    else -> "straighter arc — steady stamina"
                }
                val lenHint = when {
                    len < 110f -> "shorter arc — conserve energy, pace well"
                    len > 200f -> "long arc — strong vitality, wide circle"
                    else -> "medium arc — even vitality"
                }
                val confHint = if (conf < 0.5f) "lighter — nurture routines" else "well-marked"
                "Life Line — $vigor; $lengthNote ($lenHint), $clarity ($confHint), $detailNote · tort ${"%.2f".format(tort)} · len ${"%.0f".format(len)}px"
            }
            CreaseLineType.FATE -> {
                val presence = when {
                    conf < 0.38f -> "faint / intermittent — path shaped more by choice than circumstance"
                    conf < 0.65f -> "moderate — evolving direction, builds with effort"
                    len < 90f -> "short but clear — focused phase, goals narrow then widen"
                    else -> "clear and continuous — steady direction, purpose builds"
                }
                val curveHint = if (tort > 1.18f) "($curveNote — flexible course corrections)" else "($curveNote)"
                "Fate Line — $presence $curveHint, $lengthNote, $clarity, $detailNote · tort ${"%.2f".format(tort)}"
            }
        }
    }
}
