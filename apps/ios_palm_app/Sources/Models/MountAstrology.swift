import Foundation
import CurvPalmOverlay

/// Development level of a palm mount, derived from measured relief.
public enum MountDevelopment: String, CaseIterable, Sendable {
    case deficient
    case balanced
    case developed
    case prominent

    /// Short qualifier shown as a badge next to the measured values.
    public var label: String {
        switch self {
        case .deficient: return "Underdeveloped"
        case .balanced: return "Balanced"
        case .developed: return "Developed"
        case .prominent: return "Prominent"
        }
    }
}

/// Static Samudrika-style profile for one mount: where it sits, what it governs,
/// and the interpretive reading per measured development level.
public struct MountProfile: Sendable {
    public let mount: String
    public let displayName: String
    public let planetGlyph: String
    /// Anatomical location in plain words (shown under the region overlay).
    public let anatomicalLocation: String
    /// What the mount governs (shown as the reading's domain line).
    public let governs: String
    /// Interpretation keyed by development level.
    public let readings: [MountDevelopment: String]

    public func reading(for level: MountDevelopment) -> String {
        readings[level] ?? readings[.balanced] ?? ""
    }
}

public enum MountAstrology {

    // Development thresholds on the measured absolute relief (mm). The mesh
    // builder clamps pad depths to [-2.5, 4.0] mm, so these cover the domain.
    private static let deficientBelowMm: Float = 0.3
    private static let developedFromMm: Float = 1.4
    private static let prominentFromMm: Float = 2.6

    /// Derives the development level from measured data. Prefers the absolute
    /// millimetre relief; falls back to the estimator's relative prominence
    /// classification when no mm value was measured.
    public static func development(
        heightMm: Float?,
        prominence: MountProminence?,
        heightRelative: Float? = nil
    ) -> MountDevelopment? {
        if let mm = heightMm {
            if mm < deficientBelowMm { return .deficient }
            if mm < developedFromMm { return .balanced }
            if mm < prominentFromMm { return .developed }
            return .prominent
        }
        switch prominence {
        case .deficient: return .deficient
        case .average: return .balanced
        case .elevated: return .developed
        case nil: return nil
        }
    }

    public static func profile(for mount: String) -> MountProfile? {
        profiles[mount]
    }

    public static var allProfiles: [MountProfile] {
        profiles.values.sorted { $0.mount < $1.mount }
    }

    // MARK: - Catalog

