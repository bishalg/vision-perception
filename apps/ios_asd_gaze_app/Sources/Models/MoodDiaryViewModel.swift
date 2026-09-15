import Foundation
import Combine

@MainActor
public final class MoodDiaryViewModel: ObservableObject {
    @Published public var emotion: PediatricEmotion = .engaged
    @Published public var intensity: Double = 0.5
    @Published public var durationMinutes: Double = 10
    @Published public var selectedTags: Set<MoodContextTag> = []
    @Published public var notes: String = ""
    @Published public var source: MoodDiarySource = .manual
    @Published public var saveError: String?
    @Published public var didSave = false

    public let suggestionSeconds: Double?

    private let store: MoodDiaryStore

    public init(
        store: MoodDiaryStore = .shared,
        prefilling suggestion: MoodDiarySuggestion? = nil
    ) {
        self.store = store
        if let suggestion {
            self.emotion = suggestion.suggestedEmotion
            self.intensity = suggestion.intensityHint
            self.durationMinutes = max(2, suggestion.distressSeconds / 60.0)
            self.source = .arkitSuggested
            self.notes = ""
            self.suggestionSeconds = suggestion.distressSeconds
        } else {
            self.suggestionSeconds = nil
        }
    }

    public func toggle(_ tag: MoodContextTag) {
        if selectedTags.contains(tag) {
            selectedTags.remove(tag)
        } else {
            selectedTags.insert(tag)
        }
    }

    public func save() async {
        saveError = nil
        let entry = MoodDiaryEntry(
            emotion: emotion,
            intensity: intensity,
            durationMinutes: durationMinutes,
            tags: MoodContextTag.allCases.filter { selectedTags.contains($0) },
            notes: notes,
            source: source,
            suggestedFromDistressSeconds: suggestionSeconds
        )
        await store.upsert(entry)
        didSave = true
    }
}

public struct MoodDiarySuggestion: Equatable, Sendable, Identifiable {
    public var id: UUID
    public var distressSeconds: Double
    public var suggestedEmotion: PediatricEmotion
    public var intensityHint: Double
    public var prompt: String

    public init(
        id: UUID = UUID(),
        distressSeconds: Double,
        suggestedEmotion: PediatricEmotion = .overstimulated,
        intensityHint: Double = 0.7
    ) {
        self.id = id
        self.distressSeconds = distressSeconds
        self.suggestedEmotion = suggestedEmotion
        self.intensityHint = intensityHint
        self.prompt = "We noticed some distress earlier. Would you like to log this?"
    }
}
