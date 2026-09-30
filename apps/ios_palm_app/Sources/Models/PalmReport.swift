import Foundation
import CurvPalmOverlay

/**
 * Conversion of the 3D scan analysis into one cohesive astrological report.
 *
 * `PalmReportGenerator.generate` takes everything the 3D mapping produced —
 * the per-mount relief map (orbit analyzer), the pad-depth fallbacks, and the
 * landmark biometrics — and produces a ranked, confidence-aware `PalmReport`:
 *
 *  - `mountReports`: every mount, ranked strongest-first, each with its
 *    development level and matched Samudrika reading. Unmeasured mounts are
 *    included with an explicit "not measured" state instead of being dropped.
 *  - `dominantMounts`: the strongest reliable mounts that characterize the hand.
 *  - `synthesis`: a composed paragraph grounded in the measured data
 *    (dominant mounts + palm shape + 2D:4D biomarker).
 *  - `dataQualityNote`: what the reader should know about scan coverage.
 *
 * Pure and unit-tested; no UI dependencies.
 */
public struct PalmMountReport: Identifiable, Sendable {
    public let id: String
    public let profile: MountProfile
    public let development: MountDevelopment?
    public let heightMm: Float?
    public let heightRelative: Float?
    public let prominence: MountProminence?
    public let confidence: Float?
    public let viewCount: Int?
    public let isMeasured: Bool
    public let isReliable: Bool

    public var reading: String {
        if let development {
            return profile.reading(for: development)
        }
        return "This mount was not measured clearly in the scan, so no reading is offered for it yet. Rescan with a full orbit capture to include it."
    }

    public var displayName: String { profile.displayName }
    public var planetGlyph: String { profile.planetGlyph }
}

public struct PalmReport: Sendable {
    public let handContext: String
    public let handShapeLabel: String?
    public let handShapeReading: String?
    public let palmBreadthMm: Float?
    public let palmLengthMm: Float?
    public let ratio2D4D: Float?
    public let ratio2D4DReading: String?
    /// All mounts, strongest measured first; unmeasured mounts last.
    public let mountReports: [PalmMountReport]
    public let dominantMounts: [PalmMountReport]
    public let synthesis: String
    public let dataQualityNote: String?
}

public enum PalmReportGenerator {

    /// Minimum estimator confidence for a mount to anchor the synthesis.
    private static let reliableConfidence: Float = 0.6
    /// Minimum orbit views contributing for a mount to anchor the synthesis.
    private static let reliableViewCount = 3

    // MARK: - Entry

    public static func generate(
        mountReliefMap: MountReliefMap?,
        padDepths: [String: Float],
        landmarks: [CGPoint],
        referencePalmWidthMm: Float = 80.0,
        handSide: HandSide
    ) -> PalmReport {
        let biometrics = HandBiometricsCalculator.compute(
            landmarks: landmarks.map { [Float($0.x), Float($0.y)] },
            referencePalmWidthMm: referencePalmWidthMm,
            padDepthMm: padDepths
        )

        let mounts = buildMountReports(mountReliefMap: mountReliefMap, padDepths: padDepths)
        let dominant = dominantMounts(of: mounts)
        let shape = shapeReading(biometrics?.palm.handShapeType)
        let ratioReading = ratio2D4DReading(biometrics?.ratios.ratio2D4D)

        return PalmReport(
            handContext: handSide == .left ? "Left Palm • Inherited Potential" : "Right Palm • Active Manifestation",
            handShapeLabel: biometrics?.palm.handShapeType.capitalized,
            handShapeReading: shape,
            palmBreadthMm: biometrics?.palm.metacarpalBreadthMm,
            palmLengthMm: biometrics?.palm.palmLengthMm,
            ratio2D4D: biometrics?.ratios.ratio2D4D,
            ratio2D4DReading: ratioReading,
            mountReports: mounts,
            dominantMounts: dominant,
            synthesis: synthesis(
                dominant: dominant,
                measuredCount: mounts.filter(\.isMeasured).count
            ),
            dataQualityNote: dataQualityNote(mounts: mounts)
        )
    }

    // MARK: - Mount reports

