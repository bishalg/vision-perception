import CurvCameraOverlay
import Foundation

public struct VastuZoneInfo: Identifiable, Equatable {
    public var id: String { key }
    public let key: String
    public let name: String
    public let element: String

    public init(key: String, name: String, element: String) {
        self.key = key
        self.name = name
        self.element = element
    }

    /// 3x3 layout, north-up: NW N NE / W C E / SW S SE
    public static let mandalaGrid: [[VastuZoneInfo]] = [
        [
            VastuZoneInfo(key: "vayu", name: "Vayu", element: "Air"),
            VastuZoneInfo(key: "uttara", name: "Uttara", element: "Wealth"),
            VastuZoneInfo(key: "ishanya", name: "Ishanya", element: "Water")
        ],
        [
            VastuZoneInfo(key: "pashchima", name: "Pashchima", element: "West"),
            VastuZoneInfo(key: "brahmasthan", name: "Brahmasthan", element: "Space"),
            VastuZoneInfo(key: "purva", name: "Purva", element: "Light")
        ],
        [
            VastuZoneInfo(key: "nairutya", name: "Nairutya", element: "Earth"),
            VastuZoneInfo(key: "dakshina", name: "Dakshina", element: "Fire"),
            VastuZoneInfo(key: "agni", name: "Agni", element: "Fire")
        ]
    ]
}

public enum VastuOverlay {
    public static let profile = OverlayProfile(
        id: "vastu",
        brand: "AstroFusion",
        productTitle: "Vastu Perception",
        domainId: "floor_plan",
        reticleWidthFraction: 0.82,
        reticleHeightFraction: 0.62,
        reticleCornerRadius: 20,
        thresholds: CaptureThresholds(
            requireDetectedSubject: false,
            minScale: 0.40,
            maxScale: 0.95,
            maxCenterOffset: 0.22,
            maxRollAngleDeg: 12,
            minFlatnessRatio: 0.65,
            minLuminance: 70,
            maxGlareRatio: 0.04,
            maxMotionDisplacement: 0.030,
            steadyDurationSec: 0.35,
            skipGeometryGates: true
        ),
        telemetry: [.lux, .glare],
        copy: GuidanceCopy(
            searching: "Align the floor plan with the mandala",
            center: "Center the plan in the grid",
            closer: "Move closer so walls fill the grid",
            further: "Move back to fit the full plan",
            flat: "Hold the plan flat, north at the top",
            dark: "Too dark — add more light",
            glare: "Reduce glare on the plan",
            steady: "Hold steady...",
            ready: "Capturing Vastu layout...",
            processing: "Mapping walls to the Purusha Mandala..."
        )
    )

    /// Normalized quad (TL, TR, BR, BL) covering the HUD reticle — C-ABI `process_quad_roi`.
    public static func reticleQuad() -> [Float] {
        let w = Float(profile.reticleWidthFraction)
        let h = Float(profile.reticleHeightFraction)
        let x0 = 0.5 - w * 0.5
        let y0 = 0.5 - h * 0.5
        let x1 = 0.5 + w * 0.5
        let y1 = 0.5 + h * 0.5
        return [x0, y0, x1, y0, x1, y1, x0, y1]
    }
}
