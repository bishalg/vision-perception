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
        return try? JSONDecoder().decode(PalmAnalysisResponse.self, from: data)
    }
}

/**
 * UI display model for rendering Catmull-Rom crease splines on screen.
 */
public struct CreaseSpline: Identifiable {
    public let id: String
    public let lineType: CreaseLineType
    public let points: [CGPoint] // Normalized (0..1) coordinates in camera frame
    public let confidence: Float
    public let tortuosity: Float
    public let arcLengthPx: Float

    public init(
        id: String = UUID().uuidString,
        lineType: CreaseLineType,
        points: [CGPoint],
        confidence: Float,
        tortuosity: Float,
        arcLengthPx: Float
    ) {
        self.id = id
        self.lineType = lineType
        self.points = points
        self.confidence = confidence
        self.tortuosity = tortuosity
        self.arcLengthPx = arcLengthPx
    }
}

public enum CreaseLineType: String, CaseIterable {
    case heart = "heart"
    case head = "head"
    case life = "life"
    case fate = "fate"

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
