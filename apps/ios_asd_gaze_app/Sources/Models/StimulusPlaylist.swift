import Foundation
import CurvAsdGazeKit

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

/// Screening playlists read from `stimulus_catalog.json`.
/// Clip membership, order, duration, and age range live in that file.
public enum StimulusPlaylistBattery {

    /// Standard 30-second express battery for the full supported age span.
    public static var standard30s: [StimulusPlaylistItem] {
        playlist(forTargetDuration: 30)
    }

    /// Comprehensive 60-second battery for the full supported age span.
    public static var comprehensive60s: [StimulusPlaylistItem] {
        playlist(forTargetDuration: 60)
    }

    /// Every clip the shipped 30s and 60s batteries can ask for. This is the download checklist.
    public static var shippedClips: [StimulusPlaylistItem] {
        var seen = Set<String>()
        return (standard30s + comprehensive60s).filter { seen.insert($0.videoName).inserted }
    }

    /// Playlist for a session length. Pass `ageMonths` to keep only clips marked for that age.
    public static func playlist(forTargetDuration durationSec: Double, ageMonths: Int? = nil) -> [StimulusPlaylistItem] {
        StimulusCatalog.current.playlist(forTargetDuration: durationSec, ageMonths: ageMonths).map { clip in
            StimulusPlaylistItem(
                videoName: clip.id,
                title: clip.title,
                category: clip.category,
                targetDurationSec: clip.durationSec
            )
        }
    }
}
