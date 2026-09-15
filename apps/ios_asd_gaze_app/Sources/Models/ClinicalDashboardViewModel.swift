import Foundation
import SwiftUI

@MainActor
public final class ObservationLogStore: ObservableObject {
    public static let shared = ObservationLogStore()

    private let key = "org.curv.asd.today.observation"
    @Published public var today: TodayObservation {
        didSet { persist() }
    }

    public init() {
        if let data = UserDefaults.standard.data(forKey: key),
           let decoded = try? JSONDecoder().decode(TodayObservation.self, from: data),
           Calendar.current.isDateInToday(decoded.date) {
            self.today = decoded
        } else {
            self.today = TodayObservation()
        }
    }

    public func toggle(tag: BehavioralObservationTag) {
        if let idx = today.tags.firstIndex(of: tag) {
            today.tags.remove(at: idx)
        } else {
            today.tags.append(tag)
        }
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(today) {
            UserDefaults.standard.set(data, forKey: key)
        }
    }
}

@MainActor
public final class ClinicalDashboardViewModel: ObservableObject {
    @Published public var selectedAge: CDCSurveillanceAge = .months18

    public func trendPoints(
        weekly: [LongitudinalAttentionPoint],
        sessions: [GazeSessionRecord]
    ) -> [AttentionTrendPoint] {
        weekly.enumerated().map { index, point in
            let match = sessions.last(where: {
                Calendar.current.isDate($0.endedAt, equalTo: Date(), toGranularity: .weekOfYear)
            })
            let duration = match?.durationSec ?? 30
            let tag: StimulusContextTag = duration >= 45 ? .screenStimuli : .faceToFace
            return AttentionTrendPoint(
                id: point.id,
                dayLabel: point.dayLabel,
                socialPreferenceScore: point.socialPreferenceScore,
                sessionDurationSec: duration,
                stimulusTag: tag,
                isLatest: index == weekly.count - 1
            )
        }
    }

    public func domainRatio(milestones: [Milestone], domain: DevelopmentalDomain) -> Double {
        let items = milestones.filter { $0.domain == domain }
        guard !items.isEmpty else { return 0 }
        return items.map(\.completionRatio).reduce(0, +) / Double(items.count)
    }
}
