import Foundation

#if canImport(HealthKit)
import HealthKit
#endif

/// Unified HealthKit authorization and CRUD for mood / State of Mind samples.
/// Local diary remains source of truth when Health is unavailable or denied.
@MainActor
public final class HealthKitMoodStore: ObservableObject {
    public static let shared = HealthKitMoodStore()

    @Published public private(set) var authorizationStatusText: String = "Not requested"
    @Published public private(set) var lastErrorMessage: String?

    public var isHealthDataAvailable: Bool {
        #if canImport(HealthKit)
        HKHealthStore.isHealthDataAvailable()
        #else
        false
        #endif
    }

    #if canImport(HealthKit)
    private let store = HKHealthStore()
    #endif

    private init() {}

    public func requestAuthorization() async {
        lastErrorMessage = nil
        guard isHealthDataAvailable else {
            authorizationStatusText = "Health data not available on this device"
            return
        }
        #if canImport(HealthKit)
        #if compiler(>=6.0)
        if #available(iOS 18.0, *) {
            do {
                let type = HKStateOfMindType.stateOfMindType()
                try await store.requestAuthorization(toShare: [type], read: [type])
                authorizationStatusText = "Authorized (State of Mind)"
            } catch {
                lastErrorMessage = error.localizedDescription
                authorizationStatusText = "Authorization failed"
            }
            return
        }
        #endif
        authorizationStatusText = "State of Mind requires iOS 18 SDK"
        #else
        authorizationStatusText = "HealthKit not linked"
        #endif
    }

    /// Writes a diary row to Apple Health as a momentary State of Mind sample.
    @discardableResult
    public func save(_ entry: MoodDiaryEntry) async -> UUID? {
        lastErrorMessage = nil
        #if canImport(HealthKit)
        #if compiler(>=6.0)
        if #available(iOS 18.0, *) {
            do {
                let sample = try makeStateOfMindSample(from: entry)
                try await store.save(sample)
                return sample.uuid
            } catch {
                lastErrorMessage = error.localizedDescription
                return nil
            }
        }
        #endif
        #endif
        return nil
    }

    public func deleteHealthSample(uuid: UUID) async {
        #if canImport(HealthKit)
        #if compiler(>=6.0)
        if #available(iOS 18.0, *) {
            do {
                let type = HKStateOfMindType.stateOfMindType()
                let predicate = HKQuery.predicateForObject(with: uuid)
                try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
                    store.deleteObjects(of: type, predicate: predicate) { _, _, error in
                        if let error {
                            cont.resume(throwing: error)
                        } else {
                            cont.resume()
                        }
                    }
                }
            } catch {
                lastErrorMessage = error.localizedDescription
            }
        }
        #endif
        #endif
    }

    public func fetchRecentStateOfMind(limit: Int = 50) async -> [MoodDiaryEntry] {
        #if canImport(HealthKit)
        #if compiler(>=6.0)
        if #available(iOS 18.0, *) {
            do {
                return try await queryStateOfMind(limit: limit)
            } catch {
                lastErrorMessage = error.localizedDescription
            }
        }
        #endif
        #endif
        return []
    }

    #if canImport(HealthKit)
    #if compiler(>=6.0)
    @available(iOS 18.0, *)
    private func makeStateOfMindSample(from entry: MoodDiaryEntry) throws -> HKStateOfMind {
        let valence = min(1, max(-1, entry.emotion.healthValence * (0.5 + entry.intensity * 0.5)))
        let labels = healthKitLabels(for: entry.emotion)
        return HKStateOfMind(
            date: entry.loggedAt,
            kind: .momentaryEmotion,
            valence: valence,
            labels: labels,
            associations: []
        )
    }

    @available(iOS 18.0, *)
    private func healthKitLabels(for emotion: PediatricEmotion) -> [HKStateOfMind.Label] {
        switch emotion {
        case .joy: return [.joyful, .happy]
        case .engaged: return [.content, .calm]
        case .overstimulated: return [.overwhelmed, .stressed]
        case .meltdown: return [.frustrated, .sad]
        }
    }

    @available(iOS 18.0, *)
    private func queryStateOfMind(limit: Int) async throws -> [MoodDiaryEntry] {
        let type = HKStateOfMindType.stateOfMindType()
        let sort = NSSortDescriptor(key: HKSampleSortIdentifierEndDate, ascending: false)
        return try await withCheckedThrowingContinuation { cont in
            let query = HKSampleQuery(sampleType: type, predicate: nil, limit: limit, sortDescriptors: [sort]) { _, samples, error in
                if let error {
                    cont.resume(throwing: error)
                    return
                }
                let mapped = (samples as? [HKStateOfMind] ?? []).map { sample -> MoodDiaryEntry in
                    let emotion = PediatricEmotion.fromHealthValence(sample.valence)
                    return MoodDiaryEntry(
                        id: sample.uuid,
                        loggedAt: sample.endDate,
                        emotion: emotion,
                        intensity: abs(sample.valence),
                        durationMinutes: 5,
                        source: .manual,
                        healthKitUUID: sample.uuid
                    )
                }
                cont.resume(returning: mapped)
            }
            store.execute(query)
        }
    }
    #endif
    #endif
}

extension PediatricEmotion {
    static func fromHealthValence(_ valence: Double) -> PediatricEmotion {
        if valence >= 0.5 { return .joy }
        if valence >= 0 { return .engaged }
        if valence >= -0.6 { return .overstimulated }
        return .meltdown
    }
}
