import Foundation

/**
 * Domain-agnostic hand anthropometry and biometric models.
 *
 * Provides physical linear dimensions (mm), standard biomarker digit ratios
 * (including 2D:4D), palmar shape proportions, and anatomical pad morphometry
 * for scientific, clinical, and ergonomic research.
 */

public struct DigitMeasurement: Codable, Sendable, Hashable, Identifiable {
    public var id: String { name }
    public let name: String
    public let totalLengthMm: Float
    public let proximalPhalanxMm: Float
    public let intermediatePhalanxMm: Float
    public let distalPhalanxMm: Float
    public let lengthRelativeToHand: Float

    public init(
        name: String,
        totalLengthMm: Float,
        proximalPhalanxMm: Float,
        intermediatePhalanxMm: Float,
        distalPhalanxMm: Float,
        lengthRelativeToHand: Float
    ) {
        self.name = name
        self.totalLengthMm = totalLengthMm
        self.proximalPhalanxMm = proximalPhalanxMm
        self.intermediatePhalanxMm = intermediatePhalanxMm
        self.distalPhalanxMm = distalPhalanxMm
        self.lengthRelativeToHand = lengthRelativeToHand
    }
}

public struct DigitRatios: Codable, Sendable, Hashable {
    /// 2D:4D Digit Ratio (Index to Ring finger length) - widely studied biomarker in developmental biology and genetics
    public let ratio2D4D: Float
    public let ratio2D3D: Float
    public let ratio3D4D: Float
    public let ratio4D5D: Float
    public let thumbToIndex: Float

    public init(
        ratio2D4D: Float,
        ratio2D3D: Float,
        ratio3D4D: Float,
        ratio4D5D: Float,
        thumbToIndex: Float
    ) {
        self.ratio2D4D = ratio2D4D
        self.ratio2D3D = ratio2D3D
        self.ratio3D4D = ratio3D4D
        self.ratio4D5D = ratio4D5D
        self.thumbToIndex = thumbToIndex
    }
}

public struct PalmDimensions: Codable, Sendable, Hashable {
    public let metacarpalBreadthMm: Float
    public let palmLengthMm: Float
    public let totalHandLengthMm: Float
    public let aspectRatio: Float
    public let palmSurfaceAreaCm2: Float
    public let handShapeType: String

    public init(
        metacarpalBreadthMm: Float,
        palmLengthMm: Float,
        totalHandLengthMm: Float,
        aspectRatio: Float,
        palmSurfaceAreaCm2: Float,
        handShapeType: String
    ) {
        self.metacarpalBreadthMm = metacarpalBreadthMm
        self.palmLengthMm = palmLengthMm
        self.totalHandLengthMm = totalHandLengthMm
        self.aspectRatio = aspectRatio
        self.palmSurfaceAreaCm2 = palmSurfaceAreaCm2
        self.handShapeType = handShapeType
    }
}

public struct InterdigitalSpan: Codable, Sendable, Hashable {
    public let thumbIndexAngleDeg: Float
    public let indexMiddleAngleDeg: Float
    public let middleRingAngleDeg: Float
    public let ringPinkyAngleDeg: Float
    public let thumbIndexDistanceMm: Float
    public let indexMiddleDistanceMm: Float
    public let middleRingDistanceMm: Float
    public let ringPinkyDistanceMm: Float

    public init(
        thumbIndexAngleDeg: Float,
        indexMiddleAngleDeg: Float,
        middleRingAngleDeg: Float,
        ringPinkyAngleDeg: Float,
        thumbIndexDistanceMm: Float,
        indexMiddleDistanceMm: Float,
        middleRingDistanceMm: Float,
        ringPinkyDistanceMm: Float
    ) {
        self.thumbIndexAngleDeg = thumbIndexAngleDeg
        self.indexMiddleAngleDeg = indexMiddleAngleDeg
        self.middleRingAngleDeg = middleRingAngleDeg
        self.ringPinkyAngleDeg = ringPinkyAngleDeg
        self.thumbIndexDistanceMm = thumbIndexDistanceMm
        self.indexMiddleDistanceMm = indexMiddleDistanceMm
        self.middleRingDistanceMm = middleRingDistanceMm
        self.ringPinkyDistanceMm = ringPinkyDistanceMm
    }
}

