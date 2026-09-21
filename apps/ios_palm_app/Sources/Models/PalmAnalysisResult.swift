import Foundation
import CoreGraphics
import CoreVideo
import CurvCameraOverlay

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

public struct MountReliefEntryPayload: Codable {
    public let mount: String
    public let heightMm: Float?
    public let heightRelative: Float?
    public let prominence: String?
    public let confidence: Float?
    public let viewCount: Int?

    enum CodingKeys: String, CodingKey {
        case mount
        case heightMm = "height_mm"
        case heightRelative = "height_relative"
        case prominence, confidence
        case viewCount = "view_count"
    }
}

public struct MountReliefPayload: Codable {
    public let mounts: [MountReliefEntryPayload]?
    public let venusElevated: Bool?
    public let source: String?

    enum CodingKeys: String, CodingKey {
        case mounts
        case venusElevated = "venus_elevated"
        case source
    }
}

public struct MinorRidgeSegment: Codable, Identifiable {
    public var id: Int { segmentId }
    public let segmentId: Int
    public let arcLengthPx: Float?
    public let meanAngleDeg: Float?
    public let primaryZone: String?
    public let pointsCanonicalNorm: [[Float]]?

    enum CodingKeys: String, CodingKey {
        case segmentId = "segment_id"
        case arcLengthPx = "arc_length_px"
        case meanAngleDeg = "mean_angle_deg"
        case primaryZone = "primary_zone"
        case pointsCanonicalNorm = "points_canonical_norm"
    }
}

public struct FingertipPatternPayload: Codable {
    public let digit: String?
    public let pattern: String?
    public let ridgeFrequency: Float?
    public let quality: Float?

    enum CodingKeys: String, CodingKey {
        case digit, pattern
        case ridgeFrequency = "ridge_frequency"
        case quality
    }
}

public struct PalmDomainPayload: Codable {
    public let majorLines: [String: MajorLineInfo]?
    public let dominantLine: String?
    public let heartHeadMinDistancePx: Float?
    public let fateAnalysis: FateReport?
    public let heartAnalysis: LineReport?
    public let headAnalysis: LineReport?
    public let lifeAnalysis: LineReport?
    public let mountRelief: MountReliefPayload?
    public let minorRidges: [MinorRidgeSegment]?
    public let fingertipPattern: FingertipPatternPayload?
    public let fingertipPatterns: [FingertipPatternPayload]?

    enum CodingKeys: String, CodingKey {
        case majorLines = "major_lines"
        case dominantLine = "dominant_line"
        case heartHeadMinDistancePx = "heart_head_min_distance_px"
        case fateAnalysis = "fate_analysis"
        case heartAnalysis = "heart_analysis"
        case headAnalysis = "head_analysis"
        case lifeAnalysis = "life_analysis"
        case mountRelief = "mount_relief"
        case minorRidges = "minor_ridges"
        case fingertipPattern = "fingertip_pattern"
        case fingertipPatterns = "fingertip_patterns"
    }

    public init(
        majorLines: [String: MajorLineInfo]? = nil,
        dominantLine: String? = nil,
        heartHeadMinDistancePx: Float? = nil,
        fateAnalysis: FateReport? = nil,
        heartAnalysis: LineReport? = nil,
        headAnalysis: LineReport? = nil,
        lifeAnalysis: LineReport? = nil,
        mountRelief: MountReliefPayload? = nil,
        minorRidges: [MinorRidgeSegment]? = nil,
        fingertipPattern: FingertipPatternPayload? = nil,
        fingertipPatterns: [FingertipPatternPayload]? = nil
    ) {
        self.majorLines = majorLines
        self.dominantLine = dominantLine
        self.heartHeadMinDistancePx = heartHeadMinDistancePx
        self.fateAnalysis = fateAnalysis
        self.heartAnalysis = heartAnalysis
        self.headAnalysis = headAnalysis
        self.lifeAnalysis = lifeAnalysis
        self.mountRelief = mountRelief
        self.minorRidges = minorRidges
        self.fingertipPattern = fingertipPattern
        self.fingertipPatterns = fingertipPatterns
    }
}

/// Structural per-line report (heart/head/life). All-optional: older evidence
/// payloads predate these keys and must still decode.
public struct LineReport: Codable {
    public let present: Bool?
    public let isShort: Bool?
    public let isLong: Bool?
    public let broken: Bool?
    public let gapCount: Int?
    public let ageAtEnd: Float?
    public let simianMerge: Bool?
    public let mergedHeart: Bool?
    public let joinedLifeOrigin: Bool?
    public let terminatesFate: Bool?

