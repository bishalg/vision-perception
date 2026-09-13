import Foundation

/**
 * Phase 3 close-up mosaic tiles — coach user to fill frame with palm regions.
 */
public enum CloseUpTile: String, CaseIterable, Codable, Sendable {
    case thenar = "thenar"       // Life / Venus
    case distal = "distal"       // Heart band
    case midPalm = "mid_palm"    // Head / Fate
    case ulnar = "ulnar"         // Luna / Mercury

    public var displayName: String {
        switch self {
        case .thenar: return "Move closer — thenar (Venus / Life)"
        case .distal: return "Close-up: upper palm (Heart lines)"
        case .midPalm: return "Close-up: mid palm (Head / Fate)"
        case .ulnar: return "Close-up: pinky side (Moon mount)"
        }
    }

    /// Minimum pose.scale to accept this tile (higher = closer).
    public var minScale: Float {
        switch self {
        case .thenar: return 0.52
        case .distal: return 0.48
        case .midPalm: return 0.46
        case .ulnar: return 0.50
        }
    }
}

public struct CloseUpTileFrame: Codable, Identifiable, Sendable {
    public let id: UUID
    public let tile: CloseUpTile
    public let timestamp: TimeInterval
    public let landmarks: [[Float]]
    public let intrinsics: CameraIntrinsics
    public let imageWidth: Int
    public let imageHeight: Int
    public var imagePath: String?

    public init(
        id: UUID = UUID(),
        tile: CloseUpTile,
        timestamp: TimeInterval,
        landmarks: [[Float]],
        intrinsics: CameraIntrinsics,
        imageWidth: Int,
        imageHeight: Int,
        imagePath: String? = nil
    ) {
        self.id = id
        self.tile = tile
        self.timestamp = timestamp
        self.landmarks = landmarks
        self.intrinsics = intrinsics
        self.imageWidth = imageWidth
        self.imageHeight = imageHeight
        self.imagePath = imagePath
    }
}

public struct CloseUpMosaicSession: Codable, Sendable {
    public static let schemaVersion = "curv_closeup_mosaic_v1"

    public let schema: String
    public var tiles: [CloseUpTileFrame]

    public init(tiles: [CloseUpTileFrame] = []) {
        self.schema = Self.schemaVersion
        self.tiles = tiles
    }

    public var filledTiles: Set<CloseUpTile> { Set(tiles.map(\.tile)) }
}

public final class CloseUpTileGuidance {
    public var thresholds: CaptureThresholds

    public init(thresholds: CaptureThresholds = CaptureThresholds()) {
        self.thresholds = thresholds
    }

    public func nextTile(in session: CloseUpMosaicSession) -> CloseUpTile? {
        CloseUpTile.allCases.first { !session.filledTiles.contains($0) }
    }

    public func evaluate(
        pose: SubjectPose?,
        photometrics: PhotometricSample,
        target: CloseUpTile
    ) -> (ready: Bool, message: String) {
        guard let pose else {
            return (false, "Show palm for \(target.displayName)")
        }
        let illumOk = photometrics.meanLuminance >= thresholds.minLuminance &&
            photometrics.glareRatio <= thresholds.maxGlareRatio
        if pose.scale < target.minScale {
            return (false, target.displayName)
        }
        if !illumOk {
            return (false, "Improve lighting for close-up")
        }
        return (true, "Hold steady — capturing \(target.rawValue)")
    }
}
