package org.curv.palmapp.model

import android.graphics.PointF
import org.json.JSONObject
import java.util.UUID

/**
 * Parsed data models representing CurvEngine palm extraction output on Android.
 * Parity with iOS PalmAnalysisResult.swift — parses points_image_norm polylines.
 */
data class MajorLineInfo(
    val name: String,
    val present: Boolean,
    val confidence: Float,
    val segmentIds: List<Int>,
    val arcLengthPx: Float,
    val chordLengthPx: Float,
    val tortuosity: Float,
    val meanAngleDeg: Float,
    val startZone: String,
    val endZone: String,
    val gapCount: Int,
    val forkCount: Int,
    val meanWidthPx: Float,
    val meanIntensity: Float,
    val pointsImageNorm: List<PointF>? = null
)

data class PalmDomainPayload(
    val majorLines: Map<String, MajorLineInfo>,
    val dominantLine: String?,
    val heartHeadMinDistancePx: Float?
)

data class PalmAnalysisResponse(
    val isUsable: Boolean,
    val totalDurationUs: Long,
    val domainPayload: PalmDomainPayload?
) {
    companion object {
        fun fromJson(jsonStr: String): PalmAnalysisResponse? {
            return try {
                val root = JSONObject(jsonStr)
                val isUsable = root.optBoolean("is_usable", true)
                val durationUs = root.optLong("total_duration_us", 0L)

                val payloadObj = root.optJSONObject("domain_payload")
                var domainPayload: PalmDomainPayload? = null

                if (payloadObj != null) {
                    val linesMap = mutableMapOf<String, MajorLineInfo>()
                    val linesObj = payloadObj.optJSONObject("major_lines")
                    if (linesObj != null) {
                        for (key in linesObj.keys()) {
                            val lObj = linesObj.getJSONObject(key)
                            val segArray = lObj.optJSONArray("segment_ids")
                            val segIds = mutableListOf<Int>()
                            if (segArray != null) {
                                for (i in 0 until segArray.length()) {
                                    segIds.add(segArray.getInt(i))
                                }
                            }
                            // Parse points_image_norm — image-normalized polylines (0..1).
                            var pts: List<PointF>? = null
                            val ptsArr = lObj.optJSONArray("points_image_norm")
                            if (ptsArr != null && ptsArr.length() > 0) {
                                val list = mutableListOf<PointF>()
                                for (i in 0 until ptsArr.length()) {
                                    val pair = ptsArr.optJSONArray(i) ?: continue
                                    if (pair.length() >= 2) {
                                        list.add(PointF(pair.optDouble(0).toFloat(), pair.optDouble(1).toFloat()))
                                    }
                                }
                                if (list.isNotEmpty()) pts = list
                            }
                            linesMap[key] = MajorLineInfo(
                                name = lObj.optString("name", key),
                                present = lObj.optBoolean("present", false),
                                confidence = lObj.optDouble("confidence", 0.0).toFloat(),
                                segmentIds = segIds,
                                arcLengthPx = lObj.optDouble("arc_length_px", 0.0).toFloat(),
                                chordLengthPx = lObj.optDouble("chord_length_px", 0.0).toFloat(),
                                tortuosity = lObj.optDouble("tortuosity", 1.0).toFloat(),
                                meanAngleDeg = lObj.optDouble("mean_angle_deg", 0.0).toFloat(),
                                startZone = lObj.optString("start_zone", "unknown"),
                                endZone = lObj.optString("end_zone", "unknown"),
                                gapCount = lObj.optInt("gap_count", 0),
                                forkCount = lObj.optInt("fork_count", 0),
                                meanWidthPx = lObj.optDouble("mean_width_px", 0.0).toFloat(),
                                meanIntensity = lObj.optDouble("mean_intensity", 0.0).toFloat(),
                                pointsImageNorm = pts
                            )
                        }
                    }

                    val domLine = payloadObj.optString("dominant_line", null)
                    val hhDist = if (payloadObj.has("heart_head_min_distance_px")) {
                        payloadObj.optDouble("heart_head_min_distance_px").toFloat()
                    } else null

                    domainPayload = PalmDomainPayload(linesMap, domLine, hhDist)
                }

                PalmAnalysisResponse(isUsable, durationUs, domainPayload)
            } catch (e: Exception) {
                null
            }
        }
    }

    /** Build overlay splines from Steger polylines (points_image_norm). Returns null if none present — parity with iOS creaseSplinesFromPolylines(). */
    fun creaseSplinesFromPolylines(): List<CreaseSpline>? {
        val lines = domainPayload?.majorLines ?: return null
        val keyMap = listOf(
            "heart_line" to CreaseLineType.HEART,
            "head_line" to CreaseLineType.HEAD,
            "life_line" to CreaseLineType.LIFE,
            "fate_line" to CreaseLineType.FATE
        )
        val splines = mutableListOf<CreaseSpline>()
        for ((key, type) in keyMap) {
            val info = lines[key] ?: continue
            if (!info.present) continue
            val pts = info.pointsImageNorm ?: continue
            if (pts.size < 2) continue
            val controlPoints = CreaseSpline.resampleToControlPoints(pts, 4)
            splines.add(
                CreaseSpline(
                    id = key,
                    lineType = type,
                    points = controlPoints,
                    confidence = info.confidence,
                    tortuosity = info.tortuosity,
                    arcLengthPx = info.arcLengthPx
                )
            )
        }
        return if (splines.isEmpty()) null else splines
    }
}

