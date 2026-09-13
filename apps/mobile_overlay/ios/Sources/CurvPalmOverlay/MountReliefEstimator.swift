import CoreGraphics
import CurvCameraOverlay
import Foundation

/**
 * Swift-side mount prominence from orbit keyframes.
 * Uses palm-plane tilt proxy when LiDAR / native C++ relief is unavailable.
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
    private static let mountNames = ["jupiter", "saturn", "apollo", "mercury", "venus", "luna", "mars"]

    /// Estimate relative mount heights from orbit session gravity + landmark stability.
    public static func estimate(from session: OrbitCaptureSession) -> MountReliefMap {
        guard !session.frames.isEmpty else {
            return MountReliefMap(mounts: [], venusElevated: false, source: "orbit_empty")
        }

        let viewCounts = session.mountViewCounts()
        var heights: [String: Float] = [:]

        for mount in mountNames {
            let views = viewCounts[mount, default: 0]
            var tiltScore: Float = 0
            for frame in session.frames where frame.bin.primaryMounts.contains(mount) {
                // More tilt toward camera → higher apparent relief for thenar mounts.
                let gz = frame.gravity.count >= 3 ? frame.gravity[2] : 0
                tiltScore += abs(gz) + abs(frame.deviceRollDeg) * 0.01
            }
            let base = Float(views) * 0.15 + tiltScore * 0.08
            heights[mount] = base
        }

        // Venus-specific: thumb-toward bins boost thenar estimate.
        let venusBoost = session.frames.filter { $0.bin == .thumbToward || $0.bin == .tiltLeft }.count
        heights["venus", default: 0] += Float(venusBoost) * 0.12

        let values = mountNames.compactMap { heights[$0] }
        let mean = values.isEmpty ? 0 : values.reduce(0, +) / Float(values.count)
        let std = sqrt(values.map { ($0 - mean) * ($0 - mean) }.reduce(0, +) / Float(max(1, values.count)))

        var entries: [MountReliefEntry] = []
        for mount in mountNames {
            let h = heights[mount, default: mean]
            let rel = std > 1e-4 ? (h - mean) / std : 0
            let prominence: MountProminence = rel > 0.45 ? .elevated : (rel < -0.45 ? .deficient : .average)
            let conf = min(0.95, 0.35 + Float(viewCounts[mount, default: 0]) * 0.1)
            entries.append(MountReliefEntry(
                mount: mount,
                heightMm: nil,
                heightRelative: rel,
                prominence: prominence,
                confidence: conf,
                viewCount: viewCounts[mount, default: 0]
            ))
        }

        let venusRel = entries.first { $0.mount == "venus" }?.heightRelative ?? 0
        return MountReliefMap(
            mounts: entries,
            venusElevated: venusRel > 0.35,
            source: "orbit_tilt_proxy_v1"
        )
    }
}
