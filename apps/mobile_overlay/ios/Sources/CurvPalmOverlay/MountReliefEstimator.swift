import CoreGraphics
import CurvCameraOverlay
import Foundation

/**
 * Swift-side mount prominence from orbit keyframes.
 *
 * Mirrors the native pipeline (`SurfaceRelief::estimateMetricRelief` +
 * `MountReliefAnalyzer`): tilt-exposure prior fused with multi-view parallax,
 * scaled to millimeters via the metacarpal-breadth prior, with an override
 * path for measured hardware depth. Anchor indices, weights, and source
 * strings must stay in sync with the C++ implementation.
 */
public enum MountProminence: String, Codable, Sendable {
    case deficient, average, elevated
}

public struct MountReliefEntry: Codable, Identifiable, Sendable {
    public var id: String { mount }
    public let mount: String
    public let heightMm: Float?
    public let heightRelative: Float
    public let prominence: MountProminence
    public let confidence: Float
    public let viewCount: Int

    public init(
        mount: String,
        heightMm: Float?,
        heightRelative: Float,
        prominence: MountProminence,
        confidence: Float,
        viewCount: Int
    ) {
        self.mount = mount
        self.heightMm = heightMm
        self.heightRelative = heightRelative
        self.prominence = prominence
        self.confidence = confidence
        self.viewCount = viewCount
    }
}

public struct MountReliefMap: Codable, Sendable {
    public let mounts: [MountReliefEntry]
    public let venusElevated: Bool
    public let source: String

    public init(mounts: [MountReliefEntry], venusElevated: Bool, source: String) {
        self.mounts = mounts
        self.venusElevated = venusElevated
        self.source = source
    }

    public func entry(for mount: PalmMount) -> MountReliefEntry? {
        mounts.first { $0.mount == mount.rawValue }
    }
}

public enum MountReliefEstimator {
    /// Shared anatomical scale prior (mm). Must match `MetricScale` default.
    public static let defaultPalmWidthMm: Float = 80.0
    private static let minTiltRad: Float = 0.05
    private static let maxReliefMm: Float = 6.0

    private static let mountNames = ["jupiter", "saturn", "apollo", "mercury", "venus", "luna", "mars"]

    /// Must match `mountAnchors()` in MountReliefAnalyzer.cpp.
    private static let anchors: [String: [Int]] = [
        "venus": [1, 2, 5],
        "jupiter": [5, 6, 7],
        "saturn": [9, 10, 11],
        "apollo": [13, 14, 15],
        "mercury": [17, 18, 19],
        "luna": [0, 17, 18],
        "mars": [5, 9, 13, 17]
    ]

    /// Estimate relative mount heights from an orbit session.
    public static func estimate(from session: OrbitCaptureSession) -> MountReliefMap {
        guard !session.frames.isEmpty else {
            return MountReliefMap(mounts: [], venusElevated: false, source: "orbit_empty")
        }
        let palmWidthMm = session.metricPalmWidthMm ?? defaultPalmWidthMm

        // Tilt-exposure prior: same per-view accumulation as native.
        var tiltAcc: [String: Float] = [:]
        var viewCounts: [String: Int] = [:]
        for frame in session.frames {
            let gz = frame.gravity.count >= 3 ? frame.gravity[2] : 0
            let tilt = abs(gz) + abs(frame.deviceRollDeg) * 0.01
            for mount in frame.bin.primaryMounts {
                tiltAcc[mount, default: 0] += tilt + 0.15
                viewCounts[mount, default: 0] += 1
            }
        }
        let (tiltMean, tiltStd) = meanStd(mountNames, acc: tiltAcc, counts: viewCounts)

        // Parallax path needs the flat reference view and a pixel scale.
        let ref = session.referenceLandmarks ?? []
        var palmWidthPx: Float = 0
        for frame in session.frames where frame.landmarks.count >= 18 {
            let w = dist(frame.landmarks[5], frame.landmarks[17]) * Float(frame.imageWidth)
            palmWidthPx = max(palmWidthPx, w)
        }
        let metricReady = ref.count >= 21 && palmWidthPx > 0.001 && palmWidthMm > 0.001
        let center = centroid(ref)

        var parAcc: [String: Float] = [:]
        var parCounts: [String: Int] = [:]
        var depthAcc: [String: Float] = [:]
        var depthCounts: [String: Int] = [:]

        for frame in session.frames {
            if let measured = frame.depthMm {
                for (mount, mm) in measured {
                    depthAcc[mount, default: 0] += mm
                    depthCounts[mount, default: 0] += 1
                }
            }
            guard metricReady,
                  frame.landmarks.count == ref.count,
                  frame.imageWidth > 0
            else { continue }
            let theta = abs(tiltAngle(frame.gravity, vs: session.referenceGravity))
            guard theta >= minTiltRad else { continue }
            var dxs: [Float] = []
            var dys: [Float] = []
            dxs.reserveCapacity(ref.count)
            dys.reserveCapacity(ref.count)
            for i in 0 ..< ref.count {
                dxs.append(frame.landmarks[i][0] - ref[i][0])
                dys.append(frame.landmarks[i][1] - ref[i][1])
            }
            let tx = median(dxs)
            let ty = median(dys)
            var radials: [Float] = []
            radials.reserveCapacity(ref.count)
            for i in 0 ..< ref.count {
                guard let dir = unit(ref[i][0] - center.x, ref[i][1] - center.y) else { continue }
                radials.append(((dxs[i] - tx) * dir.0 + (dys[i] - ty) * dir.1) * Float(frame.imageWidth))
            }
            guard !radials.isEmpty else { continue }
            let planePx = median(radials)
            let mmPerPx = palmWidthMm / palmWidthPx
            for mount in frame.bin.primaryMounts {
                guard let idxs = anchors[mount], !idxs.isEmpty else { continue }
                var sum: Float = 0
                var n = 0
                for idx in idxs where idx >= 0 && idx < ref.count {
                    guard let dir = unit(ref[idx][0] - center.x, ref[idx][1] - center.y) else { continue }
                    sum += ((dxs[idx] - tx) * dir.0 + (dys[idx] - ty) * dir.1) * Float(frame.imageWidth)
                    n += 1
                }
                guard n > 0 else { continue }
                parAcc[mount, default: 0] += abs(sum / Float(n) - planePx) * mmPerPx / theta
                parCounts[mount, default: 0] += 1
            }
        }
        let (parMean, parStd) = meanStd(mountNames, acc: parAcc, counts: parCounts)

        var measuredMeans: [Float] = []
        for mount in mountNames {
            if let c = depthCounts[mount], c > 0 {
                measuredMeans.append(depthAcc[mount, default: 0] / Float(c))
            }
        }
        let depthPlane = median(measuredMeans)

        var entries: [MountReliefEntry] = []
        for mount in mountNames {
            let views = viewCounts[mount, default: 0]
            guard views > 0 else {
                entries.append(MountReliefEntry(
                    mount: mount, heightMm: nil, heightRelative: 0,
                    prominence: .average, confidence: 0, viewCount: 0
                ))
                continue
            }
            let tiltZ = zscore(tiltAcc[mount, default: 0] / Float(views), mean: tiltMean, std: tiltStd)
            var fused = tiltZ
            var heightMm: Float?
            if let pc = parCounts[mount], pc > 0 {
                let parZ = zscore(parAcc[mount, default: 0] / Float(pc), mean: parMean, std: parStd)
                fused = 0.35 * tiltZ + 0.65 * parZ
                let mag = min(maxReliefMm, parAcc[mount, default: 0] / Float(pc))
                heightMm = (fused >= 0 ? 1 : -1) * mag
            }
            if let dc = depthCounts[mount], dc > 0 {
                heightMm = depthAcc[mount, default: 0] / Float(dc) - depthPlane
            }
            let prominence: MountProminence = fused > 0.45 ? .elevated : (fused < -0.45 ? .deficient : .average)
            let conf = min(0.95, 0.35 + Float(views) * 0.1)
            entries.append(MountReliefEntry(
                mount: mount, heightMm: heightMm, heightRelative: fused,
                prominence: prominence, confidence: conf, viewCount: views
            ))
        }

        let allMeasured = !entries.isEmpty && entries.allSatisfy { $0.heightMm != nil && depthCounts[$0.mount] != nil }
        let anyMetric = entries.contains { $0.heightMm != nil }
        let source = allMeasured ? "orbit_depth_v1" : (anyMetric ? "orbit_metric_v1" : "orbit_tilt_proxy_v1")
        let venusElevated = entries.first { $0.mount == "venus" }?.prominence == .elevated
        return MountReliefMap(mounts: entries, venusElevated: venusElevated, source: source)
    }

