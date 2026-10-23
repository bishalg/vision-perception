import Foundation

// MARK: - Stimulus Playlist Item

/// Represents an individual clinical stimulus item inside a screening playlist.
public struct StimulusPlaylistItem: Identifiable, Sendable, Equatable {
    public let id: String
    public let videoName: String
    public let title: String
    public let category: String
    public let targetDurationSec: Double

    public init(
        videoName: String,
        title: String,
        category: String,
        targetDurationSec: Double
    ) {
        self.id = videoName
        self.videoName = videoName
        self.title = title
        self.category = category
        self.targetDurationSec = targetDurationSec
    }
}

// MARK: - Clinical Stimulus Playlist Battery

/// Pre-configured, evidence-based stimulus batteries for pediatric screening sessions.
/// Derived from landmark ASD visual attention literature (JAMA 2023 EarliPoint, NEJM AI 2024 SenseToKnow).
public enum StimulusPlaylistBattery {

    /// Standard 30-Second Express Screening Battery (4 clinical clips, ~7–8s each).
    /// Focus: Geometric vs Social preference, direct eye contact, joint attention, and dynamic affect.
    public static let standard30s: [StimulusPlaylistItem] = [
        StimulusPlaylistItem(
            videoName: "stim_05_geopref_split_10s",
            title: "Geometric vs Social Preference",
            category: "GeoPref",
            targetDurationSec: 8.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_01_social_peekaboo_8s",
            title: "Direct Social Eye Contact",
            category: "Social Engagement",
            targetDurationSec: 8.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_02_gaze_cue_right_8s",
            title: "Joint Attention & Gaze Following",
            category: "Joint Attention",
            targetDurationSec: 7.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_07_cartoon_slapstick_penguin_8s",
            title: "Animated Affect & Anticipation",
            category: "Dynamic Affect",
            targetDurationSec: 7.0
        )
    ]

    /// Comprehensive 60-Second (1-Minute) Full Clinical Battery (8 clinical clips, ~7–9s each).
    /// Focus: Deep evaluation across preference, mutual gaze, bilateral joint attention, surprise, recentering, turn-taking, and animal motion.
    public static let comprehensive60s: [StimulusPlaylistItem] = [
        StimulusPlaylistItem(
            videoName: "stim_05_geopref_split_10s",
            title: "Geometric vs Social Preference",
            category: "GeoPref",
            targetDurationSec: 8.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_01_social_peekaboo_8s",
            title: "Direct Social Eye Contact",
            category: "Social Engagement",
            targetDurationSec: 8.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_02_gaze_cue_right_8s",
            title: "Joint Attention: Rightward Cue",
            category: "Joint Attention",
            targetDurationSec: 7.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_03_gaze_cue_left_8s",
            title: "Joint Attention: Leftward Cue",
            category: "Joint Attention",
            targetDurationSec: 7.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_04_surprise_giftbox_10s",
            title: "Affective Surprise Discovery",
            category: "Facial Affect",
            targetDurationSec: 8.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_08_recenter_bubble_fox_3s",
            title: "Center Fixation Recenter",
            category: "Fixation Recenter",
            targetDurationSec: 4.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_09_social_turn_taking_10s",
            title: "Turn-Taking & Conversational Gaze",
            category: "Social Communication",
            targetDurationSec: 9.0
        ),
        StimulusPlaylistItem(
            videoName: "stim_10_animal_puppy_play_8s",
            title: "Biological Non-Human Motion",
            category: "Biological Control",
            targetDurationSec: 9.0
        )
    ]

    /// Every clip the shipped 30s and 60s batteries can ask for. This is the download checklist.
    public static var shippedClips: [StimulusPlaylistItem] {
        var seen = Set<String>()
        return (standard30s + comprehensive60s).filter { seen.insert($0.videoName).inserted }
    }

    /// Returns the appropriate stimulus battery for the targeted screening session duration.
    public static func playlist(forTargetDuration durationSec: Double) -> [StimulusPlaylistItem] {
        if durationSec >= 50.0 {
            return comprehensive60s
        } else {
            return standard30s
        }
    }
}
