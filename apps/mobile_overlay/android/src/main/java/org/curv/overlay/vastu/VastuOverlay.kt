package org.curv.overlay.vastu

import org.curv.overlay.guidance.CaptureThresholds
import org.curv.overlay.profile.GuidanceCopy
import org.curv.overlay.profile.OverlayProfile
import org.curv.overlay.profile.TelemetryChannel

data class VastuZoneInfo(
    val key: String,
    val name: String,
    val element: String
) {
    companion object {
        val mandalaGrid: List<List<VastuZoneInfo>> = listOf(
            listOf(
                VastuZoneInfo("vayu", "Vayu", "Air"),
                VastuZoneInfo("uttara", "Uttara", "Wealth"),
                VastuZoneInfo("ishanya", "Ishanya", "Water")
            ),
            listOf(
                VastuZoneInfo("pashchima", "Pashchima", "West"),
                VastuZoneInfo("brahmasthan", "Brahmasthan", "Space"),
                VastuZoneInfo("purva", "Purva", "Light")
            ),
            listOf(
                VastuZoneInfo("nairutya", "Nairutya", "Earth"),
                VastuZoneInfo("dakshina", "Dakshina", "Fire"),
                VastuZoneInfo("agni", "Agni", "Fire")
            )
        )
    }
}

object VastuOverlay {
    val profile = OverlayProfile(
        id = "vastu",
        brand = "AstroFusion",
        productTitle = "Vastu Perception",
        domainId = "floor_plan",
        reticleWidthFraction = 0.82f,
        reticleHeightFraction = 0.62f,
        reticleCornerRadiusDp = 20f,
        thresholds = CaptureThresholds(
            requireDetectedSubject = false,
            minScale = 0.40f,
            maxScale = 0.95f,
            maxCenterOffset = 0.22f,
            maxRollAngleDeg = 12f,
            minFlatnessRatio = 0.65f,
            minLuminance = 70f,
            maxGlareRatio = 0.04f,
            maxMotionDisplacement = 0.030f,
            steadyDurationSec = 0.35f,
            skipGeometryGates = true
        ),
        telemetry = listOf(TelemetryChannel.LUX, TelemetryChannel.GLARE),
        copy = GuidanceCopy(
            searching = "Align the floor plan with the mandala",
            center = "Center the plan in the grid",
            closer = "Move closer so walls fill the grid",
            further = "Move back to fit the full plan",
            flat = "Hold the plan flat, north at the top",
            dark = "Too dark — add more light",
            glare = "Reduce glare on the plan",
            steady = "Hold steady...",
            ready = "Capturing Vastu layout...",
            processing = "Mapping walls to the Purusha Mandala..."
        )
    )

    fun reticleQuad(): FloatArray {
        val w = profile.reticleWidthFraction
        val h = profile.reticleHeightFraction
        val x0 = 0.5f - w * 0.5f
        val y0 = 0.5f - h * 0.5f
        val x1 = 0.5f + w * 0.5f
        val y1 = 0.5f + h * 0.5f
        return floatArrayOf(x0, y0, x1, y0, x1, y1, x0, y1)
    }
}
