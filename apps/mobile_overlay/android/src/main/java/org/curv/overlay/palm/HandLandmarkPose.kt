package org.curv.overlay.palm

import android.graphics.PointF
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.sqrt
import org.curv.overlay.guidance.SubjectPose

object HandLandmarkPose {
    fun from(landmarks: List<PointF>): SubjectPose? {
        if (landmarks.size < 21) return null

        val wrist = landmarks[0]
        val indexMcp = landmarks[5]
        val middleMcp = landmarks[9]
        val middleTip = landmarks[12]
        val pinkyMcp = landmarks[17]
        val tips = listOf(landmarks[4], landmarks[8], middleTip, landmarks[16], landmarks[20], wrist)
        val margin = 0.04f
        for (tip in tips) {
            if (tip.x < margin || tip.x > 1f - margin || tip.y < margin || tip.y > 1f - margin) {
                return null
            }
        }

        val centroid = PointF((wrist.x + middleMcp.x) * 0.5f, (wrist.y + middleMcp.y) * 0.5f)
        val handDx = middleTip.x - wrist.x
        val handDy = middleTip.y - wrist.y
        val scale = sqrt(handDx * handDx + handDy * handDy)

        val rollDx = middleMcp.x - wrist.x
        val rollDy = middleMcp.y - wrist.y
        val rollDeg = abs(atan2(rollDx.toDouble(), -rollDy.toDouble()) * 180.0 / Math.PI).toFloat()

        val widthDx = pinkyMcp.x - indexMcp.x
        val widthDy = pinkyMcp.y - indexMcp.y
        val palmWidth = sqrt(widthDx * widthDx + widthDy * widthDy)
        val palmLengthDx = middleMcp.x - wrist.x
        val palmLengthDy = middleMcp.y - wrist.y
        val palmLength = sqrt(palmLengthDx * palmLengthDx + palmLengthDy * palmLengthDy)
        val aspect = if (palmLength > 0.001f) palmWidth / (palmLength * 0.75f) else 1f

        return SubjectPose(
            centroid = centroid,
            scale = scale,
            rollDeg = rollDeg,
            aspectRatio = aspect,
            samplePoints = landmarks.take(21),
            confidence = 1f
        )
    }
}
