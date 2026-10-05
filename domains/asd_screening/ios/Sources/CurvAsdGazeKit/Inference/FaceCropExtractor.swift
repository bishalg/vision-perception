import Foundation
import CoreVideo
import CoreGraphics
import CoreImage

// MARK: - Crop Rotation

/// Rotation applied to the raw ARKit captured image so the age model receives an
/// upright face. TrueDepth captured frames are stored in the sensor's landscape
/// orientation; a portrait front-camera session maps to `.clockwise90`.
public enum FaceCropRotation: String, Codable, Sendable, CaseIterable {
    case none
    case clockwise90
    case counterclockwise90
    case halfTurn

    /// CoreImage EXIF orientation realizing this rotation.
    public var cgOrientation: CGImagePropertyOrientation {
        switch self {
        case .none: return .up
        case .clockwise90: return .right
        case .counterclockwise90: return .left
        case .halfTurn: return .down
        }
    }
}

// MARK: - Face Crop Extractor

/// Extracts an upright, square face crop from an ARKit captured image for the age
/// model. All rectangle math is pure and unit-tested; rendering goes through a
/// shared `CIContext`.
///
/// Crop sizing: when a projected face radius is known, the crop side is
/// `faceDiameterMultiple` × face diameter — wide enough to keep head-and-shoulder
/// context (head-to-body proportion is a strong adult-vs-child cue). When the
/// projection is unavailable, it falls back to a centered `fallbackCropSpan` crop,
/// which safely contains a TrueDepth-tracked face at the 30–75 cm gate distance.
public struct FaceCropExtractor: Sendable {
    /// Output crop edge in pixels (model input size).
    public let outputSize: Int
    /// Center-crop fraction of the smaller image dimension when no face projection is known.
    public let fallbackCropSpan: Double
    /// Minimum crop side as a fraction of the smaller image dimension.
    public let minSpanFraction: Double
    /// Crop side as a multiple of the projected face diameter.
    public let faceDiameterMultiple: Double

    public init(
        outputSize: Int = 224,
        fallbackCropSpan: Double = 0.62,
        minSpanFraction: Double = 0.35,
        faceDiameterMultiple: Double = 2.6
    ) {
        self.outputSize = outputSize
        self.fallbackCropSpan = fallbackCropSpan
        self.minSpanFraction = minSpanFraction
        self.faceDiameterMultiple = faceDiameterMultiple
    }

    // MARK: Face Geometry

    /// Normalized face geometry in captured-image pixel space (origin top-left).
    public struct FaceRect: Sendable, Equatable {
        /// Face-center x in [0, 1] of the image width.
        public let centerXNorm: Double
        /// Face-center y in [0, 1] of the image height.
        public let centerYNorm: Double
        /// Half face width as a fraction of the smaller image dimension.
        public let radiusNorm: Double

        public init(centerXNorm: Double, centerYNorm: Double, radiusNorm: Double) {
            self.centerXNorm = max(0.0, min(1.0, centerXNorm))
            self.centerYNorm = max(0.0, min(1.0, centerYNorm))
            self.radiusNorm = max(0.005, min(0.5, radiusNorm))
        }

        /// TrueDepth face tracking keeps the subject near frame center; used when
        /// the projection glue cannot produce trustworthy coordinates.
        public static let centered = FaceRect(centerXNorm: 0.5, centerYNorm: 0.5, radiusNorm: 0.14)
    }

    // MARK: Crop Math (pure, unit-tested)

    /// Square crop rectangle in source pixel coordinates, clamped to the buffer bounds.
    public func cropRect(for face: FaceRect?, pixelWidth: Int, pixelHeight: Int) -> CGRect {
        guard pixelWidth > 0, pixelHeight > 0 else { return .zero }
        let minDim = Double(min(pixelWidth, pixelHeight))

        let side: Double
        if let face {
            let faceDiameterPx = 2.0 * face.radiusNorm * minDim
            side = min(max(faceDiameterPx * faceDiameterMultiple, minSpanFraction * minDim), minDim)
        } else {
            side = fallbackCropSpan * minDim
        }

        let centerX = (face?.centerXNorm ?? 0.5) * Double(pixelWidth)
        let centerY = (face?.centerYNorm ?? 0.5) * Double(pixelHeight)

        let originX = min(max(centerX - side / 2.0, 0.0), Double(pixelWidth) - side)
        let originY = min(max(centerY - side / 2.0, 0.0), Double(pixelHeight) - side)
        return CGRect(x: originX, y: originY, width: side, height: side)
    }

    // MARK: Extraction (CoreImage)

    private static let sharedContext = CIContext(options: [.cacheIntermediates: false])

    /// Renders an upright `outputSize × outputSize` BGRA crop from the source
    /// buffer. The buffer is consumed synchronously on the caller's ARKit delegate
    /// queue and never retained.
    public func extractCrop(
        from buffer: CVPixelBuffer,
        face: FaceRect?,
        rotation: FaceCropRotation
    ) -> CVPixelBuffer? {
        let pixelWidth = CVPixelBufferGetWidth(buffer)
        let pixelHeight = CVPixelBufferGetHeight(buffer)
        let rect = cropRect(for: face, pixelWidth: pixelWidth, pixelHeight: pixelHeight)
        guard rect.width >= 8, rect.height >= 8 else { return nil }

        var image = CIImage(cvPixelBuffer: buffer).cropped(to: rect)
        image = image.oriented(rotation.cgOrientation)
        guard image.extent.width > 0, image.extent.height > 0 else { return nil }

        // Scale from the exact crop side — CoreImage rounds extents to whole pixels,
        // so the transformed extent lands within ±1–2 px of the target and the
        // render into the fixed-size output buffer clips the remainder.
        let scale = Double(outputSize) / Double(max(rect.width, rect.height))
        image = image.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        image = image.transformed(
            by: CGAffineTransform(translationX: -image.extent.origin.x, y: -image.extent.origin.y)
        )
        guard abs(image.extent.width - CGFloat(outputSize)) <= 2.0,
              abs(image.extent.height - CGFloat(outputSize)) <= 2.0 else { return nil }

        var output: CVPixelBuffer?
        // IOSurface backing is required for CIContext rendering on macOS and is the
        // default on iOS; Metal compatibility lets the crop feed ANE/GPU pipelines.
        let attributes: [CFString: Any] = [
            kCVPixelBufferCGImageCompatibilityKey: true,
            kCVPixelBufferMetalCompatibilityKey: true,
            kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary
        ]
        let status = CVPixelBufferCreate(
            kCFAllocatorDefault,
            outputSize,
            outputSize,
            kCVPixelFormatType_32BGRA,
            attributes as CFDictionary,
            &output
        )
        guard status == kCVReturnSuccess, let output else { return nil }

        Self.sharedContext.render(image, to: output)
        return output
    }
}
