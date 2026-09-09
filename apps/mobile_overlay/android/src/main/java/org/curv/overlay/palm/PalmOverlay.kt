package org.curv.overlay.palm

import org.curv.overlay.guidance.CaptureThresholds
import org.curv.overlay.profile.GuidanceCopy
import org.curv.overlay.profile.OverlayProfile
import org.curv.overlay.profile.TelemetryChannel

object PalmOverlay {
    val profile = OverlayProfile(
        id = "palm",
        brand = "AstroFusion",
        productTitle = "Palm Perception",
        domainId = "palm",
        reticleWidthFraction = 0.84f,
        reticleHeightFraction = 0.72f,
        reticleCornerRadiusDp = 28f,
        thresholds = CaptureThresholds(
            requireDetectedSubject = true,
            minScale = 0.42f,
            maxScale = 0.68f,
            maxCenterOffset = 0.16f,
            maxRollAngleDeg = 18f,
            minFlatnessRatio = 0.55f,
            minLuminance = 70f,
            maxGlareRatio = 0.04f,
            maxMotionDisplacement = 0.028f,
            steadyDurationSec = 3.0f,
            skipGeometryGates = false
        ),
        telemetry = listOf(TelemetryChannel.LUX, TelemetryChannel.GLARE, TelemetryChannel.TILT),
        copy = GuidanceCopy(
            searching = "Show your full hand — palm + all fingers",
            center = "Center your whole hand in the outline",
            closer = "Move a bit closer — keep all fingers visible",
            further = "Move back — include wrist and fingertips",
            flat = "Hold hand flat, fingers spread toward camera",
            dark = "Too dark — add more light",
            glare = "Angle hand away from glare",
            steady = "Hold still for capture…",
            ready = "Capturing…",
            processing = "Mapping palm lines..."
        )
    )
}
