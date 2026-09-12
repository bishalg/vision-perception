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

    /** Mirrors core/geometry/RoiWarper::detectIsLeftHand — robust thumb vs pinky lateral position. */
    fun isLeftHand(landmarks: List<PointF>): Boolean {
        if (landmarks.size < 18) return false
        val p0 = landmarks[0] // wrist
        val p9 = landmarks[9] // middle MCP
        val upX = p9.x - p0.x
        val upY = p9.y - p0.y
        val upLen = sqrt(upX * upX + upY * upY)
        if (upLen < 1e-4f) return false
        val rightX = -upY / upLen
        val rightY = upX / upLen

        // 1. Primary: Thumb lateral vs Pinky lateral relative to palm centerline (wrist -> middle MCP)
        if (landmarks.size >= 21) {
            val p2 = landmarks[2]  // thumb MP
            val p4 = landmarks[4]  // thumb tip
            val p17 = landmarks[17] // pinky MCP
            val p20 = landmarks[20] // pinky tip

            val thumbX = (p2.x + p4.x) * 0.5f - p0.x
            val thumbY = (p2.y + p4.y) * 0.5f - p0.y
            val thumbLateral = thumbX * rightX + thumbY * rightY

            val pinkyX = (p17.x + p20.x) * 0.5f - p0.x
            val pinkyY = (p17.y + p20.y) * 0.5f - p0.y
            val pinkyLateral = pinkyX * rightX + pinkyY * rightY

            // For palm facing camera: Left palm has thumb on the right (thumbLateral > pinkyLateral)
            return thumbLateral > pinkyLateral
        }

        // 2. Fallback: Index MCP (p5) to Pinky MCP (p17) dot product
        val p5 = landmarks[5]
        val p17 = landmarks[17]
        val palmWX = p17.x - p5.x
        val palmWY = p17.y - p5.y
        val dot = palmWX * rightX + palmWY * rightY
        return dot < 0f
    }
}