public struct AnatomicalPadMetric: Codable, Sendable, Hashable, Identifiable {
    public var id: String { padName }
    public let padName: String
    public let heightMm: Float
    public let relativeProminence: Float
    public let areaMm2: Float
    public let curvature: Float

    public var anatomicalPad: AnatomicalPad? {
        AnatomicalPad(rawValue: padName) ?? AnatomicalPad.fromAstrologicalMount(padName)
    }

    public init(
        padName: String,
        heightMm: Float,
        relativeProminence: Float,
        areaMm2: Float,
        curvature: Float
    ) {
        self.padName = padName
        self.heightMm = heightMm
        self.relativeProminence = relativeProminence
        self.areaMm2 = areaMm2
        self.curvature = curvature
    }
}

public struct PalmarCreaseMetric: Codable, Sendable, Hashable, Identifiable {
    public var id: String { anatomicalName }
    public let anatomicalName: String
    public let traditionalAlias: String
    public let arcLengthMm: Float
    public let chordLengthMm: Float
    public let tortuosity: Float
    public let isSimianVariant: Bool

    public var scientificDisplayName: String {
        switch anatomicalName {
        case "distal_transverse_crease": return "Distal Transverse Crease"
        case "proximal_transverse_crease": return "Proximal Transverse Crease"
        case "thenar_longitudinal_crease": return "Thenar Longitudinal Crease"
        case "median_longitudinal_crease": return "Median Longitudinal Crease"
        default: return anatomicalName
        }
    }

    public init(
        anatomicalName: String,
        traditionalAlias: String,
        arcLengthMm: Float,
        chordLengthMm: Float,
        tortuosity: Float,
        isSimianVariant: Bool = false
    ) {
        self.anatomicalName = anatomicalName
        self.traditionalAlias = traditionalAlias
        self.arcLengthMm = arcLengthMm
        self.chordLengthMm = chordLengthMm
        self.tortuosity = tortuosity
        self.isSimianVariant = isSimianVariant
    }
}

public struct HandBiometricsReport: Codable, Sendable, Hashable {
    public let schemaVersion: String
    public let palm: PalmDimensions
    public let digits: [DigitMeasurement]
    public let ratios: DigitRatios
    public let spans: InterdigitalSpan
    public let pads: [AnatomicalPadMetric]
    public let creases: [PalmarCreaseMetric]
    public let mmPerPixel: Float
    public let handSide: String

    public init(
        schemaVersion: String = "curv_hand_anthropometry_v1",
        palm: PalmDimensions,
        digits: [DigitMeasurement],
        ratios: DigitRatios,
        spans: InterdigitalSpan,
        pads: [AnatomicalPadMetric],
        creases: [PalmarCreaseMetric] = [],
        mmPerPixel: Float,
        handSide: String
    ) {
        self.schemaVersion = schemaVersion
        self.palm = palm
        self.digits = digits
        self.ratios = ratios
        self.spans = spans
        self.pads = pads
        self.creases = creases
        self.mmPerPixel = mmPerPixel
        self.handSide = handSide
    }
}

public enum HandBiometricsCalculator {
    private static func dist(_ p1: [Float], _ p2: [Float]) -> Float {
        let dx = p1[0] - p2[0]
        let dy = p1[1] - p2[1]
        return (dx * dx + dy * dy).squareRoot()
    }

    private static func angleBetween(v1: (Float, Float), v2: (Float, Float)) -> Float {
        let mag1 = (v1.0 * v1.0 + v1.1 * v1.1).squareRoot()
        let mag2 = (v2.0 * v2.0 + v2.1 * v2.1).squareRoot()
        guard mag1 > 1e-6, mag2 > 1e-6 else { return 0.0 }
        let dot = v1.0 * v2.0 + v1.1 * v2.1
        let cosTheta = max(-1.0, min(1.0, dot / (mag1 * mag2)))
        return acos(cosTheta) * 180.0 / .pi
    }

