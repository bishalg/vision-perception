import CoreGraphics
import CoreVideo

/**
 * Fast sub-sampled luminance + specular glare probe on a BGRA `CVPixelBuffer`.
 * Shared by every overlay app; the sample window follows the subject centroid.
 */
public enum Photometrics {
    public static func sample(
        pixelBuffer: CVPixelBuffer,
        center: CGPoint,
        radius: Float
    ) -> PhotometricSample {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return .unknown
        }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)

        let cx = Int(center.x * CGFloat(width))
        let cy = Int(center.y * CGFloat(height))
        let rPx = Int(CGFloat(radius) * CGFloat(min(width, height)))

        let x0 = max(0, cx - rPx)
        let x1 = min(width - 1, cx + rPx)
        let y0 = max(0, cy - rPx)
        let y1 = min(height - 1, cy + rPx)

        if x1 <= x0 || y1 <= y0 { return .unknown }

        var totalLum: Float = 0
        var glareCount = 0
        var sampleCount = 0
        let step = 4

        for y in stride(from: y0, through: y1, by: step) {
            let rowOffset = y * bytesPerRow
            for x in stride(from: x0, through: x1, by: step) {
                let pixelOffset = rowOffset + x * 4
                let b = Float(ptr[pixelOffset])
                let g = Float(ptr[pixelOffset + 1])
                let r = Float(ptr[pixelOffset + 2])
                let yVal = 0.299 * r + 0.587 * g + 0.114 * b
                totalLum += yVal
                if yVal > 245 { glareCount += 1 }
                sampleCount += 1
            }
        }

        if sampleCount == 0 { return .unknown }
        return PhotometricSample(
            meanLuminance: totalLum / Float(sampleCount),
            glareRatio: Float(glareCount) / Float(sampleCount)
        )
    }
}
