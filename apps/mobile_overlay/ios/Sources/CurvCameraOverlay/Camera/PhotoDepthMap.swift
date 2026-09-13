import AVFoundation
import CoreVideo
import Foundation

/**
 * Downsampled depth snapshot in meters, safe to pass across queues.
 *
 * Conversion policy (kept in one place so capture code stays thin):
 * - `DepthFloat32` (LiDAR / TrueDepth): meters, used directly.
 * - `DisparityFloat32/16` (dual camera): converted with calibration data
 *   via depth = focalPx * baselineM / disparity. Without calibration data
 *   the conversion returns nil and the caller falls back silently.
 * - Invalid samples (<= 0, NaN, infinite) are dropped at conversion time.
 * - Long edge is capped at 256 px by stride sampling: a full-res photo
 *   depth map (tens of MB as float32) never survives this function.
 */
public struct PhotoDepthMap: Sendable {
    public let width: Int
    public let height: Int
    /// Row-major meters, `width * height` entries, invalid already removed
    /// (stored as .nan and skipped by the sampler).
    public let meters: [Float]
    /// `lidar_avdepth_v1` / `disparity_avdepth_v1` (see DepthCapability).
    public let source: String
    /// `AVDepthData.quality` raw value at capture (0 = low, 1 = high).
    public let quality: Int

    public init(width: Int, height: Int, meters: [Float], source: String, quality: Int) {
        self.width = width
        self.height = height
        self.meters = meters
        self.source = source
        self.quality = quality
    }

    public var isValid: Bool { width > 0 && height > 0 && meters.count == width * height }

    /// Thin AVFoundation bridge. Returns nil when conversion is impossible;
    /// callers treat nil as "no depth on this keyframe" (fallback path).
    public static func from(
        depthData: AVDepthData,
        calibration: AVCameraCalibrationData?,
        source: String,
        maxEdge: Int = 256
    ) -> PhotoDepthMap? {
        let type = depthData.depthDataType
        if type == kCVPixelFormatType_DepthFloat32 {
            return fromAbsoluteMap(depthData.depthDataMap, source: source,
                                   quality: depthData.depthDataQuality, maxEdge: maxEdge)
        }
        if type == kCVPixelFormatType_DisparityFloat32 || type == kCVPixelFormatType_DisparityFloat16 {
            // Prefer an absolute conversion done by the framework when possible.
            let converted = depthData.converting(toDepthDataType: kCVPixelFormatType_DepthFloat32)
            if CVPixelBufferGetPixelFormatType(converted.depthDataMap) == kCVPixelFormatType_DepthFloat32 {
                return fromAbsoluteMap(converted.depthDataMap, source: source,
                                       quality: depthData.depthDataQuality, maxEdge: maxEdge)
            }
            return fromDisparityMap(depthData.depthDataMap, calibration: calibration, source: source,
                                    quality: depthData.depthDataQuality, maxEdge: maxEdge)
        }
        return nil
    }

    // MARK: - Private converters

    private static func fromAbsoluteMap(
        _ map: CVPixelBuffer, source: String, quality: AVDepthData.Quality, maxEdge: Int
    ) -> PhotoDepthMap? {
        guard CVPixelBufferGetPixelFormatType(map) == kCVPixelFormatType_DepthFloat32 else { return nil }
        let w = CVPixelBufferGetWidth(map)
        let h = CVPixelBufferGetHeight(map)
        guard w > 0 && h > 0 else { return nil }
        let step = max(1, max(w, h) / max(1, maxEdge))
        let outW = (w + step - 1) / step
        let outH = (h + step - 1) / step
        CVPixelBufferLockBaseAddress(map, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(map, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(map) else { return nil }
        let rowBytes = CVPixelBufferGetBytesPerRow(map)
        var out: [Float] = []
        out.reserveCapacity(outW * outH)
        for y in stride(from: 0, to: h, by: step) {
            let row = base.advanced(by: y * rowBytes).assumingMemoryBound(to: Float.self)
            for x in stride(from: 0, to: w, by: step) {
                let v = row[x]
                out.append(v.isFinite && v > 0 ? v : .nan)
            }
        }
        return PhotoDepthMap(width: outW, height: outH, meters: out,
                             source: source, quality: quality == .high ? 1 : 0)
    }

    private static func fromDisparityMap(
        _ map: CVPixelBuffer, calibration: AVCameraCalibrationData?,
        source: String, quality: AVDepthData.Quality, maxEdge: Int
    ) -> PhotoDepthMap? {
        guard let calibration else { return nil }
        let format = CVPixelBufferGetPixelFormatType(map)
        let isFloat16 = (format == kCVPixelFormatType_DisparityFloat16)
        guard format == kCVPixelFormatType_DisparityFloat32 || isFloat16 else { return nil }
        // depth(m) = focalPx * baselineM / disparityPx (Apple depth-capture guidance).
        let focalPx = calibration.intrinsicMatrix.columns.0.x
        let t = calibration.extrinsicMatrix.columns.3
        let baselineM = sqrt(t.x * t.x + t.y * t.y + t.z * t.z)
        guard focalPx > 0 && baselineM > 0 else { return nil }

        let w = CVPixelBufferGetWidth(map)
        let h = CVPixelBufferGetHeight(map)
        guard w > 0 && h > 0 else { return nil }
        let step = max(1, max(w, h) / max(1, maxEdge))
        let outW = (w + step - 1) / step
        let outH = (h + step - 1) / step
        CVPixelBufferLockBaseAddress(map, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(map, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(map) else { return nil }
        let rowBytes = CVPixelBufferGetBytesPerRow(map)
        var out: [Float] = []
        out.reserveCapacity(outW * outH)
        for y in stride(from: 0, to: h, by: step) {
            for x in stride(from: 0, to: w, by: step) {
                let d: Float
                if isFloat16 {
                    let row = base.advanced(by: y * rowBytes).assumingMemoryBound(to: UInt16.self)
                    d = Float(Float16(bitPattern: row[x]))
                } else {
                    let row = base.advanced(by: y * rowBytes).assumingMemoryBound(to: Float.self)
                    d = row[x]
                }
                if d.isFinite && d > 1e-6 {
                    out.append(focalPx * baselineM / d)
                } else {
                    out.append(.nan)
                }
            }
        }
        return PhotoDepthMap(width: outW, height: outH, meters: out,
                             source: source, quality: quality == .high ? 1 : 0)
    }
}
