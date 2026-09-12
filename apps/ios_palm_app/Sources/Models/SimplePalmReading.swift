import Foundation
import CoreGraphics
import CurvPalmOverlay

/**
 * Basic local Samudrika palm reading — deterministic, rule-based, no network / no VLM.
 * Turns CreaseSpline metrics (confidence, tortuosity, arcLengthPx, points count) into
 * readable interpretive sentences. Purely descriptive of geometry; not a prediction claim.
 */
public struct SimplePalmReading {

    // MARK: - Public API (spec)

    /// Per-line readings for all splines, ordered as passed in.
    public static func reading(for splines: [CreaseSpline], handSide: HandSide) -> [String] {
        splines.map { readingText(for: $0, handSide: handSide) }
    }

    /// Overload without handSide (defaults to .right / active hand).
    public static func reading(for splines: [CreaseSpline]) -> [String] {
        reading(for: splines, handSide: .right)
    }

    /// Single-line interpretive sentence + compact metric suffix.
    public static func readingText(for spline: CreaseSpline, handSide: HandSide) -> String {
        let handNote: String = {
            switch handSide {
            case .left: return "Left (inherited inclination)"
            case .right: return "Right (active expression)"
            case .auto: return "Active hand"
            }
        }()
        let base = baseReading(for: spline)
        // Append a short hand-side cue only for heart/life where tradition distinguishes.
        let sideCue: String
        switch spline.lineType {
        case .heart, .life:
            sideCue = " — \(handNote)"
        default:
            sideCue = ""
        }
        return base + sideCue
    }

    /// Single-line without side.
    public static func readingText(for spline: CreaseSpline) -> String {
        baseReading(for: spline)
    }

    // MARK: - Private templates

    private static func baseReading(for s: CreaseSpline) -> String {
        let conf = s.confidence
        let tort = s.tortuosity
        let len = s.arcLengthPx
        let pts = s.points.count

        // Confidence tier
        let clarity: String = {
            if conf < 0.35 { return "faint / lightly marked" }
            if conf < 0.55 { return "lightly marked" }
            if conf < 0.75 { return "moderately clear" }
            if conf < 0.88 { return "clear" }
            return "very clear, strongly marked"
        }()

        // Length tier (px, ~100 px ≈ medium; heuristic from fallback *1400)
        let lengthNote: String = {
            if len < 85 { return "short" }
            if len < 130 { return "medium-short" }
            if len < 180 { return "medium-long" }
            return "long"
        }()

        // Points density
        let detailNote = pts < 4 ? "few anchor points" : pts <= 6 ? "well-traced" : "finely traced (\(pts) pts)"

        // Tortuosity tier
        let curveNote: String = {
            if tort > 1.30 { return "strongly curved / winding" }
            if tort > 1.22 { return "curved" }
            if tort > 1.10 { return "gently curved" }
            return "straighter"
        }()

        switch s.lineType {
        case .heart:
            let temper: String = {
                if tort > 1.25 { return "expressive, emotionally open" }
                if tort > 1.12 { return "balanced warmth and reserve" }
                return "measured, steadier in affections"
            }()
            let lenHint = len < 100 ? "selective attachments" : len > 175 ? "deep continuity in bonds" : "steady warmth"
            return "Heart Line — \(curveNote), \(temper); \(lengthNote) (\(lenHint)), \(clarity)."
        case .head:
            let focus: String = {
                if tort > 1.28 { return "flexible, imaginative thinking" }
                if tort > 1.14 { return "balanced logic and imagination" }
                return "focused, linear reasoning"
            }()
            let lenHint = len < 95 ? "concise, to-the-point focus" : len > 170 ? "extended analytical reach" : "even, sustained focus"
            return "Head Line — \(curveNote), \(focus); \(lengthNote) (\(lenHint)), \(clarity)."
        case .life:
            let vigor: String = {
                if tort > 1.32 { return "adaptive, draws around obstacles" }
                if tort > 1.18 { return "gentle arc — resilient" }
                return "steady stamina and vigor"
            }()
            let lenHint = len < 110 ? "shorter arc — conserve energy, pace well" : len > 200 ? "long arc — strong vitality, wide circle" : "medium arc — even vitality"
            let confHint = conf < 0.5 ? "lighter — nurture routines" : "well-marked"
            return "Life Line — \(vigor); \(lengthNote) (\(lenHint)), \(clarity) (\(confHint))."
        case .fate:
            let presence: String = {
                if conf < 0.38 { return "faint / intermittent — path shaped more by choice than circumstance" }
                if conf < 0.65 { return "moderate — evolving direction, builds with effort" }
                if len < 90 { return "short but clear — focused phase, goals narrow then widen" }
                return "clear and continuous — steady direction, purpose builds"
            }()
            let curveHint = tort > 1.18 ? "(\(curveNote) — flexible course corrections)" : "(\(curveNote))"
            return "Fate Line — \(presence) \(curveHint), \(lengthNote), \(clarity)."
        }
    }
}