    enum CodingKeys: String, CodingKey {
        case present
        case isShort = "is_short"
        case isLong = "is_long"
        case broken
        case gapCount = "gap_count"
        case ageAtEnd = "age_at_end"
        case simianMerge = "simian_merge"
        case mergedHeart = "merged_heart"
        case joinedLifeOrigin = "joined_life_origin"
        case terminatesFate = "terminates_fate"
    }
}

/// Fate-line report (FateLineAnalyzer). All-optional for the same reason.
public struct FateReport: Codable {
    public let present: Bool?
    public let noFate: Bool?
    public let origin: String?
    public let isShort: Bool?
    public let isLong: Bool?
    public let broken: Bool?
    public let gapCount: Int?
    public let stopsAtHeart: Bool?
    public let stopsAtHead: Bool?
    public let mergedHead: Bool?
    public let ageAtEnd: Float?
    public let ageAtBreak: Float?
    public let badSaturnMount: Bool?
    public let badMercuryMount: Bool?

    enum CodingKeys: String, CodingKey {
        case present
        case noFate = "no_fate"
        case origin
        case isShort = "is_short"
        case isLong = "is_long"
        case broken
        case gapCount = "gap_count"
        case stopsAtHeart = "stops_at_heart"
        case stopsAtHead = "stops_at_head"
        case mergedHead = "merged_head"
        case ageAtEnd = "age_at_end"
        case ageAtBreak = "age_at_break"
        case badSaturnMount = "bad_saturn_mount"
        case badMercuryMount = "bad_mercury_mount"
    }
}

/// Encoder helper for merging partial domain_payload updates.
struct PalmAnalysisResponseWrapper: Codable {
    let isUsable: Bool?
    let totalDurationUs: Int?
    let domainPayload: PalmDomainPayload?

    enum CodingKeys: String, CodingKey {
        case isUsable = "is_usable"
        case totalDurationUs = "total_duration_us"
        case domainPayload = "domain_payload"
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
            // Decode failure detail without echoing palm analysis JSON into logs.
            print("[PalmAnalysisResponse] JSON decode failed: \(error)")
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

    /// Standard scientific anatomical designation.
    public var anatomicalName: String {
        switch self {
        case .heart: return "distal_transverse_crease"
        case .head: return "proximal_transverse_crease"
        case .life: return "thenar_longitudinal_crease"
        case .fate: return "median_longitudinal_crease"
        }
    }

    /// Scientific display name for clinical and research views.
    public var scientificDisplayName: String {
        switch self {
        case .heart: return "Distal Transverse Crease"
        case .head: return "Proximal Transverse Crease"
        case .life: return "Thenar Longitudinal Crease"
        case .fate: return "Median Longitudinal Crease"
        }
    }

    public static func fromAnatomicalName(_ name: String) -> CreaseLineType? {
        switch name.lowercased() {
        case "distal_transverse_crease", "distal_transverse", "dtc": return .heart
        case "proximal_transverse_crease", "proximal_transverse", "ptc": return .head
        case "thenar_longitudinal_crease", "thenar_longitudinal", "tlc": return .life
        case "median_longitudinal_crease", "median_longitudinal", "mlc": return .fate
        default: return nil
        }
    }
}

/// Source of the overlay currently shown on the result screen.
public enum CreaseOverlaySource: String {
    case landmarkFallback
    case nativeEngine
}

// MARK: - Dermatoglyphic Classifier

/**
 * On-device Swift computer vision classifier for dermatoglyphic patterns.
 * Analyzes epidermal ridge orientation flow, singular points (cores/deltas),
 * and spatial ridge frequency from the finger pad ROI.
 *
 * Privacy-preserving: extracts pattern category (Arch, Loop, Whorl),
 * ridge frequency, and quality score only; never stores biometric minutiae templates.
 */
public enum DermatoglyphicClassifier {

    public static func classify(
        pixelBuffer: CVPixelBuffer,
        landmarks: [CGPoint],
        digit: FingertipDigit
    ) -> FingertipScanResult {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        guard width >= 64, height >= 64,
              let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return fallbackResult(digit: digit)
        }

        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let pixelFormat = CVPixelBufferGetPixelFormatType(pixelBuffer)

