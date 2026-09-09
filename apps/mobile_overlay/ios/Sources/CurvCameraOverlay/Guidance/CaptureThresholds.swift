import Foundation

/**
 * Five-gate capture thresholds shared by every mobile overlay app.
 * Domain packs override the defaults; the evaluator stays domain-agnostic.
 */
public struct CaptureThresholds: Equatable {
    /// When true, a missing `SubjectPose` keeps the HUD in `.searching`.
    public var requireDetectedSubject: Bool
    public var minScale: Float
    public var maxScale: Float
    public var maxCenterOffset: Float
    public var maxRollAngleDeg: Float
    public var minFlatnessRatio: Float
    public var minLuminance: Float
    public var maxGlareRatio: Float
    public var maxMotionDisplacement: Float
    public var steadyDurationSec: TimeInterval
    /// Skip centering / scale / tilt gates (photometrics + steadiness only).
    public var skipGeometryGates: Bool

    public init(
        requireDetectedSubject: Bool = true,
        minScale: Float = 0.35,
        maxScale: Float = 0.60,
        maxCenterOffset: Float = 0.18,
        maxRollAngleDeg: Float = 22.0,
        minFlatnessRatio: Float = 0.70,
        minLuminance: Float = 70.0,
        maxGlareRatio: Float = 0.04,
        maxMotionDisplacement: Float = 0.025,
        steadyDurationSec: TimeInterval = 0.30,
        skipGeometryGates: Bool = false
    ) {
        self.requireDetectedSubject = requireDetectedSubject
        self.minScale = minScale
        self.maxScale = maxScale
        self.maxCenterOffset = maxCenterOffset
        self.maxRollAngleDeg = maxRollAngleDeg
        self.minFlatnessRatio = minFlatnessRatio
        self.minLuminance = minLuminance
        self.maxGlareRatio = maxGlareRatio
        self.maxMotionDisplacement = maxMotionDisplacement
        self.steadyDurationSec = steadyDurationSec
        self.skipGeometryGates = skipGeometryGates
    }
}
