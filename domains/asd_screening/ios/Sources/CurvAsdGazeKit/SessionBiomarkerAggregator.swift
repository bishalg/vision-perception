import Foundation

/// Clinical roles for stimulus AOI names.
///
/// Preference compares person-directed regions with geometric or object regions.
/// Biological-motion controls (`Animal_Puppies`) and joint-attention targets
/// (`Gaze_Target_*`, `Center_Fixation_Target`) stay out of the preference ratio:
/// they are scored by their own per-clip metrics, and folding them into GeoPref
/// would credit a look away from a face as social preference.
public enum AOIRoleKind: String, Sendable {
    case social
    case geometric
    case eye
    case mouth
    case orienting
}

public enum AOIRole {
    public static let social: Set<String> = [
        "Social_Actor",
        "Face_Center",
        "Face_Bounding_Box",
        "Eye_Region",
        "Actor_Face",
        "Actor_Eyes",
        "Cartoon_Character",
        "Teacher_Face",
        "Teacher_Mouth"
    ]

    public static let geometric: Set<String> = [
        "Geometric_Pattern",
        "Mechanical_Motion",
        "Gift_Box"
    ]

    public static let eye: Set<String> = [
        "Eye_Region",
        "Actor_Eyes"
    ]

    public static let mouth: Set<String> = [
        "Mouth_Region",
        "Teacher_Mouth"
    ]

    /// Person or eye regions used for first-orient latency.
    /// Matches the reported marker: time until the first fixation lands on the social actor.
    public static let orienting: Set<String> = [
        "Social_Actor",
        "Face_Center",
        "Face_Bounding_Box",
        "Eye_Region",
        "Actor_Face",
        "Actor_Eyes",
        "Teacher_Face"
    ]

    public static func intersects(_ names: [String], role: Set<String>) -> Bool {
        for name in names where role.contains(name) {
            return true
        }
        return false
    }

    /// Built-in names plus any area the stimulus catalog tags with this role.
    /// A new clip opts into scoring by setting roles in the catalog.
    public static func matches(_ name: String, kind: AOIRoleKind) -> Bool {
        if set(for: kind).contains(name) {
            return true
        }
        return StimulusCatalog.current.hasRole(name, kind.rawValue)
    }

    public static func intersects(_ names: [String], kind: AOIRoleKind) -> Bool {
        names.contains { matches($0, kind: kind) }
    }

    private static func set(for kind: AOIRoleKind) -> Set<String> {
        switch kind {
        case .social: return social
        case .geometric: return geometric
        case .eye: return eye
        case .mouth: return mouth
        case .orienting: return orienting
        }
    }
}

/// Session-level biomarker reduction shared by the live controller and the processing actor.
///
/// One frame counts at most once per role, so nested boxes (face + eyes) do not
/// double-count dwell. Eye and mouth ratios are `nil` (unmeasured, PLC-2) when the
/// session never hit a face-feature box, so a GeoPref-only clip is not scored as if
/// the child avoided the eyes. First-orient latency is the session time of the first
/// person/eye hit, and `nil` (PLC-1) when the child never oriented to a person; it is
/// never filled with a default or the session length. Smile reactivity is the session peak minus
/// the mean smile in the first 500 ms. Per-clip scoring still applies that same
/// peak-minus-baseline formula inside each clip.
///
/// The reduction lives in the Rust core (`asd-core::aggregate`, CORE-6); this type keeps the
/// public Swift API (FFI-3). The core uses the built-in role sets, which cover every role
/// the bundled catalog assigns (guarded by an asd-core test and `LegacyParityTests`).
public enum SessionBiomarkerAggregator {
    public struct Aggregates: Equatable, Sendable, Decodable {
        public let socialDwellSec: Double
        public let geometricDwellSec: Double
        public let socialPreferenceScore: Double
        public let eyeDwellRatio: Double?
        public let mouthDwellRatio: Double?
        public let firstOrientLatencyMs: Double?
        public let socialSmileIntensityMean: Double
        public let smileReactivityIndex: Double
        public let flatAffectScore: Double
        public let surpriseScoreMean: Double
    }

    public static let frameIntervalSec: Double = 1.0 / 60.0

    public static func aggregate(validFrames: [GazeFrame], durationSec: Double) -> Aggregates {
        aggregate(coreFrames: validFrames.map { CoreFrame($0) }, durationSec: durationSec)
    }

    static func aggregate(coreFrames: [CoreFrame], durationSec: Double) -> Aggregates {
        do {
            return try AsdCoreBridge.aggregateSession(coreFrames, durationSec: durationSec)
        } catch {
            AsdCoreBridge.report("aggregate_session", error)
            // Nothing measured: unmeasured ratios and latency stay nil (PLC-1/2).
            return Aggregates(
                socialDwellSec: 0,
                geometricDwellSec: 0,
                socialPreferenceScore: 0,
                eyeDwellRatio: nil,
                mouthDwellRatio: nil,
                firstOrientLatencyMs: nil,
                socialSmileIntensityMean: 0,
                smileReactivityIndex: 0,
                flatAffectScore: 1,
                surpriseScoreMean: 0
            )
        }
    }
}