    // MARK: - Small math helpers (mirror of the native implementation)

    private static func tiltAngle(_ gravity: [Float], vs reference: [Float]?) -> Float {
        guard gravity.count >= 3, let ref = reference, ref.count >= 3 else { return 0 }
        let dot = gravity[0] * ref[0] + gravity[1] * ref[1] + gravity[2] * ref[2]
        let na = sqrt(gravity[0] * gravity[0] + gravity[1] * gravity[1] + gravity[2] * gravity[2])
        let nb = sqrt(ref[0] * ref[0] + ref[1] * ref[1] + ref[2] * ref[2])
        guard na > 1e-6, nb > 1e-6 else { return 0 }
        return acos(min(1, max(-1, dot / (na * nb))))
    }

    private static func median(_ values: [Float]) -> Float {
        guard !values.isEmpty else { return 0 }
        let sorted = values.sorted()
        let mid = sorted.count / 2
        if sorted.count % 2 == 1 { return sorted[mid] }
        return (sorted[mid] + sorted[mid - 1]) * 0.5
    }

    private static func centroid(_ points: [[Float]]) -> (x: Float, y: Float) {
        guard !points.isEmpty else { return (0.5, 0.5) }
        var cx: Float = 0
        var cy: Float = 0
        for p in points where p.count >= 2 { cx += p[0]; cy += p[1] }
        return (cx / Float(points.count), cy / Float(points.count))
    }

    private static func unit(_ x: Float, _ y: Float) -> (Float, Float)? {
        let len = sqrt(x * x + y * y)
        guard len > 1e-6 else { return nil }
        return (x / len, y / len)
    }

    private static func dist(_ a: [Float], _ b: [Float]) -> Float {
        guard a.count >= 2, b.count >= 2 else { return 0 }
        let dx = a[0] - b[0]
        let dy = a[1] - b[1]
        return sqrt(dx * dx + dy * dy)
    }

    private static func meanStd(
        _ names: [String], acc: [String: Float], counts: [String: Int]
    ) -> (mean: Float, std: Float) {
        var values: [Float] = []
        for id in names {
            if let c = counts[id], c > 0 {
                values.append(acc[id, default: 0] / Float(c))
            }
        }
        guard !values.isEmpty else { return (0, 1) }
        let mean = values.reduce(0, +) / Float(values.count)
        let variance = values.map { ($0 - mean) * ($0 - mean) }.reduce(0, +) / Float(values.count)
        return (mean, sqrt(variance))
    }

    private static func zscore(_ v: Float, mean: Float, std: Float) -> Float {
        std > 1e-4 ? (v - mean) / std : 0
    }
}