        // 1. Determine finger pad center in image coordinates
        let padCenter: CGPoint
        if landmarks.count > digit.tipIndex {
            let tip = landmarks[digit.tipIndex]
            let dipIndex: Int = {
                switch digit {
                case .thumb: return 3
                case .index: return 7
                case .middle: return 11
                case .ring: return 15
                case .pinky: return 19
                }
            }()
            if landmarks.count > dipIndex {
                let dip = landmarks[dipIndex]
                // The fleshy pad is located between tip and DIP, ~35% towards the DIP joint
                padCenter = CGPoint(
                    x: tip.x * 0.65 + dip.x * 0.35,
                    y: tip.y * 0.65 + dip.y * 0.35
                )
            } else {
                padCenter = CGPoint(x: tip.x, y: min(1.0, tip.y + 0.04))
            }
        } else {
            // Default center region when macro close-up obscures whole hand
            padCenter = CGPoint(x: 0.5, y: 0.48)
        }

        let centerX = Int(padCenter.x * CGFloat(width))
        let centerY = Int(padCenter.y * CGFloat(height))

        // Crop ROI of size ~192x192 pixels around the pad
        let roiRadius = min(96, min(width, height) / 4)
        let minX = max(1, centerX - roiRadius)
        let maxX = min(width - 2, centerX + roiRadius)
        let minY = max(1, centerY - roiRadius)
        let maxY = min(height - 2, centerY + roiRadius)

        let roiW = maxX - minX
        let roiH = maxY - minY
        guard roiW >= 64, roiH >= 64 else {
            return fallbackResult(digit: digit)
        }

        // 2. Extract grayscale luminance ROI
        var gray = [Float](repeating: 0, count: roiW * roiH)
        let rawPtr = baseAddress.assumingMemoryBound(to: UInt8.self)

        for y in 0..<roiH {
            let srcY = minY + y
            let rowStart = srcY * bytesPerRow
            for x in 0..<roiW {
                let srcX = minX + x
                let lum: Float
                if pixelFormat == kCVPixelFormatType_32BGRA {
                    let offset = rowStart + srcX * 4
                    let b = Float(rawPtr[offset])
                    let g = Float(rawPtr[offset + 1])
                    let r = Float(rawPtr[offset + 2])
                    lum = 0.299 * r + 0.587 * g + 0.114 * b
                } else {
                    // Y / Gray / 420YpCbCr
                    lum = Float(rawPtr[rowStart + srcX])
                }
                gray[y * roiW + x] = lum
            }
        }

        // Contrast stretch / local normalization
        var minLum: Float = 255
        var maxLum: Float = 0
        for val in gray {
            if val < minLum { minLum = val }
            if val > maxLum { maxLum = val }
        }
        let range = max(1.0, maxLum - minLum)
        for i in 0..<gray.count {
            gray[i] = (gray[i] - minLum) / range
        }

        // 3. Compute horizontal and vertical gradients using Sobel operators
        var gradX = [Float](repeating: 0, count: roiW * roiH)
        var gradY = [Float](repeating: 0, count: roiW * roiH)

        for y in 1..<(roiH - 1) {
            for x in 1..<(roiW - 1) {
                let p00 = gray[(y - 1) * roiW + (x - 1)]
                let p02 = gray[(y - 1) * roiW + (x + 1)]
                let p10 = gray[y * roiW + (x - 1)]
                let p12 = gray[y * roiW + (x + 1)]
                let p20 = gray[(y + 1) * roiW + (x - 1)]
                let p22 = gray[(y + 1) * roiW + (x + 1)]

                let p01 = gray[(y - 1) * roiW + x]
                let p21 = gray[(y + 1) * roiW + x]

                let gx = (p02 + 2 * p12 + p22) - (p00 + 2 * p10 + p20)
                let gy = (p20 + 2 * p21 + p22) - (p00 + 2 * p01 + p02)
                gradX[y * roiW + x] = gx
                gradY[y * roiW + x] = gy
            }
        }

        // 4. Block-based orientation field and coherence tensor (16x16 blocks)
        let blockSize = 16
        let gridW = roiW / blockSize
        let gridH = roiH / blockSize
        guard gridW >= 3, gridH >= 3 else {
            return fallbackResult(digit: digit)
        }

        var blockTheta = [Float](repeating: 0, count: gridW * gridH)
        var blockCoherence = [Float](repeating: 0, count: gridW * gridH)
        var totalCoherence: Float = 0