enum class CreaseLineType(val lineName: String, val hexColor: String, val displayNameKey: String) {
    HEART("heart", "#FF2D55", "line.heart"),
    HEAD("head", "#007AFF", "line.head"),
    LIFE("life", "#34C759", "line.life"),
    FATE("fate", "#AF52DE", "line.fate")
}

enum class CreaseOverlaySource {
    NATIVE_ENGINE,
    NEEDS_MANUAL_ALIGNMENT
}

data class CreaseSpline(
    val id: String = UUID.randomUUID().toString(),
    val lineType: CreaseLineType,
    val points: List<PointF>,
    val confidence: Float,
    val tortuosity: Float,
    val arcLengthPx: Float,
    val isUserAdjusted: Boolean = false
) {
    /** Guarantee clean control points via [resampleToControlPoints] when needed. */
    fun cleaned(targetCount: Int = 4): CreaseSpline =
        if (points.size > 5) copy(points = resampleToControlPoints(points, targetCount))
        else this

    fun replacing(
        lineType: CreaseLineType? = null,
        points: List<PointF>? = null,
        markAdjusted: Boolean = true
    ): CreaseSpline {
        val newPts = points ?: this.points
        val cleanPts = if (newPts.size > 5) resampleToControlPoints(newPts, 4) else newPts
        return copy(
            lineType = lineType ?: this.lineType,
            points = cleanPts,
            isUserAdjusted = if (markAdjusted) true else isUserAdjusted
        )
    }

    /** Smooth dense points along the Catmull-Rom spline for prediction / VLM grounding. */
    fun evaluateDensePoints(samplesPerSegment: Int = 16): List<PointF> {
        if (points.size < 2) return points
        if (points.size == 2) return points
        val result = mutableListOf<PointF>()
        for (i in 0 until points.size - 1) {
            val p0 = if (i > 0) points[i - 1] else points[i]
            val p1 = points[i]
            val p2 = points[i + 1]
            val p3 = if (i + 2 < points.size) points[i + 2] else p2
            for (step in 0 until samplesPerSegment) {
                val t = step.toFloat() / samplesPerSegment.toFloat()
                val t2 = t * t
                val t3 = t2 * t
                val x = 0.5f * ((2f * p1.x) +
                    (-p0.x + p2.x) * t +
                    (2f * p0.x - 5f * p1.x + 4f * p2.x - p3.x) * t2 +
                    (-p0.x + 3f * p1.x - 3f * p2.x + p3.x) * t3)
                val y = 0.5f * ((2f * p1.y) +
                    (-p0.y + p2.y) * t +
                    (2f * p0.y - 5f * p1.y + 4f * p2.y - p3.y) * t2 +
                    (-p0.y + 3f * p1.y - 3f * p2.y + p3.y) * t3)
                result.add(PointF(x, y))
            }
        }
        points.lastOrNull()?.let { result.add(it) }
        return result
    }

    /** Payload fragment for prediction — continuous curve points (parity with iOS). */
    val predictionPointsImageNorm: List<List<Double>>
        get() = evaluateDensePoints(samplesPerSegment = 8)
            .map { listOf(it.x.toDouble(), it.y.toDouble()) }

    val engineKey: String get() = "${lineType.lineName}_line"

    companion object {
        /**
         * Reduce any raw polyline (even 100+ dense detector points) into a clean,
         * editable 4-point control curve: Start, Curve Mid 1, Curve Mid 2, End.
         * Port of iOS CreaseSpline.resampleToControlPoints.
         */
        fun resampleToControlPoints(rawPoints: List<PointF>, targetCount: Int = 4): List<PointF> {
            if (rawPoints.size <= targetCount || targetCount < 2) return rawPoints
            val cumulative = mutableListOf(0f)
            var totalLength = 0f
            for (i in 1 until rawPoints.size) {
                val dx = rawPoints[i].x - rawPoints[i - 1].x
                val dy = rawPoints[i].y - rawPoints[i - 1].y
                val dist = kotlin.math.sqrt((dx * dx + dy * dy).toDouble()).toFloat()
                totalLength += dist
                cumulative.add(totalLength)
            }
            if (totalLength <= 1e-5f) {
                return listOf(rawPoints.first(), rawPoints.last())
            }
            val resampled = mutableListOf(rawPoints.first())
            for (k in 1 until targetCount - 1) {
                val targetDist = totalLength * (k.toFloat() / (targetCount - 1).toFloat())
                var segIndex = 0
                while (segIndex < cumulative.size - 2 && cumulative[segIndex + 1] < targetDist) {
                    segIndex++
                }
                val d0 = cumulative[segIndex]
                val d1 = cumulative[segIndex + 1]
                val segLen = d1 - d0
                val t = if (segLen > 1e-6f) (targetDist - d0) / segLen else 0f
                val p0 = rawPoints[segIndex]
                val p1 = rawPoints[segIndex + 1]
                resampled.add(PointF(p0.x + (p1.x - p0.x) * t, p0.y + (p1.y - p0.y) * t))
            }
            resampled.add(rawPoints.last())
            return resampled
        }
    }
}
