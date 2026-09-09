import CoreGraphics
import Foundation

public enum TelemetryChannel: String, Equatable {
    case lux
    case glare
    case tilt
}

public struct GuidanceCopy: Equatable {
    public var searching: String
    public var center: String
    public var closer: String
    public var further: String
    public var flat: String
    public var dark: String
    public var glare: String
    public var steady: String
    public var ready: String
    public var processing: String

    public init(
        searching: String = "Place the subject inside the outline",
        center: String = "Center the subject in the guide",
        closer: String = "Move closer to the camera",
        further: String = "Move slightly further back",
        flat: String = "Hold the subject flat facing the camera",
        dark: String = "Too dark — increase lighting",
        glare: String = "Glare detected — angle away from light",
        steady: String = "Hold steady...",
        ready: String = "Capturing...",
        processing: String = "Analyzing..."
    ) {
        self.searching = searching
        self.center = center
        self.closer = closer
        self.further = further
        self.flat = flat
        self.dark = dark
        self.glare = glare
        self.steady = steady
        self.ready = ready
        self.processing = processing
    }

    public func message(for state: GuidanceState) -> String {
        switch state {
        case .searching: return searching
        case .centerSubject: return center
        case .moveCloser: return closer
        case .moveBack: return further
        case .holdFlat: return flat
        case .tooDark: return dark
        case .glareDetected: return glare
        case .holdSteady: return steady
        case .ready(let progress):
            if progress >= 1 { return ready }
            let remaining = max(1, Int(ceil(Double(1.0 - progress) * 3.0)))
            return "\(remaining)"
        }
    }

    /// Visible 3 / 2 / 1 digit during the hold-to-capture window; nil otherwise.
    public func countdownDigit(for state: GuidanceState) -> Int? {
        guard case .ready(let progress) = state, progress < 1 else { return nil }
        return max(1, Int(ceil(Double(1.0 - progress) * 3.0)))
    }
}

/**
 * Per-app overlay configuration. The camera HUD chrome is shared; each product
 * only swaps this profile plus the view drawn inside the reticle.
 */
public struct OverlayProfile: Equatable {
    public var id: String
    public var brand: String
    public var productTitle: String
    public var domainId: String
    public var reticleWidthFraction: CGFloat
    public var reticleHeightFraction: CGFloat
    public var reticleCornerRadius: CGFloat
    public var thresholds: CaptureThresholds
    public var telemetry: [TelemetryChannel]
    public var copy: GuidanceCopy

    public init(
        id: String,
        brand: String = "AstroFusion",
        productTitle: String,
        domainId: String,
        reticleWidthFraction: CGFloat = 0.76,
        reticleHeightFraction: CGFloat = 0.58,
        reticleCornerRadius: CGFloat = 36,
        thresholds: CaptureThresholds = CaptureThresholds(),
        telemetry: [TelemetryChannel] = [.lux, .glare, .tilt],
        copy: GuidanceCopy = GuidanceCopy()
    ) {
        self.id = id
        self.brand = brand
        self.productTitle = productTitle
        self.domainId = domainId
        self.reticleWidthFraction = reticleWidthFraction
        self.reticleHeightFraction = reticleHeightFraction
        self.reticleCornerRadius = reticleCornerRadius
        self.thresholds = thresholds
        self.telemetry = telemetry
        self.copy = copy
    }
}
