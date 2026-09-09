import Foundation

/**
 * Per-frame diagnostic metrics consumed by the Liquid Glass HUD telemetry chips.
 */
public struct QualityMetrics: Equatable {
    public var scale: Float
    public var centerOffset: Float
    public var rollAngleDeg: Float
    public var flatnessRatio: Float
    public var meanLuminance: Float
    public var glareRatio: Float
    public var motionDisplacement: Float

    public var isCenteringOk: Bool
    public var isScaleOk: Bool
    public var isAngleOk: Bool
    public var isIlluminationOk: Bool
    public var isSteadinessOk: Bool
    public var isReadyForCapture: Bool

    public init(
        scale: Float = 0,
        centerOffset: Float = 0,
        rollAngleDeg: Float = 0,
        flatnessRatio: Float = 0,
        meanLuminance: Float = 0,
        glareRatio: Float = 0,
        motionDisplacement: Float = 0,
        isCenteringOk: Bool = false,
        isScaleOk: Bool = false,
        isAngleOk: Bool = false,
        isIlluminationOk: Bool = false,
        isSteadinessOk: Bool = false,
        isReadyForCapture: Bool = false
    ) {
        self.scale = scale
        self.centerOffset = centerOffset
        self.rollAngleDeg = rollAngleDeg
        self.flatnessRatio = flatnessRatio
        self.meanLuminance = meanLuminance
        self.glareRatio = glareRatio
        self.motionDisplacement = motionDisplacement
        self.isCenteringOk = isCenteringOk
        self.isScaleOk = isScaleOk
        self.isAngleOk = isAngleOk
        self.isIlluminationOk = isIlluminationOk
        self.isSteadinessOk = isSteadinessOk
        self.isReadyForCapture = isReadyForCapture
    }
}

public struct PhotometricSample: Equatable {
    public var meanLuminance: Float
    public var glareRatio: Float

    public init(meanLuminance: Float, glareRatio: Float) {
        self.meanLuminance = meanLuminance
        self.glareRatio = glareRatio
    }

    public static let unknown = PhotometricSample(meanLuminance: 128, glareRatio: 0)
}
