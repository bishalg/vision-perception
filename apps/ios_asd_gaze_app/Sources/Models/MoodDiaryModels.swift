import Foundation
import CurvAsdGazeKit

/// Age-appropriate emotion labels aligned with CDC social-emotional milestones
/// (joy/engagement, distress/meltdown, sensory overstimulation).
public enum PediatricEmotion: String, Codable, CaseIterable, Identifiable, Sendable {
    case joy
    case engaged
    case overstimulated
    case meltdown

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .joy: return "Joy"
        case .engaged: return "Engaged"
        case .overstimulated: return "Overstimulated"
        case .meltdown: return "Meltdown"
        }
    }

    public var subtitle: String {
        switch self {
        case .joy: return "Smiles, play, shared delight"
        case .engaged: return "Attending, responding, joint focus"
        case .overstimulated: return "Covering ears, withdrawing, overload"
        case .meltdown: return "Intense distress that is hard to settle"
        }
    }

    public var systemImage: String {
        switch self {
        case .joy: return "face.smiling.fill"
        case .engaged: return "eye.fill"
        case .overstimulated: return "waveform.path.ecg"
        case .meltdown: return "exclamationmark.triangle.fill"
        }
    }

    public var childMood: ChildMood {
        switch self {
        case .joy, .engaged: return .engaged
        case .overstimulated: return .overwhelmed
        case .meltdown: return .overwhelmed
        }
    }

    /// HealthKit State of Mind valence in [-1, 1].
    public var healthValence: Double {
        switch self {
        case .joy: return 0.65
        case .engaged: return 0.40
        case .overstimulated: return -0.45
        case .meltdown: return -0.80
        }
    }

    public var healthLabelIdentifiers: [String] {
        switch self {
        case .joy: return ["joyful", "happy"]
        case .engaged: return ["content", "calm"]
        case .overstimulated: return ["overwhelmed", "stressed"]
        case .meltdown: return ["frustrated", "sad"]
        }
    }

    public static func fromDominantEmotion(_ emotion: DominantEmotion) -> PediatricEmotion {
        switch emotion {
        case .joy, .excited: return .joy
        case .calm, .surprise: return .engaged
        case .distress: return .overstimulated
        }
    }
}

public enum MoodContextTag: String, Codable, CaseIterable, Identifiable, Sendable {
    case postSchool = "Post-School"
    case transitionPeriod = "Transition Period"
    case loudEnvironment = "Loud Environment"
    case mealtime = "Mealtime"
    case bedtime = "Bedtime"
    case therapy = "Therapy / Session"
    case familyVisit = "Family Visit"
    case screens = "Screens"

    public var id: String { rawValue }
}

public enum MoodDiarySource: String, Codable, Sendable {
    case manual
    case arkitSuggested
}

public struct MoodDiaryEntry: Identifiable, Codable, Sendable, Equatable {
    public var id: UUID
    public var loggedAt: Date
    public var emotion: PediatricEmotion
    public var intensity: Double
    public var durationMinutes: Double
    public var tags: [MoodContextTag]
    public var notes: String
    public var source: MoodDiarySource
    public var healthKitUUID: UUID?
    public var suggestedFromDistressSeconds: Double?

    public init(
        id: UUID = UUID(),
        loggedAt: Date = Date(),
        emotion: PediatricEmotion = .engaged,
        intensity: Double = 0.5,
        durationMinutes: Double = 5,
        tags: [MoodContextTag] = [],
        notes: String = "",
        source: MoodDiarySource = .manual,
        healthKitUUID: UUID? = nil,
        suggestedFromDistressSeconds: Double? = nil
    ) {
        self.id = id
        self.loggedAt = loggedAt
        self.emotion = emotion
        self.intensity = min(1, max(0, intensity))
        self.durationMinutes = min(180, max(0.5, durationMinutes))
        self.tags = tags
        self.notes = notes
        self.source = source
        self.healthKitUUID = healthKitUUID
        self.suggestedFromDistressSeconds = suggestedFromDistressSeconds
    }
}

/// Blend-shape distress signature used by ARKit fusion (AU1 + AU15).
public enum DistressBlendShapeHeuristic: Sendable {
    public static let browInnerUpThreshold: Double = 0.35
    public static let mouthFrownThreshold: Double = 0.30
    public static let sustainedPromptSeconds: TimeInterval = 120
    public static let sessionEndPromptSeconds: TimeInterval = 8

    public static func isDistressFrame(
        browInnerUp: Double,
        mouthFrownLeft: Double,
        mouthFrownRight: Double
    ) -> Bool {
        let frown = (mouthFrownLeft + mouthFrownRight) / 2.0
        return browInnerUp > browInnerUpThreshold && frown > mouthFrownThreshold
    }

    public static func isDistress(from affect: FacialAffectMetrics) -> Bool {
        isDistressFrame(
            browInnerUp: affect.browInnerUp,
            mouthFrownLeft: affect.mouthFrownLeft,
            mouthFrownRight: affect.mouthFrownRight
        )
    }
}
