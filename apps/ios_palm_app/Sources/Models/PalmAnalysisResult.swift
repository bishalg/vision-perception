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
            let controlPoints = CreaseSpline.resampleToControlPoints(pts, targetCount: 4)
            splines.append(
                CreaseSpline(
                    id: key,
                    lineType: type,
                    points: controlPoints,
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
 * Defined by 4 clean control points (Start, Curve 1, Curve 2, End) that form
 * one continuous, smoothly editable curve.
 */
public struct CreaseSpline: Identifiable, Equatable {
    public let id: String
    public var lineType: CreaseLineType
    /// Exactly 4-5 control points: Start, Curve Mid 1, Curve Mid 2, and End.
    public var points: [CGPoint]
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
        // Guarantee clean control points: reduce 100+ dense points down to 4 control points
        self.points = points.count > 5 ? Self.resampleToControlPoints(points, targetCount: 4) : points
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
        let newPts = points ?? self.points
        let cleanPts = newPts.count > 5 ? Self.resampleToControlPoints(newPts, targetCount: 4) : newPts
        return CreaseSpline(
            id: id,
            lineType: lineType ?? self.lineType,
            points: cleanPts,
            confidence: confidence,
            tortuosity: tortuosity,
            arcLengthPx: arcLengthPx,
            isUserAdjusted: markAdjusted ? true : isUserAdjusted
        )
    }

    /// Reduce any raw polyline (even with 100+ dense detector points) into a clean,
    /// editable 4-point control curve: Start, Curve Mid 1, Curve Mid 2, and End.
    public static func resampleToControlPoints(_ rawPoints: [CGPoint], targetCount: Int = 4) -> [CGPoint] {
        guard rawPoints.count > targetCount else { return rawPoints }
        guard targetCount >= 2 else { return rawPoints }

        var cumulative: [CGFloat] = [0]
        var totalLength: CGFloat = 0
        for i in 1..<rawPoints.count {
            let dx = rawPoints[i].x - rawPoints[i-1].x
            let dy = rawPoints[i].y - rawPoints[i-1].y
            let dist = sqrt(dx * dx + dy * dy)
            totalLength += dist
            cumulative.append(totalLength)
        }

        guard totalLength > 1e-5 else {
            return [rawPoints.first ?? .zero, rawPoints.last ?? .zero]
        }

        var resampled: [CGPoint] = []
        resampled.append(rawPoints.first!)

        for k in 1..<(targetCount - 1) {
            let targetDist = totalLength * (CGFloat(k) / CGFloat(targetCount - 1))
            var segIndex = 0
            while segIndex < cumulative.count - 2 && cumulative[segIndex + 1] < targetDist {
                segIndex += 1
            }
            let d0 = cumulative[segIndex]
            let d1 = cumulative[segIndex + 1]
            let segLen = d1 - d0
            let t = segLen > 1e-6 ? (targetDist - d0) / segLen : 0
            let p0 = rawPoints[segIndex]
            let p1 = rawPoints[segIndex + 1]
            let interp = CGPoint(
                x: p0.x + (p1.x - p0.x) * t,
                y: p0.y + (p1.y - p0.y) * t
            )
            resampled.append(interp)
        }

        resampled.append(rawPoints.last!)
        return resampled
    }

    /// Evaluates smooth continuous points along the Catmull-Rom spline for downstream analysis & predictions.
    public func evaluateDensePoints(samplesPerSegment: Int = 16) -> [CGPoint] {
        guard points.count >= 2 else { return points }
        if points.count == 2 { return points }
        var result: [CGPoint] = []
        for i in 0..<(points.count - 1) {
            let p0 = i > 0 ? points[i - 1] : points[i]
            let p1 = points[i]
            let p2 = points[i + 1]
            let p3 = (i + 2 < points.count) ? points[i + 2] : p2

            for step in 0..<samplesPerSegment {
                let t = CGFloat(step) / CGFloat(samplesPerSegment)
                let t2 = t * t
                let t3 = t2 * t
                let x = 0.5 * ((2.0 * p1.x) +
                               (-p0.x + p2.x) * t +
                               (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2 +
                               (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3)
                let y = 0.5 * ((2.0 * p1.y) +
                               (-p0.y + p2.y) * t +
                               (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2 +
                               (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3)
                result.append(CGPoint(x: x, y: y))
            }
        }
        if let last = points.last { result.append(last) }
        return result
    }

    /// Payload fragment for prediction / VLM grounding (continuous curve points).
    public var predictionPointsImageNorm: [[Double]] {
        let dense = evaluateDensePoints(samplesPerSegment: 8)
        return dense.map { [Double($0.x), Double($0.y)] }
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
        case .heart: return "#FF2D55" // Crimson Rose
        case .head: return "#007AFF"  // Sapphire Blue (pure vibrant blue, completely distinct from green)
        case .life: return "#34C759"  // Vibrant Emerald Green
        case .fate: return "#AF52DE"  // Royal Violet
        }
    }
}

/// Source of the overlay currently shown on the result screen.
public enum CreaseOverlaySource: String {
    case landmarkFallback
    case nativeEngine
}
