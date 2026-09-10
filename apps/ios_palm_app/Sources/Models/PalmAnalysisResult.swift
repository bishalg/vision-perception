import Foundation
import CoreGraphics

/**
 * Parsed domain model representing CurvEngine palm extraction output.
 * Fully decoupled from UI views.
 */
public struct MajorLineInfo: Codable, Identifiable {
    public var id: String { name }
    public let name: String
    public let present: Bool
    public let confidence: Float
    public let segmentIds: [Int]
    public let arcLengthPx: Float
    public let chordLengthPx: Float
    public let tortuosity: Float
    public let meanAngleDeg: Float
    public let startZone: String
    public let endZone: String
    public let gapCount: Int
    public let forkCount: Int
    public let meanWidthPx: Float
    public let meanIntensity: Float
    /// Image-normalized polylines (0…1, top-left origin) for overlay on the captured photo.
    public let pointsImageNorm: [[Float]]?

    enum CodingKeys: String, CodingKey {
        case name, present, confidence
        case segmentIds = "segment_ids"
        case arcLengthPx = "arc_length_px"
        case chordLengthPx = "chord_length_px"
        case tortuosity
        case meanAngleDeg = "mean_angle_deg"
        case startZone = "start_zone"
        case endZone = "end_zone"
        case gapCount = "gap_count"
        case forkCount = "fork_count"
        case meanWidthPx = "mean_width_px"
        case meanIntensity = "mean_intensity"
        case pointsImageNorm = "points_image_norm"
    }

    public var imagePoints: [CGPoint] {
        guard let pointsImageNorm else { return [] }
        return pointsImageNorm.compactMap { pair in
            guard pair.count >= 2 else { return nil }
            return CGPoint(x: CGFloat(pair[0]), y: CGFloat(pair[1]))
        }
    }
}

public struct PalmDomainPayload: Codable {
    public let majorLines: [String: MajorLineInfo]?
    public let dominantLine: String?
    public let heartHeadMinDistancePx: Float?

    enum CodingKeys: String, CodingKey {
        case majorLines = "major_lines"
        case dominantLine = "dominant_line"
        case heartHeadMinDistancePx = "heart_head_min_distance_px"
    }
}

public struct PalmAnalysisResponse: Codable {
    public let isUsable: Bool?
    public let totalDurationUs: Int?
    public let domainPayload: PalmDomainPayload?

    enum CodingKeys: String, CodingKey {
        case isUsable = "is_usable"
        case totalDurationUs = "total_duration_us"
        case domainPayload = "domain_payload"
    }

    public static func from(jsonString: String) -> PalmAnalysisResponse? {
        guard let data = jsonString.data(using: .utf8) else { return nil }
        do {
            return try JSONDecoder().decode(PalmAnalysisResponse.self, from: data)
        } catch {
            // Keep a short prefix so Console shows which field failed without dumping MBs.
            let prefix = String(jsonString.prefix(280))
            print("[PalmAnalysisResponse] JSON decode failed: \(error). prefix=\(prefix)")
            return nil
        }
    }

    /// Build overlay splines from Steger polylines (`points_image_norm`). Returns nil if none present.
    public func creaseSplinesFromPolylines() -> [CreaseSpline]? {
        guard let lines = domainPayload?.majorLines else { return nil }
        let keyMap: [(String, CreaseLineType)] = [
            ("heart_line", .heart),
            ("head_line", .head),
            ("life_line", .life),
            ("fate_line", .fate)
        ]
        var splines: [CreaseSpline] = []
        for (key, type) in keyMap {
            guard let info = lines[key], info.present else { continue }
            let pts = info.imagePoints
            guard pts.count >= 2 else { continue }
            splines.append(
                CreaseSpline(
                    id: key,
                    lineType: type,
                    points: pts,
                    confidence: info.confidence,
                    tortuosity: info.tortuosity,
                    arcLengthPx: info.arcLengthPx
                )
            )
        }
        return splines.isEmpty ? nil : splines
    }

    public static func from(data: Data) -> PalmAnalysisResponse? {
        try? JSONDecoder().decode(PalmAnalysisResponse.self, from: data)
    }
}

/**
 * UI display model for rendering Catmull-Rom crease splines on screen.
 * Mutable copies support drag / reorder / reassign before prediction.
 */
public struct CreaseSpline: Identifiable, Equatable {
    public let id: String
    public var lineType: CreaseLineType
    public var points: [CGPoint] // Normalized (0..1) coordinates in image space
    public var confidence: Float
    public var tortuosity: Float
    public var arcLengthPx: Float
    /// True when the user moved vertices, reordered, or reassigned the label.
    public var isUserAdjusted: Bool

    public init(
        id: String = UUID().uuidString,
        lineType: CreaseLineType,
        points: [CGPoint],
        confidence: Float,
        tortuosity: Float,
        arcLengthPx: Float,
        isUserAdjusted: Bool = false
    ) {
        self.id = id
        self.lineType = lineType
        self.points = points
        self.confidence = confidence
        self.tortuosity = tortuosity
        self.arcLengthPx = arcLengthPx
        self.isUserAdjusted = isUserAdjusted
    }

    public func replacing(
        lineType: CreaseLineType? = nil,
        points: [CGPoint]? = nil,
        markAdjusted: Bool = true
    ) -> CreaseSpline {
        CreaseSpline(
            id: id,
            lineType: lineType ?? self.lineType,
            points: points ?? self.points,
            confidence: confidence,
            tortuosity: tortuosity,
            arcLengthPx: arcLengthPx,
            isUserAdjusted: markAdjusted ? true : isUserAdjusted
        )
    }

    /// Payload fragment for prediction / VLM grounding (image-normalized points).
    public var predictionPointsImageNorm: [[Double]] {
        points.map { [Double($0.x), Double($0.y)] }
    }

    public var engineKey: String { "\(lineType.rawValue)_line" }
}

public enum CreaseLineType: String, CaseIterable, Identifiable {
    case heart = "heart"
    case head = "head"
    case life = "life"
    case fate = "fate"

    public var id: String { rawValue }

    public var displayNameKey: String {
        switch self {
        case .heart: return "line.heart"
        case .head: return "line.head"
        case .life: return "line.life"
        case .fate: return "line.fate"
        }
    }

    public var hexColor: String {
        switch self {
        case .heart: return "#F43F5E" // Crimson Rose
        case .head: return "#06B6D4"  // Electric Cyan
        case .life: return "#10B981"  // Emerald Green
        case .fate: return "#8B5CF6"  // Royal Violet
        }
    }
}

/// Source of the overlay currently shown on the result screen.
public enum CreaseOverlaySource: String {
    case landmarkFallback
    case nativeEngine
}
