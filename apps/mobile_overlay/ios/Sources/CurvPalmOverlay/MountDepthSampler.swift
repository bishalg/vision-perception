import CurvCameraOverlay
import Foundation

/**
 * Samples per-mount mean depth from a hardware depth map.
 *
 * Pure function over plain arrays (no AVFoundation), so it is fully
 * unit-testable on Simulator. The only Apple type it touches is
 * `PhotoDepthMap`, which is itself a Sendable value snapshot.
 *
 * Coordinate contract: `landmarks` are normalized points in the upright
 * still-image space, and the depth map is registered to that same still
 * (true for `AVCapturePhoto.depthData`). Preview-space landmarks transfer
 * approximately (both are portrait-normalized); each probe samples a disc
 * and takes the median, which absorbs small misalignments.
 *
 * Robustness rules: samples outside 0.05...3.0 m are background (rejected);
 * a mount needs >= 3 valid samples; the frame needs >= 4 valid mounts.
 * Anything less returns nil and the estimator falls back automatically.
 */
public enum MountDepthSampler {
    /// Single Swift source of truth for mount probe indices. Must match
    /// `mountAnchors()` in MountReliefAnalyzer.cpp.
    public static let anchors: [String: [Int]] = [
        "venus": [1, 2, 5],
        "jupiter": [5, 6, 7],
        "saturn": [9, 10, 11],
        "apollo": [13, 14, 15],
        "mercury": [17, 18, 19],
        "luna": [0, 17, 18],
        "mars": [5, 9, 13, 17]
    ]

    public static let minValidSamplesPerMount = 3
    public static let minValidMounts = 4
    private static let minDepthM: Float = 0.05
    private static let maxDepthM: Float = 3.0

    public static func sampleMountMeansMm(
        depthMap: PhotoDepthMap,
        landmarks: [[Float]],
        minValidMounts: Int = minValidMounts
    ) -> [String: Float]? {
        guard depthMap.isValid, landmarks.count >= 21 else { return nil }
        let radius = max(1, min(depthMap.width, depthMap.height) / 48)
        var means: [String: Float] = [:]
        for (mount, idxs) in anchors {
            var samples: [Float] = []
            for idx in idxs where idx >= 0 && idx < landmarks.count && landmarks[idx].count >= 2 {
                let cx = Int((landmarks[idx][0] * Float(depthMap.width)).rounded())
                let cy = Int((landmarks[idx][1] * Float(depthMap.height)).rounded())
                for dy in -radius ... radius {
                    for dx in -radius ... radius {
                        let x = cx + dx
                        let y = cy + dy
                        guard x >= 0, y >= 0, x < depthMap.width, y < depthMap.height else { continue }
                        let v = depthMap.meters[y * depthMap.width + x]
                        if v.isFinite && v >= minDepthM && v <= maxDepthM {
                            samples.append(v)
                        }
                    }
                }
            }
            guard samples.count >= minValidSamplesPerMount else { continue }
            means[mount] = samples.sorted()[samples.count / 2] * 1000
        }
        return means.count >= minValidMounts ? means : nil
    }
}
