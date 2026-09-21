import Foundation
import CoreGraphics
import CoreVideo
import CoreImage
import UIKit
import CurvCameraOverlay
import CurvPalmOverlay

// MARK: - Image Processing & Anatomical Fallback Helpers

extension PalmAnalysisViewModel {

    static let sharedCIContext = CIContext(options: [
        .useSoftwareRenderer: false,
        .priorityRequestLow: false
    ])

    /// Bake an upright UIImage into a BGRA CVPixelBuffer for CurvEngine (matches overlay coords).
    static func makeBGRAPixelBuffer(from image: UIImage?) -> CVPixelBuffer? {
        guard let cgImage = image?.cgImage else { return nil }
        let width = cgImage.width
        let height = cgImage.height
        var buffer: CVPixelBuffer?
        let attrs: [CFString: Any] = [
            kCVPixelBufferCGImageCompatibilityKey: true,
            kCVPixelBufferCGBitmapContextCompatibilityKey: true
        ]
        let status = CVPixelBufferCreate(
            kCFAllocatorDefault,
            width,
            height,
            kCVPixelFormatType_32BGRA,
            attrs as CFDictionary,
            &buffer
        )
        guard status == kCVReturnSuccess, let pixelBuffer = buffer else { return nil }

        CVPixelBufferLockBaseAddress(pixelBuffer, [])
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, []) }
        guard let ctx = CGContext(
            data: CVPixelBufferGetBaseAddress(pixelBuffer),
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: CVPixelBufferGetBytesPerRow(pixelBuffer),
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            return nil
        }
        ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: width, height: height))
        return pixelBuffer
    }

    /// Returns an upright UIImage (pixels already rotated) plus the Vision orientation
    /// that matches the raw capture buffer before baking.
    static func uprightImage(
        from pixelBuffer: CVPixelBuffer
    ) -> (UIImage?, CGImagePropertyOrientation) {
        let ciImage = CIImage(cvPixelBuffer: pixelBuffer)
        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)

        // High-res stills are often sensor-landscape even when the connection is portrait.
        if width > height {
            let rotated = ciImage.oriented(.right)
            guard let cgImage = sharedCIContext.createCGImage(rotated, from: rotated.extent) else {
                return (nil, .right)
            }
            return (UIImage(cgImage: cgImage, scale: 1.0, orientation: .up), .right)
        }

        guard let cgImage = sharedCIContext.createCGImage(ciImage, from: ciImage.extent) else {
            return (nil, .up)
        }
        return (UIImage(cgImage: cgImage, scale: 1.0, orientation: .up), .up)
    }

    /**
     * Landmark-anchored guides — **fallback only** when CurvEngine XCFramework is missing
     * or native JSON has no `points_image_norm`. Must not ship as the product detection path.
     * Phase 1: always returns *something* so overlay never empty for debugging.
     */
    func fallbackLandmarkCreases(landmarks: [CGPoint]) -> [CreaseSpline] {
        // If no landmarks yet (e.g. simulator without camera), return centered scaffold so overlay is still testable.
        if landmarks.count < 21 {
            let cx: CGFloat = 0.5, cy: CGFloat = 0.55
            func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: cx + x*0.28, y: cy + y*0.28) }
            return [
                CreaseSpline(id: "heart_line", lineType: .heart, points: [p(-0.6, -0.22), p(-0.1, -0.24), p(0.45, -0.20), p(0.62, -0.12)], confidence: 0.2, tortuosity: 1.1, arcLengthPx: 120),
                CreaseSpline(id: "head_line",  lineType: .head,  points: [p(-0.55, 0.02), p(-0.05, 0.04), p(0.42, 0.06), p(0.62, 0.08)], confidence: 0.2, tortuosity: 1.1, arcLengthPx: 130),
                CreaseSpline(id: "life_line",  lineType: .life,  points: [p(-0.38, -0.02), p(-0.48, 0.18), p(-0.42, 0.38), p(-0.20, 0.58)], confidence: 0.2, tortuosity: 1.2, arcLengthPx: 180),
                CreaseSpline(id: "fate_line",  lineType: .fate,  points: [p(0.02, 0.52), p(0.04, 0.22), p(0.06, -0.02), p(0.06, -0.18)], confidence: 0.2, tortuosity: 1.0, arcLengthPx: 110)
            ]
        }

        let wrist = landmarks[0]
        let thumbCmc = landmarks[1]
        let thumbMcp = landmarks[2]
        let thumbIp = landmarks[3]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let middleTip = landmarks[12]
        let ringMcp = landmarks[13]
        let pinkyMcp = landmarks[17]

        func lerp(_ a: CGPoint, _ b: CGPoint, _ t: CGFloat) -> CGPoint {
            CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t)
        }

        func palmDepth(_ mcp: CGPoint, _ t: CGFloat) -> CGPoint {
            lerp(mcp, wrist, t)
        }

        func length(_ pts: [CGPoint]) -> Float {
            var sum: Float = 0
            for i in 1..<pts.count {
                let dx = Float(pts[i].x - pts[i - 1].x)
                let dy = Float(pts[i].y - pts[i - 1].y)
                sum += sqrt(dx * dx + dy * dy)
            }
            return sum * 1400
        }

        let heart: [CGPoint] = [
            palmDepth(pinkyMcp, 0.06),
            palmDepth(ringMcp, 0.10),
            palmDepth(middleMcp, 0.12),
            palmDepth(indexMcp, 0.10),
            lerp(palmDepth(indexMcp, 0.08), palmDepth(middleMcp, 0.08), 0.35)
        ]
        let lifeOrigin = lerp(indexMcp, thumbMcp, 0.45)
        let head: [CGPoint] = [
            lerp(lifeOrigin, wrist, 0.18),
            palmDepth(indexMcp, 0.32),
            palmDepth(middleMcp, 0.36),
            palmDepth(ringMcp, 0.34),
            palmDepth(pinkyMcp, 0.30)
        ]
        let life: [CGPoint] = [
            lerp(lifeOrigin, wrist, 0.08),
            lerp(thumbMcp, indexMcp, 0.25),
            lerp(thumbMcp, wrist, 0.35),
            lerp(thumbIp, wrist, 0.55),
            lerp(thumbCmc, wrist, 0.72),
            lerp(wrist, thumbCmc, 0.15)
        ]
        let fate: [CGPoint] = [
            lerp(wrist, middleTip, 0.12),
            lerp(wrist, middleTip, 0.38),
            lerp(wrist, middleTip, 0.58),
            palmDepth(middleMcp, 0.22)
        ]

        return [
            CreaseSpline(id: "heart_line", lineType: .heart, points: heart, confidence: 0.86, tortuosity: 1.12, arcLengthPx: length(heart)),
            CreaseSpline(id: "head_line", lineType: .head, points: head, confidence: 0.90, tortuosity: 1.08, arcLengthPx: length(head)),
            CreaseSpline(id: "life_line", lineType: .life, points: life, confidence: 0.93, tortuosity: 1.25, arcLengthPx: length(life)),
            CreaseSpline(id: "fate_line", lineType: .fate, points: fate, confidence: 0.78, tortuosity: 1.03, arcLengthPx: length(fate))
        ]
    }
}