    private static func buildMountReports(
        mountReliefMap: MountReliefMap?,
        padDepths: [String: Float]
    ) -> [PalmMountReport] {
        let chipSpecs: [(mount: PalmMount, fallbackKeys: [String])] = [
            (.venus, ["venus", "thenar"]),
            (.jupiter, ["jupiter", "radial_interdigital"]),
            (.saturn, ["saturn", "central_interdigital"]),
            (.apollo, ["apollo", "ring_interdigital"]),
            (.mercury, ["mercury", "ulnar_interdigital"]),
            (.luna, ["luna", "hypothenar"]),
            (.mars, ["mars", "palmar_fossa"]),
        ]

        let reports: [PalmMountReport] = chipSpecs.map { spec in
            let entry = mountReliefMap?.entry(for: spec.mount)
            let fallbackMm = spec.fallbackKeys.compactMap { padDepths[$0] }.first
            let mm = entry?.heightMm ?? fallbackMm
            let development = MountAstrology.development(
                heightMm: mm,
                prominence: entry?.prominence
            )
            let confidence = entry?.confidence
            let views = entry?.viewCount
            let reliable = (confidence.map { $0 >= reliableConfidence } ?? false)
                && (views.map { $0 >= reliableViewCount } ?? false)
            return PalmMountReport(
                id: spec.mount.rawValue,
                profile: MountAstrology.profile(for: spec.mount.rawValue)!,
                development: development,
                heightMm: mm,
                heightRelative: entry.map { $0.heightRelative },
                prominence: entry?.prominence,
                confidence: confidence,
                viewCount: views,
                isMeasured: development != nil,
                isReliable: development != nil && reliable
            )
        }

        // Strongest measured first; among equally-ranked, keep anatomical order.
        func rank(_ r: PalmMountReport) -> Float {
            if let mm = r.heightMm { return mm }
            switch r.prominence {
            case .elevated: return 1.5
            case .average: return 0.75
            case .deficient: return 0.0
            case nil: return -10.0
            }
        }
        return reports.enumerated().sorted { a, b in
            let ra = rank(a.element), rb = rank(b.element)
            if abs(ra - rb) > 0.01 { return ra > rb }
            return a.offset < b.offset
        }.map(\.element)
    }

    private static func dominantMounts(of mounts: [PalmMountReport]) -> [PalmMountReport] {
        let reliable = mounts.filter(\.isReliable)
        let pool = reliable.isEmpty ? mounts.filter(\.isMeasured) : reliable
        return Array(pool.prefix(2))
    }

    // MARK: - Composers

    private static func shapeReading(_ shapeType: String?) -> String? {
        switch shapeType {
        case "broad":
            return "A broad, square-leaning palm is classically read as an earth hand — practical, dependable, and comfortable working with tangible things."
        case "elongated":
            return "A long, narrow palm is classically read as a water hand — sensitive, imaginative, and emotionally attuned to the people around you."
        case "proportional":
            return "Balanced palm proportions are read as an even blend of the practical and the imaginative — steadiness with room for ideas."
        default:
            return nil
        }
    }

    private static func ratio2D4DReading(_ ratio: Float?) -> String? {
        guard let ratio, ratio > 0 else { return nil }
        if ratio < 0.95 {
            return String(format: "A longer ring finger (2D:4D %.2f) is traditionally read as a competitive, go-getter streak with strong drive.", ratio)
        } else if ratio <= 1.0 {
            return String(format: "Near-equal index and ring fingers (2D:4D %.2f) are read as a balanced blend of assertiveness and empathy.", ratio)
        }
        return String(format: "A longer index finger (2D:4D %.2f) is traditionally read as confidence in leadership and clear communication.", ratio)
    }

    private static func synthesis(
        dominant: [PalmMountReport],
        measuredCount: Int
    ) -> String {
        guard !dominant.isEmpty else {
            return measuredCount == 0
                ? "No mount relief was captured in this scan, so the reading leans on palm shape and finger proportions alone. Run a full orbit scan for the complete mount analysis."
                : "The measured mounts are close in strength, giving an even-tempered profile rather than one sharply dominant trait."
        }

        var parts: [String] = []
        let names = dominant.map(\.displayName)
        if dominant.count == 1 {
            parts.append("Your palm is anchored by a strong \(names[0]).")
        } else {
            parts.append("Your palm is anchored by two strong regions — the \(names[0]) and the \(names[1]).")
        }
        for mount in dominant {
            // Strip the trailing period so the joined clause stays clean.
            var governs = mount.profile.governs.lowercased()
            if governs.hasSuffix(".") { governs.removeLast() }
            parts.append("\(mount.displayName): \(governs) — \(leadClause(mount)).")
        }
        // Palm shape and 2D:4D are first-class report fields rendered above the
        // synthesis; the paragraph stays focused on the dominant mounts.
        return parts.filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// Short character clause derived from the dominant mount's level.
    private static func leadClause(_ mount: PalmMountReport) -> String {
        switch mount.development {
        case .prominent: return "its prominence signals a defining trait in your temperament"
        case .developed: return "its firmness marks it as one of your leading strengths"
        case .balanced: return "it is steady rather than dominant, lending quiet support to the whole hand"
        case .deficient: return "its relative flatness suggests the energy is kept in reserve"
        case nil: return "it awaits a clearer measurement"
        }
    }

    private static func dataQualityNote(mounts: [PalmMountReport]) -> String? {
        let measured = mounts.filter(\.isMeasured)
        let unmeasured = mounts.count - measured.count
        let lowConfidence = measured.filter { !$0.isReliable }

        var notes: [String] = []
        if unmeasured > 0 {
            notes.append("\(unmeasured) of \(mounts.count) mounts were not clearly measured in this scan.")
        }
        if !lowConfidence.isEmpty {
            let names = lowConfidence.map(\.displayName).joined(separator: ", ")
            notes.append("Lower confidence on \(names) — treat those readings as indicative only.")
        }
        guard !notes.isEmpty else { return nil }
        return notes.joined(separator: " ")
    }
}
