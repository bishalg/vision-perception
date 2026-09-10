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
            splines.add(
                CreaseSpline(
                    id = key,
                    lineType = type,
                    points = pts,
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
    HEART("heart", "#F43F5E", "line.heart"),
    HEAD("head", "#06B6D4", "line.head"),
    LIFE("life", "#10B981", "line.life"),
    FATE("fate", "#8B5CF6", "line.fate")
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
    fun replacing(
        lineType: CreaseLineType? = null,
        points: List<PointF>? = null,
        markAdjusted: Boolean = true
    ): CreaseSpline = copy(
        lineType = lineType ?: this.lineType,
        points = points ?: this.points,
        isUserAdjusted = if (markAdjusted) true else isUserAdjusted
    )

    val predictionPointsImageNorm: List<List<Double>>
        get() = points.map { listOf(it.x.toDouble(), it.y.toDouble()) }

    val engineKey: String get() = "${lineType.lineName}_line"
}