    static let profiles: [String: MountProfile] = [
        "jupiter": MountProfile(
            mount: "jupiter",
            displayName: "Mount of Jupiter",
            planetGlyph: "♃",
            anatomicalLocation: "Beneath the base of the index finger.",
            governs: "Leadership, ambition, self-confidence, honor, and social standing.",
            readings: [
                .deficient: "A low Jupiter mount suggests modest outer ambition and a preference for supporting roles. Confidence is present but kept private; recognition is pursued quietly rather than claimed openly.",
                .balanced: "A balanced Jupiter mount points to healthy self-assurance and dependable judgment. Responsibility is accepted naturally, and leadership emerges when the situation genuinely calls for it.",
                .developed: "A firm Jupiter mount indicates a natural leadership presence — optimism, honor-seeking, and the drive to organize and guide others. Doors tend to open through personal authority rather than force.",
                .prominent: "A pronounced Jupiter mount signals a commanding presence and strong status orientation. The gift is inspiration; the caution is overreach — taking on more authority than the follow-through can sustain."
            ]
        ),
        "saturn": MountProfile(
            mount: "saturn",
            displayName: "Mount of Saturn",
            planetGlyph: "♄",
            anatomicalLocation: "Beneath the base of the middle finger.",
            governs: "Discipline, patience, duty, wisdom, and endurance.",
            readings: [
                .deficient: "A flat Saturn mount suggests restlessness with routine and a tendency to sidestep heavy obligations. Lessons of patience arrive later in life, usually through experience rather than temperament.",
                .balanced: "A balanced Saturn mount reflects a steady, pragmatic nature — reliable under pressure, comfortable with responsibility, and capable of long-term commitments without losing flexibility.",
                .developed: "A developed Saturn mount marks deep focus and scholarly discipline. Work is approached seriously; endurance and self-reliance carry projects through where enthusiasm alone would fade.",
                .prominent: "A pronounced Saturn mount can tip into excessive seriousness — overwork, self-imposed isolation, or carrying duties that belong to others. The wisdom is real; the boundary-setting needs attention."
            ]
        ),
        "apollo": MountProfile(
            mount: "apollo",
            displayName: "Mount of Apollo",
            planetGlyph: "☉",
            anatomicalLocation: "Beneath the base of the ring finger.",
            governs: "Creativity, charisma, artistic success, and vitality.",
            readings: [
                .deficient: "An understated Apollo mount suggests a private relationship with art and expression — appreciation is strong, but the spotlight holds little appeal. Satisfaction comes from craft, not applause.",
                .balanced: "A balanced Apollo mount shows warm creativity and an easy charm. Beauty is noticed and made; recognition arrives naturally without being chased.",
                .developed: "A developed Apollo mount indicates genuine magnetism — artistic flair, social warmth, and the kind of vitality that draws opportunities. Creative work finds its audience.",
                .prominent: "A pronounced Apollo mount is the showman's mark: charisma, dramatic flair, and a strong pull toward visibility. The energy is real; the risk is scattering it across too many stages."
            ]
        ),
        "mercury": MountProfile(
            mount: "mercury",
            displayName: "Mount of Mercury",
            planetGlyph: "☿",
            anatomicalLocation: "Beneath the base of the little finger.",
            governs: "Communication, wit, commerce, and adaptability.",
            readings: [
                .deficient: "A low Mercury mount suggests a deliberate communicator — words are measured, and quick repartee holds little interest. Trust is earned slowly and rarely misplaced.",
                .balanced: "A balanced Mercury mount reflects a quick, workable mind: easy rapport, practical negotiation, and good instincts for timing in conversation and commerce alike.",
                .developed: "A developed Mercury mount marks persuasiveness — wit, business acumen, and the ability to make complex ideas feel simple. Words open doors.",
                .prominent: "A pronounced Mercury mount signals a restless, deal-making intelligence that thrives on variety. The gift is persuasion; the caution is committing to fewer, deeper ventures."
            ]
        ),
        "venus": MountProfile(
            mount: "venus",
            displayName: "Mount of Venus",
            planetGlyph: "♀",
            anatomicalLocation: "The fleshy mound at the base of the thumb.",
            governs: "Love, warmth, vitality, generosity, and appreciation of beauty.",
            readings: [
                .deficient: "A reserved Venus mount suggests affection expressed through steadiness rather than intensity, and physical energy that is conserved carefully. The heart runs deep but quiet.",
                .balanced: "A balanced Venus mount shows warmth, generosity, and a healthy love of comfort, music, and company — vitality spent gladly and replenished easily.",
                .developed: "A developed Venus mount indicates strong vitality and magnetic warmth: passion for life, loyalty in love, and genuine generosity toward the people in one's orbit.",
                .prominent: "A pronounced Venus mount signals abundant passion and appetite for life's pleasures. The warmth is a gift; the caution is overindulgence and spreading affection too thin."
            ]
        ),
        "luna": MountProfile(
            mount: "luna",
            displayName: "Mount of Luna",
            planetGlyph: "☾",
            anatomicalLocation: "The outer edge of the palm, below the little finger, toward the wrist.",
            governs: "Imagination, intuition, travel, and the inner life.",
            readings: [
                .deficient: "A flat Luna mount belongs to the practical mind — grounded, concrete, and wary of speculation. Change is resisted until its usefulness is proven.",
                .balanced: "A balanced Luna mount reflects gentle imagination and everyday intuition: empathetic, quietly creative, and open to new places without being ruled by them.",
                .developed: "A developed Luna mount marks vivid intuition and a taste for travel and ideas — creativity that draws from dreams as much as from experience.",
                .prominent: "A pronounced Luna mount signals a visionary inner life. Imagination leads; grounding it in routine and deadlines is the lifelong practice."
            ]
        ),
        "mars": MountProfile(
            mount: "mars",
            displayName: "Plain of Mars",
            planetGlyph: "♂",
            anatomicalLocation: "The center of the palm, between the heart and head lines.",
            governs: "Courage, resilience, temperament, and endurance under pressure.",
            readings: [
                .deficient: "A recessed Plain of Mars suggests a conflict-averse temperament — disputes are settled by avoidance or diplomacy rather than confrontation.",
                .balanced: "A balanced Plain of Mars reflects calm courage: pressure is met steadily, anger passes quickly, and resilience is renewed without drama.",
                .developed: "A developed Plain of Mars indicates real fighting spirit — persistence under adversity and the stamina to defend both self and others.",
                .prominent: "A pronounced Plain of Mars signals a quick temper and appetite for challenge. Courage is abundant; its direction determines whether it protects or burns."
            ]
        ),
    ]
}
