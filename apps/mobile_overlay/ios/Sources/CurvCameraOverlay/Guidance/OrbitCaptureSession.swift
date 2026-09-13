import CoreGraphics
import Foundation

/**
 * One keyframe in a Mode B orbit scan.
 */
public struct OrbitFrame: Codable, Identifiable, Sendable {
    public let id: UUID
    public let bin: OrbitBin
    public let timestamp: TimeInterval
    /// Normalized 21 MediaPipe-order landmarks.
    public let landmarks: [[Float]]
    /// Device gravity vector at capture (x, y, z).
    public let gravity: [Float]
    public let deviceRollDeg: Float
    public let intrinsics: CameraIntrinsics
    public let imageWidth: Int
    public let imageHeight: Int
    public let digitSpreadOk: Bool
    public let maxJointDrift: Float
    /// Optional on-disk image path (relative to session folder).
    public var imagePath: String?

    public init(
        id: UUID = UUID(),
        bin: OrbitBin,
        timestamp: TimeInterval,
        landmarks: [[Float]],
        gravity: [Float],
        deviceRollDeg: Float,
        intrinsics: CameraIntrinsics,
        imageWidth: Int,
        imageHeight: Int,
        digitSpreadOk: Bool,
        maxJointDrift: Float,
        imagePath: String? = nil
    ) {
        self.id = id
        self.bin = bin
        self.timestamp = timestamp
        self.landmarks = landmarks
        self.gravity = gravity
        self.deviceRollDeg = deviceRollDeg
        self.intrinsics = intrinsics
        self.imageWidth = imageWidth
        self.imageHeight = imageHeight
        self.digitSpreadOk = digitSpreadOk
        self.maxJointDrift = maxJointDrift
        self.imagePath = imagePath
    }
}

/**
 * Full orbit capture session — serializable evidence bag for mount relief + mosaic.
 */
public struct OrbitCaptureSession: Codable, Sendable {
    public static let schemaVersion = "curv_orbit_session_v1"

    public let schema: String
    public let handSide: String
    public let startedAt: TimeInterval
    public var completedAt: TimeInterval?
    public var frames: [OrbitFrame]
    public var referenceLandmarks: [[Float]]?

    public init(
        handSide: String,
        startedAt: TimeInterval = Date().timeIntervalSince1970,
        frames: [OrbitFrame] = [],
        referenceLandmarks: [[Float]]? = nil
    ) {
        self.schema = Self.schemaVersion
        self.handSide = handSide
        self.startedAt = startedAt
        self.completedAt = nil
        self.frames = frames
        self.referenceLandmarks = referenceLandmarks
    }

    public var filledBins: Set<OrbitBin> {
        Set(frames.map(\.bin))
    }

    public var coverageFraction: Float {
        Float(filledBins.count) / Float(OrbitBin.allCases.count)
    }

    public func mountViewCounts() -> [String: Int] {
        var counts: [String: Int] = [:]
        for frame in frames {
            for mount in frame.bin.primaryMounts {
                counts[mount, default: 0] += 1
            }
        }
        return counts
    }

    public func toJSONData() throws -> Data {
        try JSONEncoder().encode(self)
    }

    public static func fromJSONData(_ data: Data) throws -> OrbitCaptureSession {
        try JSONDecoder().decode(OrbitCaptureSession.self, from: data)
    }
}