        for by in 0..<gridH {
            for bx in 0..<gridW {
                var sumGxx: Float = 0
                var sumGyy: Float = 0
                var sumGxy: Float = 0

                for py in (by * blockSize)..<((by + 1) * blockSize) {
                    for px in (bx * blockSize)..<((bx + 1) * blockSize) {
                        let gx = gradX[py * roiW + px]
                        let gy = gradY[py * roiW + px]
                        sumGxx += gx * gx
                        sumGyy += gy * gy
                        sumGxy += gx * gy
                    }
                }

                let vX = 2 * sumGxy
                let vY = sumGxx - sumGyy
                let mag = sqrt(vX * vX + vY * vY)
                let denom = sumGxx + sumGyy + 1e-4
                let coherence = mag / denom

                // Ridge flow orientation: perpendicular to gradient
                let theta = 0.5 * atan2(vX, vY) + Float.pi * 0.5
                let idx = by * gridW + bx
                blockTheta[idx] = theta
                blockCoherence[idx] = coherence
                totalCoherence += coherence
            }
        }

        let meanCoherence = totalCoherence / Float(gridW * gridH)
        let quality = min(0.96, max(0.35, meanCoherence * 1.5 + (range / 255.0) * 0.4))

        // 5. Dermatoglyphic pattern classification from orientation field
        let centerBx = Float(gridW) * 0.5
        let centerBy = Float(gridH) * 0.5

        var circularFlowSum: Float = 0
        var circularBlockCount: Int = 0
        var loopCurvatureCount: Int = 0
        var horizontalCount: Int = 0
        var evaluatedBlocks: Int = 0

        for by in 0..<gridH {
            for bx in 0..<gridW {
                let idx = by * gridW + bx
                guard blockCoherence[idx] > 0.15 else { continue }
                evaluatedBlocks += 1

                let dx = Float(bx) - centerBx
                let dy = Float(by) - centerBy
                let dist = sqrt(dx * dx + dy * dy)
                if dist >= 0.5 && dist <= Float(gridW) * 0.48 {
                    let tangentAngle = atan2(dy, dx) + Float.pi / 2.0
                    let theta = blockTheta[idx]
                    let diff = abs(sin(theta - tangentAngle))
                    circularFlowSum += (1.0 - diff)
                    circularBlockCount += 1
                }

                let angle = blockTheta[idx]
                // Flat horizontal ridges (Arch characteristic):
                if abs(sin(angle)) < 0.35 {
                    horizontalCount += 1
                }
                // High vertical curve / loop turnaround:
                if abs(cos(angle)) < 0.40 {
                    loopCurvatureCount += 1
                }
            }
        }

        let validBlocks = max(1, evaluatedBlocks)
        let circularBlocks = max(1, circularBlockCount)
        let circularScore = circularFlowSum / Float(circularBlocks)
        let horizontalRatio = Float(horizontalCount) / Float(validBlocks)
        let loopRatio = Float(loopCurvatureCount) / Float(validBlocks)

        let pattern: DermatoglyphicPattern
        if circularScore > 0.38 {
            pattern = .whorl
        } else if loopRatio > 0.22 || (circularScore > 0.22 && horizontalRatio < 0.65) {
            pattern = .loop
        } else if horizontalRatio > 0.50 {
            pattern = .arch
        } else {
            pattern = .loop
        }

        // 6. Ridge spatial frequency estimation (bandpass zero-crossing spacing)
        var ridgeSpacings: [Float] = []
        let midY = roiH / 2
        for offset in [-12, 0, 12] {
            let row = midY + offset
            guard row >= 0, row < roiH else { continue }
            var lastPeak = -1
            for x in 1..<(roiW - 1) {
                let prev = gray[row * roiW + (x - 1)]
                let curr = gray[row * roiW + x]
                let next = gray[row * roiW + (x + 1)]
                if curr > prev && curr > next && curr > 0.5 {
                    if lastPeak >= 0 {
                        let spacing = Float(x - lastPeak)
                        if spacing >= 4 && spacing <= 25 {
                            ridgeSpacings.append(spacing)
                        }
                    }
                    lastPeak = x
                }
            }
        }

        let meanSpacingPx: Float = ridgeSpacings.isEmpty
            ? 10.0
            : (ridgeSpacings.reduce(0, +) / Float(ridgeSpacings.count))
        let ridgeFrequency = max(0.4, min(1.4, 8.5 / meanSpacingPx))

        return FingertipScanResult(
            digit: digit,
            pattern: pattern,
            ridgeFrequency: (ridgeFrequency * 100).rounded() / 100,
            quality: (quality * 100).rounded() / 100,
            timestamp: Date().timeIntervalSince1970
        )
    }

    private static func fallbackResult(digit: FingertipDigit) -> FingertipScanResult {
        FingertipScanResult(
            digit: digit,
            pattern: .loop,
            ridgeFrequency: 0.85,
            quality: 0.5,
            timestamp: Date().timeIntervalSince1970
        )
    }
}
