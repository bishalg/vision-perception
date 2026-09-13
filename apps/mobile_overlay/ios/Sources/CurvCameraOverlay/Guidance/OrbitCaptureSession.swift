import CoreGraphics
import Foundation

/**
 * One keyframe in a Mode B orbit scan.
 *
 * JSON keys are snake_case to match the `curv_orbit_session_v1` contract
 * shared with the Android overlay kit and the native C++ analyzer
 * (`MountReliefAnalyzer`). Do not rename keys without updating all three.
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
    /// Measured per-mount depth in mm from hardware depth (LiDAR/TrueDepth).
    /// Nil until a depth source is wired; the estimator then skips the
    /// measured-depth path.
    public var depthMm: [String: Float]?
    public var depthSource: String?

    private enum CodingKeys: String, CodingKey {
        case id, bin, timestamp, landmarks, gravity, intrinsics
        case deviceRollDeg = "device_roll_deg"
        case imageWidth = "image_width"
        case imageHeight = "image_height"
        case digitSpreadOk = "digit_spread_ok"
        case maxJointDrift = "max_joint_drift"
        case imagePath = "image_path"
        case depthMm = "depth_mm"
        case depthSource = "depth_source"
    }

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
        imagePath: String? = nil,
        depthMm: [String: Float]? = nil,
        depthSource: String? = nil
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
        self.depthMm = depthMm
        self.depthSource = depthSource
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
    /// Device gravity at the flat reference view; the parallax tilt angle of
    /// each frame is measured against this. Set from the first recorded frame.
    public var referenceGravity: [Float]?
    /// Anatomical scale prior in mm (metacarpal breadth, landmark 5 -> 17).
    /// Nil selects the shared 80 mm default in the estimators.
    public var metricPalmWidthMm: Float?
    public var metricSource: String?

    private enum CodingKeys: String, CodingKey {
        case schema, frames
        case handSide = "hand_side"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case referenceLandmarks = "reference_landmarks"
        case referenceGravity = "reference_gravity"
        case metricPalmWidthMm = "metric_palm_width_mm"
        case metricSource = "metric_source"
    }

    public init(
        handSide: String,
        startedAt: TimeInterval = Date().timeIntervalSince1970,
        frames: [OrbitFrame] = [],
        referenceLandmarks: [[Float]]? = nil,
        referenceGravity: [Float]? = nil,
        metricPalmWidthMm: Float? = nil,
        metricSource: String? = nil
    ) {
        self.schema = Self.schemaVersion
        self.handSide = handSide
        self.startedAt = startedAt
        self.completedAt = nil
        self.frames = frames
        self.referenceLandmarks = referenceLandmarks
        self.referenceGravity = referenceGravity
        self.metricPalmWidthMm = metricPalmWidthMm
        self.metricSource = metricSource
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
