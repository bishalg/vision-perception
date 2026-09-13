import Foundation
import CoreGraphics
import simd

/// Mathematical raycaster projecting 3D ocular axes onto 2D device screen geometry.
public struct ScreenRaycaster: Sendable {
    public let screenWidthPoints: CGFloat
    public let screenHeightPoints: CGFloat
    public let screenWidthMeters: Float
    public let screenHeightMeters: Float

    /// Default initializer configured for a standard iPad or iPhone display.
    public init(
        screenWidthPoints: CGFloat = 820.0,
        screenHeightPoints: CGFloat = 1180.0,
        screenWidthMeters: Float = 0.178,
        screenHeightMeters: Float = 0.247
    ) {
        self.screenWidthPoints = screenWidthPoints
        self.screenHeightPoints = screenHeightPoints
        self.screenWidthMeters = max(screenWidthMeters, 0.05)
        self.screenHeightMeters = max(screenHeightMeters, 0.05)
    }

    /// Projects 3D gaze vector and eye position onto screen coordinates.
    ///
    /// - Parameters:
    ///   - lookAtPoint: 3D gaze vector in camera/face space [x, y, z] in meters.
    ///   - distanceMeters: Estimated distance from face to camera/screen in meters.
    ///   - eyeOffsetMeters: Optional independent eye offset relative to face center.
    /// - Returns: ScreenRaycastResult containing physical points and normalized [0, 1] coordinates.
    public func projectGazeToScreen(
        lookAtPoint: simd_float3,
        distanceMeters: Float,
        eyeOffsetMeters: simd_float3 = simd_float3(0, 0, 0)
    ) -> ScreenRaycastResult {
        let dist = max(distanceMeters, 0.1) // avoid divide by zero
        let distCm = Double(dist * 100.0)

        // Intersect ray from eye towards lookAtPoint with the screen plane (Z = 0)
        // In typical front-camera coordinates:
        // lookAtPoint.x: positive right
        // lookAtPoint.y: positive up
        let effectiveX = lookAtPoint.x + eyeOffsetMeters.x
        let effectiveY = lookAtPoint.y + eyeOffsetMeters.y

        // Normalize relative to screen physical dimensions
        // Face center looking straight at center of screen corresponds to norm = (0.5, 0.5)
        let normX = Double(0.5 + (effectiveX / screenWidthMeters))
        // Screen Y in UIKit/SwiftUI points downwards, whereas ARKit Y points upwards
        let normY = Double(0.5 - (effectiveY / screenHeightMeters))

        let clampedX = min(max(normX, 0.0), 1.0)
        let clampedY = min(max(normY, 0.0), 1.0)

        let isWithin = (normX >= 0.0 && normX <= 1.0 && normY >= 0.0 && normY <= 1.0)

        let screenPoint = CGPoint(
            x: clampedX * screenWidthPoints,
            y: clampedY * screenHeightPoints
        )
        let normPoint = CGPoint(x: clampedX, y: clampedY)

        return ScreenRaycastResult(
            screenPoint: screenPoint,
            normalizedPoint: normPoint,
            distanceCm: distCm,
            isWithinScreen: isWithin
        )
    }
}
