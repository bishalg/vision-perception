import CoreGraphics

/**
 * Domain-agnostic subject geometry in normalized frame coordinates (origin top-left).
 *
 * Palm apps fill this from 21 hand landmarks; vastu / floor-plan apps fill it from
 * a document quad or treat the reticle as the subject (`reticleFill`).
 */
public struct SubjectPose: Equatable {
    public var centroid: CGPoint
    public var scale: Float
    public var rollDeg: Float
    public var aspectRatio: Float
    public var samplePoints: [CGPoint]
    public var confidence: Float

    public init(
        centroid: CGPoint,
        scale: Float,
        rollDeg: Float,
        aspectRatio: Float,
        samplePoints: [CGPoint] = [],
        confidence: Float = 1.0
    ) {
        self.centroid = centroid
        self.scale = scale
        self.rollDeg = rollDeg
        self.aspectRatio = aspectRatio
        self.samplePoints = samplePoints
        self.confidence = confidence
    }

    /// Synthetic pose covering the HUD reticle. Used by document / vastu capture
    /// where the user aligns content to the overlay instead of a landmark tracker.
    public static func reticleFill(
        widthFraction: CGFloat,
        heightFraction: CGFloat
    ) -> SubjectPose {
        let w = Float(widthFraction)
        let h = Float(heightFraction)
        let cx: CGFloat = 0.5
        let cy: CGFloat = 0.5
        let corners = [
            CGPoint(x: cx - widthFraction * 0.5, y: cy - heightFraction * 0.5),
            CGPoint(x: cx + widthFraction * 0.5, y: cy - heightFraction * 0.5),
            CGPoint(x: cx + widthFraction * 0.5, y: cy + heightFraction * 0.5),
            CGPoint(x: cx - widthFraction * 0.5, y: cy + heightFraction * 0.5)
        ]
        return SubjectPose(
            centroid: CGPoint(x: cx, y: cy),
            scale: h,
            rollDeg: 0,
            aspectRatio: h > 0.001 ? w / h : 1,
            samplePoints: corners,
            confidence: 1
        )
    }
}
