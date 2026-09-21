import CurvCameraOverlay
import Foundation

public enum PalmOverlay {
    public static let profile = OverlayProfile(
        id: "palm",
        brand: "AstroFusion",
        productTitle: "Palm Perception",
        domainId: "palm",
        reticleWidthFraction: 0.84,
        reticleHeightFraction: 0.64,
        reticleCornerRadius: 28,
        thresholds: CaptureThresholds(
            requireDetectedSubject: true,
            // Wider distance band so different hand sizes / arms lengths work.
            minScale: 0.32,
            maxScale: 0.78,
            maxCenterOffset: 0.22,
            maxRollAngleDeg: 28,
            minFlatnessRatio: 0.45,
            minLuminance: 55,
            maxGlareRatio: 0.06,
            maxMotionDisplacement: 0.040,
            steadyDurationSec: 3.0,
            skipGeometryGates: false
        ),
        telemetry: [.lux, .glare, .tilt],
        copy: GuidanceCopy(
            searching: "Show your full hand — palm + all fingers",
            center: "Center your whole hand in the outline",
            closer: "Move closer",
            further: "Move farther",
            flat: "Hold hand flat, fingers toward camera",
            dark: "Too dark — add more light",
            glare: "Angle hand away from glare",
            steady: "Hold still for capture…",
            ready: "Capturing…",
            processing: "Mapping palm lines..."
        )
    )

    /// Maps normalized hand-length error to an approximate cm coaching hint.
    /// Tuned so ~0.022 scale ≈ 1 cm at typical phone holding distance.
    public static func distanceHintCm(scale: Float, thresholds: CaptureThresholds) -> (cm: Int, closer: Bool)? {
        let perCm: Float = 0.022
        if scale < thresholds.minScale {
            let cm = max(1, Int(round((thresholds.minScale - scale) / perCm)))
            return (min(cm, 25), true)
        }
        if scale > thresholds.maxScale {
            let cm = max(1, Int(round((scale - thresholds.maxScale) / perCm)))
            return (min(cm, 25), false)
        }
        return nil
    }
}
