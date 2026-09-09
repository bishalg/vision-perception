import Foundation
import Vision
import CoreVideo
import os.log

/**
 * High-performance hand pose tracker using Apple Vision Framework.
 * Runs on the Apple Neural Engine (ANE) with near-zero memory footprint.
 */
public final class VisionHandTracker {
    private let logger = Logger(subsystem: "org.curv.palm", category: "VisionHandTracker")
    private let handPoseRequest = VNDetectHumanHandPoseRequest()

    // Keypoint mapping in canonical MediaPipe 21-point order:
    // 0: Wrist
    // 1-4: Thumb (cmc, mcp, ip, tip)
    // 5-8: Index (mcp, pip, dip, tip)
    // 9-12: Middle (mcp, pip, dip, tip)
    // 13-16: Ring (mcp, pip, dip, tip)
    // 17-20: Little (mcp, pip, dip, tip)
    private let jointKeys: [VNHumanHandPoseObservation.JointName] = [
        .wrist,
        .thumbCMC, .thumbMP, .thumbIP, .thumbTip,
        .indexMCP, .indexPIP, .indexDIP, .indexTip,
        .middleMCP, .middlePIP, .middleDIP, .middleTip,
        .ringMCP, .ringPIP, .ringDIP, .ringTip,
        .littleMCP, .littlePIP, .littleDIP, .littleTip
    ]

    public init() {
        handPoseRequest.maximumHandCount = 1
    }

    /**
     * Extracts 21 normalized landmarks from an incoming video frame CVPixelBuffer.
     * Output coordinates: x in [0, 1], y in [0, 1] with origin (0, 0) at top-left.
     */
    public func processFrame(
        _ pixelBuffer: CVPixelBuffer,
        orientation: CGImagePropertyOrientation = .up
    ) -> [CGPoint]? {
        let handler = VNImageRequestHandler(cvPixelBuffer: pixelBuffer, orientation: orientation, options: [:])
        do {
            try handler.perform([handPoseRequest])
            guard let observation = handPoseRequest.results?.first else {
                return nil
            }

            var points: [CGPoint] = []
            points.reserveCapacity(21)

            for key in jointKeys {
                guard let point = try? observation.recognizedPoint(key), point.confidence > 0.2 else {
                    return nil // Incomplete hand
                }
                // Vision has (0, 0) at bottom-left; flip y to standard top-left
                points.append(CGPoint(x: point.location.x, y: 1.0 - point.location.y))
            }

            return points
        } catch {
            logger.error("Vision hand pose detection failed: \(error.localizedDescription)")
            return nil
        }
    }

    /**
     * Converts an array of 21 CGPoints to 42 flat normalized floats [x0, y0, x1, y1, ...]
     * for direct ingestion by CurvEngine C-ABI.
     */
    public static func toFlatLandmarksArray(_ points: [CGPoint]) -> [Float] {
        var flat = [Float]()
        flat.reserveCapacity(points.count * 2)
        for pt in points {
            flat.append(Float(pt.x))
            flat.append(Float(pt.y))
        }
        return flat
    }
}
