import Foundation
import Combine

/// On-device mood diary with optional HealthKit and CareKit daily-mood mirrors.
@MainActor
public final class MoodDiaryStore: ObservableObject {
    public static let shared = MoodDiaryStore()

    @Published public private(set) var entries: [MoodDiaryEntry] = []

    private let defaults: UserDefaults
    private let storageKey = "org.curv.asd.mood.diary.entries.v1"
    private let healthKit: HealthKitMoodStore
    private let carePlan: CarePlanStore?

    public init(
        defaults: UserDefaults = .standard,
        healthKit: HealthKitMoodStore = .shared,
        carePlan: CarePlanStore? = CarePlanStore.shared
    ) {
        self.defaults = defaults
        self.healthKit = healthKit
        self.carePlan = carePlan
        load()
    }

    public var recentEntries: [MoodDiaryEntry] {
        entries.sorted { $0.loggedAt > $1.loggedAt }
    }

    public func upsert(_ entry: MoodDiaryEntry, syncHealth: Bool = true) async {
        var next = entry
        if syncHealth, next.healthKitUUID == nil {
            next.healthKitUUID = await healthKit.save(next)
        }
        if let index = entries.firstIndex(where: { $0.id == next.id }) {
            entries[index] = next
        } else {
            entries.append(next)
        }
        persist()
        if let carePlan {
            carePlan.updateMood(next.emotion.childMood)
            if !next.notes.isEmpty {
                let merged = [carePlan.diaryEntry.notes, next.notes]
                    .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                    .filter { !$0.isEmpty }
                    .joined(separator: "\n")
                carePlan.updateNotes(merged)
            }
        }
    }

    public func delete(_ id: UUID) async {
        guard let existing = entries.first(where: { $0.id == id }) else { return }
        if let hk = existing.healthKitUUID {
            await healthKit.deleteHealthSample(uuid: hk)
        }
        entries.removeAll { $0.id == id }
        persist()
    }

    private func load() {
        guard let data = defaults.data(forKey: storageKey),
              let decoded = try? JSONDecoder().decode([MoodDiaryEntry].self, from: data) else {
            entries = []
            return
        }
        entries = decoded
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(entries) {
            defaults.set(data, forKey: storageKey)
        }
    }
}
