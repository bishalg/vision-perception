import Foundation

/**
 * Real-time HUD states. Copy is supplied by the active `OverlayProfile`
 * so palm, vastu, and future apps can speak in their own vocabulary.
 */
public enum GuidanceState: Equatable {
    case searching
    case centerSubject
    case moveCloser
    case moveBack
    case holdFlat
    case tooDark
    case glareDetected
    case holdSteady
    case ready(countdownProgress: Float)

    public var isReady: Bool {
        if case .ready = self { return true }
        return false
    }

    public var countdownProgress: Float {
        if case .ready(let progress) = self { return progress }
        return 0
    }
}

public struct GuidanceEvaluation: Equatable {
    public var state: GuidanceState
    public var metrics: QualityMetrics

    public init(state: GuidanceState, metrics: QualityMetrics) {
        self.state = state
        self.metrics = metrics
    }
}