    public static func compute(
        landmarks: [[Float]],
        imageWidth: Float = 1000.0,
        imageHeight: Float = 1000.0,
        referencePalmWidthMm: Float = 80.0,
        padDepthMm: [String: Float] = [:],
        handSide: String = "auto"
    ) -> HandBiometricsReport? {
        guard landmarks.count >= 21 else { return nil }

        // Scale factor: Landmark 5 (Index MCP) to 17 (Pinky MCP)
        let p5 = [landmarks[5][0] * imageWidth, landmarks[5][1] * imageHeight]
        let p17 = [landmarks[17][0] * imageWidth, landmarks[17][1] * imageHeight]
        let palmBreadthPx = dist(p5, p17)
        guard palmBreadthPx > 1.0 else { return nil }

        let mmPerPx = referencePalmWidthMm / palmBreadthPx
        let pxToMm = { (px: Float) in px * mmPerPx }

        // Palm length: wrist (0) to middle MCP (9)
        let p0 = [landmarks[0][0] * imageWidth, landmarks[0][1] * imageHeight]
        let p9 = [landmarks[9][0] * imageWidth, landmarks[9][1] * imageHeight]
        let palmLengthMm = pxToMm(dist(p0, p9))

        // Total hand length: wrist (0) to middle tip (12)
        let p12 = [landmarks[12][0] * imageWidth, landmarks[12][1] * imageHeight]
        let totalHandLengthMm = pxToMm(dist(p0, p12))

        let metacarpalBreadthMm = referencePalmWidthMm
        let aspectRatio = palmLengthMm > 1e-3 ? (metacarpalBreadthMm / palmLengthMm) : 1.0
        let areaCm2 = (metacarpalBreadthMm * palmLengthMm * 0.85) / 100.0
        let shapeType = aspectRatio > 0.9 ? "broad" : (aspectRatio < 0.75 ? "elongated" : "proportional")

        let palm = PalmDimensions(
            metacarpalBreadthMm: metacarpalBreadthMm,
            palmLengthMm: palmLengthMm,
            totalHandLengthMm: totalHandLengthMm,
            aspectRatio: aspectRatio,
            palmSurfaceAreaCm2: areaCm2,
            handShapeType: shapeType
        )

        // Digit chains
        struct Chain {
            let name: String
            let indices: [Int]
        }
        let chains = [
            Chain(name: "thumb", indices: [1, 2, 3, 4]),
            Chain(name: "index", indices: [5, 6, 7, 8]),
            Chain(name: "middle", indices: [9, 10, 11, 12]),
            Chain(name: "ring", indices: [13, 14, 15, 16]),
            Chain(name: "pinky", indices: [17, 18, 19, 20])
        ]

        var digits: [DigitMeasurement] = []
        var digitLengths: [String: Float] = [:]

        for c in chains {
            let pts = c.indices.map { [landmarks[$0][0] * imageWidth, landmarks[$0][1] * imageHeight] }
            let p1 = pxToMm(dist(pts[0], pts[1]))
            let p2 = pxToMm(dist(pts[1], pts[2]))
            let p3 = pxToMm(dist(pts[2], pts[3]))
            let total = p1 + p2 + p3
            digitLengths[c.name] = total
            let rel = totalHandLengthMm > 1e-3 ? (total / totalHandLengthMm) : 0.0

            digits.append(DigitMeasurement(
                name: c.name,
                totalLengthMm: total,
                proximalPhalanxMm: p1,
                intermediatePhalanxMm: c.name == "thumb" ? 0.0 : p2,
                distalPhalanxMm: p3,
                lengthRelativeToHand: rel
            ))
        }

        // Digit ratios
        let indexLen = digitLengths["index"] ?? 70.0
        let middleLen = digitLengths["middle"] ?? 78.0
        let ringLen = digitLengths["ring"] ?? 72.0
        let pinkyLen = digitLengths["pinky"] ?? 55.0
        let thumbLen = digitLengths["thumb"] ?? 50.0

        let ratios = DigitRatios(
            ratio2D4D: ringLen > 1e-3 ? (indexLen / ringLen) : 0.97,
            ratio2D3D: middleLen > 1e-3 ? (indexLen / middleLen) : 0.90,
            ratio3D4D: ringLen > 1e-3 ? (middleLen / ringLen) : 1.08,
            ratio4D5D: pinkyLen > 1e-3 ? (ringLen / pinkyLen) : 1.30,
            thumbToIndex: indexLen > 1e-3 ? (thumbLen / indexLen) : 0.70
        )

        // Spans & Angles
        let tip4 = [landmarks[4][0] * imageWidth, landmarks[4][1] * imageHeight]
        let tip8 = [landmarks[8][0] * imageWidth, landmarks[8][1] * imageHeight]
        let tip16 = [landmarks[16][0] * imageWidth, landmarks[16][1] * imageHeight]
        let tip20 = [landmarks[20][0] * imageWidth, landmarks[20][1] * imageHeight]

        let vThumb = (tip4[0] - p0[0], tip4[1] - p0[1])
        let vIndex = (tip8[0] - p0[0], tip8[1] - p0[1])
        let vMiddle = (p12[0] - p0[0], p12[1] - p0[1])
        let vRing = (tip16[0] - p0[0], tip16[1] - p0[1])
        let vPinky = (tip20[0] - p0[0], tip20[1] - p0[1])

        let spans = InterdigitalSpan(
            thumbIndexAngleDeg: angleBetween(v1: vThumb, v2: vIndex),
            indexMiddleAngleDeg: angleBetween(v1: vIndex, v2: vMiddle),
            middleRingAngleDeg: angleBetween(v1: vMiddle, v2: vRing),
            ringPinkyAngleDeg: angleBetween(v1: vRing, v2: vPinky),
            thumbIndexDistanceMm: pxToMm(dist(tip4, tip8)),
            indexMiddleDistanceMm: pxToMm(dist(tip8, p12)),
            middleRingDistanceMm: pxToMm(dist(p12, tip16)),
            ringPinkyDistanceMm: pxToMm(dist(tip16, tip20))
        )

        // Anatomical Pads
        var pads: [AnatomicalPadMetric] = []
        for pad in AnatomicalPad.allCases {
            let key = pad.rawValue
            let astroKey = pad.astrologicalMountName
            let hMm = padDepthMm[key] ?? padDepthMm[astroKey] ?? 0.0
            let prominence = max(-1.0, min(1.0, hMm / 4.0))
            pads.append(AnatomicalPadMetric(
                padName: key,
                heightMm: hMm,
                relativeProminence: prominence,
                areaMm2: 150.0,
                curvature: prominence * 0.15
            ))
        }

        // Palmar flexion creases (standard anatomical nomenclature)
        let p6 = [landmarks[6][0] * imageWidth, landmarks[6][1] * imageHeight]
        let p18 = [landmarks[18][0] * imageWidth, landmarks[18][1] * imageHeight]
        let p2 = [landmarks[2][0] * imageWidth, landmarks[2][1] * imageHeight]

        let dtcChord = pxToMm(dist(p17, p6) * 0.90)
        let ptcChord = pxToMm(dist(p5, p18) * 0.85)
        let tlcChord = pxToMm(dist(p2, p0) * 0.95)
        let mlcChord = pxToMm(dist(p0, p9) * 0.75)

        let creases = [
            PalmarCreaseMetric(
                anatomicalName: "distal_transverse_crease",
                traditionalAlias: "heart",
                arcLengthMm: dtcChord * 1.05,
                chordLengthMm: dtcChord,
                tortuosity: 1.05
            ),
            PalmarCreaseMetric(
                anatomicalName: "proximal_transverse_crease",
                traditionalAlias: "head",
                arcLengthMm: ptcChord * 1.04,
                chordLengthMm: ptcChord,
                tortuosity: 1.04
            ),
            PalmarCreaseMetric(
                anatomicalName: "thenar_longitudinal_crease",
                traditionalAlias: "life",
                arcLengthMm: tlcChord * 1.15,
                chordLengthMm: tlcChord,
                tortuosity: 1.15
            ),
            PalmarCreaseMetric(
                anatomicalName: "median_longitudinal_crease",
                traditionalAlias: "fate",
                arcLengthMm: mlcChord * 1.02,
                chordLengthMm: mlcChord,
                tortuosity: 1.02
            )
        ]

        return HandBiometricsReport(
            palm: palm,
            digits: digits,
            ratios: ratios,
            spans: spans,
            pads: pads,
            creases: creases,
            mmPerPixel: mmPerPx,
            handSide: handSide
        )
    }
}
