import CoreGraphics
import CoreVideo
import Foundation

/**
 * Pinhole camera intrinsics for a captured still.
 * Used by orbit SfM, close-up mosaic warping, and mount relief estimation.
 */
public struct CameraIntrinsics: Codable, Equatable, Sendable {
    public let focalLengthX: Float
    public let focalLengthY: Float
    public let principalPointX: Float
    public let principalPointY: Float
    public let imageWidth: Int
    public let imageHeight: Int
    /// Horizontal field of view in degrees (derived when focal length known).
    public let fieldOfViewDeg: Float?

    public init(
        focalLengthX: Float,
        focalLengthY: Float,
        principalPointX: Float,
        principalPointY: Float,
        imageWidth: Int,
        imageHeight: Int,
        fieldOfViewDeg: Float? = nil
    ) {
        self.focalLengthX = focalLengthX
        self.focalLengthY = focalLengthY
        self.principalPointX = principalPointX
        self.principalPointY = principalPointY
        self.imageWidth = imageWidth
        self.imageHeight = imageHeight
        self.fieldOfViewDeg = fieldOfViewDeg
    }

    /// Approximate intrinsics when only FOV and image size are known.
    public static func fromFieldOfView(imageWidth: Int, imageHeight: Int, fovHorizontalDeg: Float) -> CameraIntrinsics {
        let w = Float(max(1, imageWidth))
        let h = Float(max(1, imageHeight))
        let fovRad = fovHorizontalDeg * .pi / 180.0
        let fx = w / (2.0 * tan(fovRad / 2.0))
        let fy = fx
        return CameraIntrinsics(
            focalLengthX: fx,
            focalLengthY: fy,
            principalPointX: w * 0.5,
            principalPointY: h * 0.5,
            imageWidth: imageWidth,
            imageHeight: imageHeight,
            fieldOfViewDeg: fovHorizontalDeg
        )
    }

    public var jsonDictionary: [String: Any] {
        var dict: [String: Any] = [
            "fx": focalLengthX,
            "fy": focalLengthY,
            "cx": principalPointX,
            "cy": principalPointY,
            "image_width": imageWidth,
            "image_height": imageHeight
        ]
        if let fov = fieldOfViewDeg {
            dict["fov_horizontal_deg"] = fov
        }
        return dict
    }
}

/// High-res still plus calibration metadata.
public struct CapturedPhoto: Sendable {
    public let pixelBuffer: CVPixelBuffer
    public let intrinsics: CameraIntrinsics

    public init(pixelBuffer: CVPixelBuffer, intrinsics: CameraIntrinsics) {
        self.pixelBuffer = pixelBuffer
        self.intrinsics = intrinsics
    }
}
