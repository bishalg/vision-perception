import Foundation
import CoreGraphics
import simd

#if canImport(UIKit) && !os(watchOS)
import UIKit
#endif

/// Mathematical raycaster projecting 3D ocular axes onto 2D device screen geometry.
public struct ScreenRaycaster: Sendable {
    public let screenWidthPoints: CGFloat
    public let screenHeightPoints: CGFloat
    public let screenWidthMeters: Float
    public let screenHeightMeters: Float

    /// Default initializer automatically adapting to iPhone vs iPad display dimensions.
    public init(
        screenWidthPoints: CGFloat? = nil,
        screenHeightPoints: CGFloat? = nil,
        screenWidthMeters: Float? = nil,
        screenHeightMeters: Float? = nil
    ) {
        #if canImport(UIKit) && !os(watchOS)
        let isPad = UIDevice.current.userInterfaceIdiom == .pad
        let defaultPointsW: CGFloat = isPad ? 820.0 : 393.0
        let defaultPointsH: CGFloat = isPad ? 1180.0 : 852.0
        let defaultMetersW: Float = isPad ? 0.178 : 0.071
        let defaultMetersH: Float = isPad ? 0.247 : 0.154
        #else
        let defaultPointsW: CGFloat = 820.0
        let defaultPointsH: CGFloat = 1180.0
        let defaultMetersW: Float = 0.178
        let defaultMetersH: Float = 0.247
        #endif

        self.screenWidthPoints = screenWidthPoints ?? defaultPointsW
        self.screenHeightPoints = screenHeightPoints ?? defaultPointsH
        self.screenWidthMeters = max(screenWidthMeters ?? defaultMetersW, 0.05)
        self.screenHeightMeters = max(screenHeightMeters ?? defaultMetersH, 0.05)
    }

    /// Projects 3D gaze from ARFaceAnchor (face transform, local lookAtPoint, and eye transforms)
    /// onto the device screen plane (Z = 0) in camera coordinate space.
    ///
    /// - Parameters:
    ///   - faceTransform: 4x4 matrix representing the face pose in camera coordinate space.
    ///   - lookAtPoint: 3D gaze point relative to face anchor origin in meters.
    ///   - leftEyeTransform: Optional left eye 4x4 transform relative to face anchor.
    ///   - rightEyeTransform: Optional right eye 4x4 transform relative to face anchor.
    /// - Returns: ScreenRaycastResult with screen points, normalized [0, 1] coordinates, and distance.
    public func projectGazeToScreen(
        faceTransform: simd_float4x4,
        lookAtPoint: simd_float3,
        leftEyeTransform: simd_float4x4? = nil,
        rightEyeTransform: simd_float4x4? = nil
    ) -> ScreenRaycastResult {
        // 1. Transform lookAtPoint from face local coordinates into camera coordinate space:
        let pFaceLook = simd_float4(lookAtPoint.x, lookAtPoint.y, lookAtPoint.z, 1.0)
        let pCamLook4 = faceTransform * pFaceLook
        let targetCam = simd_float3(pCamLook4.x, pCamLook4.y, pCamLook4.z)

        // 2. Compute eye origin in camera coordinate space:
        // Use midpoint of left and right eyes if available, otherwise face center
        let eyeOriginCam: simd_float3
        if let leftEye = leftEyeTransform, let rightEye = rightEyeTransform,
           (leftEye.columns.3.x != 0 || leftEye.columns.3.y != 0 || leftEye.columns.3.z != 0) {
            let leftCam4 = faceTransform * leftEye.columns.3
            let rightCam4 = faceTransform * rightEye.columns.3
            eyeOriginCam = simd_float3(
                (leftCam4.x + rightCam4.x) * 0.5,
                (leftCam4.y + rightCam4.y) * 0.5,
                (leftCam4.z + rightCam4.z) * 0.5
            )
        } else {
            eyeOriginCam = simd_float3(
                faceTransform.columns.3.x,
                faceTransform.columns.3.y + 0.03,
                faceTransform.columns.3.z
            )
        }

        let distanceMeters = max(0.1, -eyeOriginCam.z)
        let distCm = Double(distanceMeters * 100.0)

        // 3. Raycast from eye origin through look-at target to screen plane Z = 0
        // Ray equation: R(t) = eyeOriginCam + t * (targetCam - eyeOriginCam)
        let rayDir = targetCam - eyeOriginCam
        let hitScreenCam: simd_float3
        let validForwardRay: Bool
        if abs(rayDir.z) > 0.001 {
            let t = -eyeOriginCam.z / rayDir.z
            validForwardRay = t > 0
            hitScreenCam = eyeOriginCam + t * rayDir
        } else {
            validForwardRay = true
            hitScreenCam = targetCam
        }

        // 4. Map camera physical coordinates (meters) to screen coordinates [0, 1]
        // Camera is centered horizontally (x = 0). Left edge is -width/2, Right edge is +width/2.
        let normX = Double(0.5 + (hitScreenCam.x / screenWidthMeters))

        // Camera is near top of phone.
        // +Y_cam points UP; SwiftUI Y points DOWN.
        // Top edge of screen is at cameraYOffsetFromScreenTopMeters (~0.008 m).
        let cameraYTop: Float = 0.008
        let normY = Double((cameraYTop - hitScreenCam.y) / screenHeightMeters)

        let clampedX = min(max(normX, 0.0), 1.0)
        let clampedY = min(max(normY, 0.0), 1.0)
        let isWithin = validForwardRay && (normX >= 0.0 && normX <= 1.0 && normY >= 0.0 && normY <= 1.0)

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
