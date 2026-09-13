import CurvAsdGazeKit
import Foundation
import SwiftUI

/// Categories of developmental care plan interventions.
public enum CareTaskCategory: String, Codable, CaseIterable, Sendable {
    case observation = "Gaze Screening"
    case attention = "Joint Attention"
    case language = "Name & Social"
    case play = "Reciprocal Play"

    public var systemIcon: String {
        switch self {
        case .observation: return "eyes"
        case .attention: return "scope"
        case .language: return "bubble.left.and.exclamationmark.bubble.right"
        case .play: return "figure.2.and.child.holdinghands"
        }
    }
}

/// A single daily intervention task in the CareKit protocol.
public struct CarePlanTask: Identifiable, Codable, Sendable, Equatable {
    public let id: String
    public var title: String
    public var subtitle: String
    public var icon: String
    public var category: CareTaskCategory
    public var isCompleted: Bool
    public var completedAt: Date?

    public init(
        id: String,
        title: String,
        subtitle: String,
        icon: String,
        category: CareTaskCategory,
        isCompleted: Bool = false,
        completedAt: Date? = nil
    ) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.icon = icon
        self.category = category
        self.isCompleted = isCompleted
        self.completedAt = completedAt
    }
}

/// Caregiver mood and engagement rating for the child.
public enum ChildMood: String, Codable, CaseIterable, Identifiable, Sendable {
    case engaged = "Engaged"
    case calm = "Calm"
    case distracted = "Distracted"
    case overwhelmed = "Overwhelmed"

    public var id: String { rawValue }

    public var emoji: String {
        switch self {
        case .engaged: return "😊"
        case .calm: return "🙂"
        case .distracted: return "😐"
        case .overwhelmed: return "😫"
        }
    }

    public var localizedLabel: String {
        switch self {
        case .engaged: return "Very Engaged"
        case .calm: return "Calm & Content"
        case .distracted: return "Distracted"
        case .overwhelmed: return "Sensory Sensitive"
        }
    }
}

/// Daily sensory and observation diary entry.
public struct DailyDiaryEntry: Identifiable, Codable, Sendable {
    public var id: UUID
    public var date: Date
    public var mood: ChildMood
    public var notes: String
    public var eyeContactRating: Int // 1 to 5 scale
    public var jointAttentionCount: Int

    public init(
        id: UUID = UUID(),
        date: Date = Date(),
        mood: ChildMood = .calm,
        notes: String = "",
        eyeContactRating: Int = 3,
        jointAttentionCount: Int = 2
    ) {
        self.id = id
        self.date = date
        self.mood = mood
        self.notes = notes
        self.eyeContactRating = eyeContactRating
        self.jointAttentionCount = jointAttentionCount
    }
}

/// Caregiver-logged daily medication / supplement schedule, tracked CareKit-style:
/// one multi-dose daily task per item, each scheduled dose time toggleable for today.
/// The presence of an item is caregiver-entered, not a prescription.
public struct MedicationItem: Identifiable, Codable, Sendable, Equatable {
    public let id: String
    public var name: String
    public var dose: String
    public var instructions: String
    /// "HH:MM" dose times, e.g. ["08:00", "20:00"].
    public let schedule: [String]
    /// Per-dose completion for today, aligned with `schedule`.
    public var dosesTakenToday: [Bool]
    public var lastTakenAt: Date?

    public init(
        id: String,
        name: String,
        dose: String,
        instructions: String,
        schedule: [String],
        dosesTakenToday: [Bool] = [],
        lastTakenAt: Date? = nil
    ) {
        self.id = id
        self.name = name
        self.dose = dose
        self.instructions = instructions
        self.schedule = schedule
        self.dosesTakenToday = Self.alignedDoses(dosesTakenToday, to: schedule)
        self.lastTakenAt = lastTakenAt
    }

    /// Decode leniently: persisted `dosesTakenToday` from an older schedule
    /// shape is resized (prefix preserved, remainder false) instead of
    /// silently disabling doses.
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let schedule = try container.decode([String].self, forKey: .schedule)
        self.init(
            id: try container.decode(String.self, forKey: .id),
            name: try container.decode(String.self, forKey: .name),
            dose: try container.decode(String.self, forKey: .dose),
            instructions: try container.decode(String.self, forKey: .instructions),
            schedule: schedule,
            dosesTakenToday: (try? container.decode([Bool].self, forKey: .dosesTakenToday)) ?? [],
            lastTakenAt: try? container.decode(Date.self, forKey: .lastTakenAt)
        )
    }

    /// Resize a taken-array to the schedule length (prefix preserved).
    private static func alignedDoses(_ doses: [Bool], to schedule: [String]) -> [Bool] {
        if doses.count == schedule.count { return doses }
        var fixed = Array(doses.prefix(schedule.count))
        while fixed.count < schedule.count { fixed.append(false) }
        return fixed
    }
}

/// Longitudinal social visual preference data point.
public struct LongitudinalAttentionPoint: Identifiable, Codable, Sendable {
    public let id: String
    public let dayLabel: String
    public let socialPreferenceScore: Double // -1.0 to +1.0
    public let validFramePercentage: Double
    public let sessionCount: Int

    public init(
        id: String,
        dayLabel: String,
        socialPreferenceScore: Double,
        validFramePercentage: Double,
        sessionCount: Int
    ) {
        self.id = id
        self.dayLabel = dayLabel
        self.socialPreferenceScore = socialPreferenceScore
        self.validFramePercentage = validFramePercentage
        self.sessionCount = sessionCount
    }
}

/// CDC / AAP developmental milestone item.
public struct MilestoneItem: Identifiable, Codable, Sendable {
    public let id: String
    public let ageMonths: Int
    public let title: String
    public let dueDateString: String
    public var isCompleted: Bool
    public let checklist: [String]

    public init(
        id: String,
        ageMonths: Int,
        title: String,
        dueDateString: String,
        isCompleted: Bool,
        checklist: [String]
    ) {
        self.id = id
        self.ageMonths = ageMonths
        self.title = title
        self.dueDateString = dueDateString
        self.isCompleted = isCompleted
        self.checklist = checklist
    }
}

/// Observable store managing daily CareKit tasks, diary, streak, and longitudinal analytics.
@MainActor
public final class CarePlanStore: ObservableObject {
    public static let shared = CarePlanStore()

    /// Injected clinical-record backend (`.shared` in production; an in-memory
    /// store in tests so unit tests never touch the on-disk record).
    private let clinicalStore: CareKitClinicalStore
    /// Injected research-record backend running in parallel to CareKit for study tracking.
    private let researchStore: ResearchKitStudyStore

    @Published public var tasks: [CarePlanTask] = []
    @Published public var diaryEntry: DailyDiaryEntry = DailyDiaryEntry()
    @Published public var medications: [MedicationItem] = []
    @Published public var streakDays: Int = 5
    @Published public var weeklyHistory: [LongitudinalAttentionPoint] = []
    @Published public var milestones: [MilestoneItem] = []
    /// In-flight CareKit mirror write counts per care-plan task id. A counter
    /// (not a set) so rapid double-taps can't clear the spinner early.
    @Published public var pendingCareKitCounts: [String: Int] = [:]

    private let tasksKey = "org.curv.asd.careplan.tasks"
    private let diaryKey = "org.curv.asd.careplan.diary"
    private let medicationsKey = "org.curv.asd.careplan.medications"
    private let streakKey = "org.curv.asd.careplan.streak"
    private let historyKey = "org.curv.asd.careplan.history"

    public init(
        clinicalStore: CareKitClinicalStore = .shared,
        researchStore: ResearchKitStudyStore = .shared
    ) {
        self.clinicalStore = clinicalStore
        self.researchStore = researchStore
        loadState()
    }

    /// Progress ratio of today's completed tasks (0.0 to 1.0).
    public var completionRatio: Double {
        guard !tasks.isEmpty else { return 0.0 }
        let completed = tasks.filter { $0.isCompleted }.count
        return Double(completed) / Double(tasks.count)
    }

    /// Number of completed tasks today.
    public var completedTaskCount: Int {
        tasks.filter { $0.isCompleted }.count
    }

    /// Toggle task completion status and save.
    public func toggleTask(id: String) {
        if let index = tasks.firstIndex(where: { $0.id == id }) {
            tasks[index].isCompleted.toggle()
            tasks[index].completedAt = tasks[index].isCompleted ? Date() : nil
            saveTasks()
            let task = tasks[index]
            // Mirror into the on-device CareKit clinical record (fire-and-forget);
            // the pending counter drives the row's mini spinner until the write lands.
            pendingCareKitCounts[task.id, default: 0] += 1
            clinicalStore.recordTaskCompletion(task, knownTasks: tasks) {
                Task { @MainActor in
                    self.decrementPending(for: task.id)
                }
            }
        }
    }

    /// Decrement the in-flight counter for a task (floor at zero), removing the key.
    private func decrementPending(for id: String) {
        guard let count = pendingCareKitCounts[id] else { return }
        if count <= 1 {
            pendingCareKitCounts.removeValue(forKey: id)
        } else {
            pendingCareKitCounts[id] = count - 1
        }
    }

    /// Update mood entry and persist.
    public func updateMood(_ mood: ChildMood) {
        diaryEntry.mood = mood
        saveDiary()
    }

    /// Update caregiver diary notes and persist.
    public func updateNotes(_ notes: String) {
        diaryEntry.notes = notes
        saveDiary()
    }

    /// Update eye contact rating (1-5) and persist.
    public func updateEyeContactRating(_ rating: Int) {
        diaryEntry.eyeContactRating = max(1, min(5, rating))
        saveDiary()
    }

    /// Update spontaneous joint-attention count for today and persist.
    public func updateJointAttentionCount(_ count: Int) {
        diaryEntry.jointAttentionCount = max(0, count)
        saveDiary()
    }

    /// Toggle one scheduled dose time of a medication for today.
    public func toggleMedicationTaken(id: String, doseIndex: Int) {
        guard let index = medications.firstIndex(where: { $0.id == id }),
              medications[index].dosesTakenToday.indices.contains(doseIndex) else { return }
        medications[index].dosesTakenToday[doseIndex].toggle()
        let isNowTaken = medications[index].dosesTakenToday[doseIndex]
        if medications[index].dosesTakenToday.contains(true) {
            if isNowTaken || medications[index].lastTakenAt == nil {
                medications[index].lastTakenAt = Date()
            }
        } else {
            medications[index].lastTakenAt = nil
        }
        saveMedications()
        let med = medications[index]
        let key = "med_\(med.id)"
        pendingCareKitCounts[key, default: 0] += 1
        // Mirror the dose event into the CareKit clinical record (fire-and-forget).
        clinicalStore.recordMedicationTaken(
            med, doseIndex: doseIndex, markedAt: med.lastTakenAt ?? Date()
        ) { [weak self] in
            Task { @MainActor in
                self?.decrementPending(for: key)
            }
        }
    }

    /// Proportion of today's scheduled doses that have been given (0.0 to 1.0).
    public var medicationCompletionRatio: Double {
        let total = medications.reduce(0) { $0 + $1.dosesTakenToday.count }
        guard total > 0 else { return 0.0 }
        let taken = medications.reduce(0) { $0 + $1.dosesTakenToday.filter { $0 }.count }
        return Double(taken) / Double(total)
    }

    /// Record completion of a gaze screening session in both CareKit and ResearchKit.
    public func markScreeningCompleted(
        socialScore: Double,
        validFramePercentage: Double = 94.0,
        summary: GazeSessionReportSummary? = nil
    ) {
        if let index = tasks.firstIndex(where: { $0.category == .observation }) {
            tasks[index].isCompleted = true
            tasks[index].completedAt = Date()
            saveTasks()
        }
        if let lastIndex = weeklyHistory.indices.last {
            let existing = weeklyHistory[lastIndex]
            weeklyHistory[lastIndex] = LongitudinalAttentionPoint(
                id: existing.id,
                dayLabel: existing.dayLabel,
                socialPreferenceScore: max(0.0, min(1.0, (existing.socialPreferenceScore + socialScore) / 2.0)),
                validFramePercentage: validFramePercentage,
                sessionCount: existing.sessionCount + 1
            )
            saveHistory()
        }
        // 1. CareKit track: Mirror the screening outcome into CareKit (fire-and-forget);
        // keep the observation row's spinner up until the write lands.
        if let observation = tasks.first(where: { $0.category == .observation }) {
            pendingCareKitCounts[observation.id, default: 0] += 1
        }
        clinicalStore.recordScreeningOutcome(
            socialScore: socialScore, validFramePercentage: validFramePercentage, knownTasks: tasks
        ) { [weak self] in
            Task { @MainActor in
                guard let observation = self?.tasks.first(where: { $0.category == .observation }) else { return }
                self?.decrementPending(for: observation.id)
            }
        }
        // 2. ResearchKit track: In parallel, compile and store ORKActiveStepResult
        if let summary {
            researchStore.recordActiveStepResult(summary: summary)
        }
    }

    /// Record completion of M-CHAT-R/F survey in both ResearchKit and CareKit milestone tracking.
    public func recordMCHATSurvey(
        answers: [Int: Bool],
        completion: (@Sendable (MCHATScoreResult) -> Void)? = nil
    ) {
        researchStore.recordSurveyResult(answers: answers) { [weak self] result in
            Task { @MainActor in
                if let idx = self?.milestones.firstIndex(where: { $0.ageMonths == 24 || $0.id.contains("mchat") }) {
                    self?.milestones[idx].isCompleted = true
                }
                completion?(result)
            }
        }
    }

    /// Exports clinical CareKit summary adhering to OCKCarePlanActivity structure.
    public func exportClinicalCareKitJSON() -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601

        struct CareKitExportPayload: Encodable {
            let exportTimestamp: Date
            let protocolVersion: String
            let streakDays: Int
            let completionRate: Double
            let todayDiary: DailyDiaryEntry
            let activeTasks: [CarePlanTask]
            let medications: [MedicationItem]
            let weeklyAttentionTrend: [LongitudinalAttentionPoint]
            let milestonesStatus: [MilestoneItem]
        }

        let payload = CareKitExportPayload(
            exportTimestamp: Date(),
            protocolVersion: "CareKit-ASD-v1.3",
            streakDays: streakDays,
            completionRate: completionRatio,
            todayDiary: diaryEntry,
            activeTasks: tasks,
            medications: medications,
            weeklyAttentionTrend: weeklyHistory,
            milestonesStatus: milestones
        )

        if let data = try? encoder.encode(payload),
           let jsonString = String(data: data, encoding: .utf8) {
            return jsonString
        }
        return "{}"
    }

    // MARK: - Persistence & Initial State

    private func loadState() {
        // Load tasks or initialize defaults
        if let data = UserDefaults.standard.data(forKey: tasksKey),
           let saved = try? JSONDecoder().decode([CarePlanTask].self, from: data) {
            self.tasks = saved
        } else {
            self.tasks = Self.defaultTasks
        }

        // Load diary
        if let data = UserDefaults.standard.data(forKey: diaryKey),
           let saved = try? JSONDecoder().decode(DailyDiaryEntry.self, from: data) {
            self.diaryEntry = saved
        } else {
            self.diaryEntry = DailyDiaryEntry()
        }

        // Load medications (reset per-dose taken state when crossing a new day)
        if let data = UserDefaults.standard.data(forKey: medicationsKey),
           let saved = try? JSONDecoder().decode([MedicationItem].self, from: data) {
            self.medications = saved.map { item in
                if let lastTaken = item.lastTakenAt, !Calendar.current.isDateInToday(lastTaken) {
                    var fresh = item
                    fresh.dosesTakenToday = Array(repeating: false, count: item.schedule.count)
                    return fresh
                }
                return item
            }
        } else {
            self.medications = Self.defaultMedications
        }

        // Load streak
        let savedStreak = UserDefaults.standard.integer(forKey: streakKey)
        self.streakDays = savedStreak > 0 ? savedStreak : 5

        // Load weekly history or populate standard 7-day observation points
        if let data = UserDefaults.standard.data(forKey: historyKey),
           let saved = try? JSONDecoder().decode([LongitudinalAttentionPoint].self, from: data) {
            self.weeklyHistory = saved
        } else {
            self.weeklyHistory = Self.defaultWeeklyHistory
        }

        // Milestones
        self.milestones = Self.defaultMilestones
    }

    private func saveTasks() {
        if let data = try? JSONEncoder().encode(tasks) {
            UserDefaults.standard.set(data, forKey: tasksKey)
        }
    }

    private func saveDiary() {
        if let data = try? JSONEncoder().encode(diaryEntry) {
            UserDefaults.standard.set(data, forKey: diaryKey)
        }
        // Mirror the diary into the on-device CareKit clinical record (fire-and-forget).
        clinicalStore.recordDiaryEntry(diaryEntry)
    }

    private func saveMedications() {
        if let data = try? JSONEncoder().encode(medications) {
            UserDefaults.standard.set(data, forKey: medicationsKey)
        }
    }

    private func saveHistory() {
        if let data = try? JSONEncoder().encode(weeklyHistory) {
            UserDefaults.standard.set(data, forKey: historyKey)
        }
    }

    // MARK: - Clinical Defaults

    private static var defaultTasks: [CarePlanTask] {
        [
            CarePlanTask(
                id: "task_gaze_screen",
                title: "Daily Social Gaze Screening",
                subtitle: "60-second TrueDepth eye-tracking stimulus observation",
                icon: "eyes",
                category: .observation,
                isCompleted: true,
                completedAt: Date().addingTimeInterval(-3600)
            ),
            CarePlanTask(
                id: "task_joint_attention",
                title: "Joint Attention Engagement",
                subtitle: "Point at 3 novel objects during play & monitor gaze shifting",
                icon: "scope",
                category: .attention,
                isCompleted: true,
                completedAt: Date().addingTimeInterval(-1800)
            ),
            CarePlanTask(
                id: "task_name_orient",
                title: "Name Orientation Trial",
                subtitle: "Call child's name from 2m away (3 trials) & record latency",
                icon: "bubble.left.and.exclamationmark.bubble.right",
                category: .language,
                isCompleted: false,
                completedAt: nil
            ),
            CarePlanTask(
                id: "task_reciprocal_play",
                title: "Reciprocal Turn-Taking",
                subtitle: "5-minute roll-the-ball or stacking block exchange",
                icon: "figure.2.and.child.holdinghands",
                category: .play,
                isCompleted: false,
                completedAt: nil
            )
        ]
    }

    private static var defaultWeeklyHistory: [LongitudinalAttentionPoint] {
        [
            LongitudinalAttentionPoint(id: "day_1", dayLabel: "Mon", socialPreferenceScore: 0.42, validFramePercentage: 91.0, sessionCount: 1),
            LongitudinalAttentionPoint(id: "day_2", dayLabel: "Tue", socialPreferenceScore: 0.48, validFramePercentage: 95.0, sessionCount: 1),
            LongitudinalAttentionPoint(id: "day_3", dayLabel: "Wed", socialPreferenceScore: 0.39, validFramePercentage: 88.0, sessionCount: 1),
            LongitudinalAttentionPoint(id: "day_4", dayLabel: "Thu", socialPreferenceScore: 0.52, validFramePercentage: 94.0, sessionCount: 2),
            LongitudinalAttentionPoint(id: "day_5", dayLabel: "Fri", socialPreferenceScore: 0.46, validFramePercentage: 90.0, sessionCount: 1),
            LongitudinalAttentionPoint(id: "day_6", dayLabel: "Sat", socialPreferenceScore: 0.55, validFramePercentage: 96.0, sessionCount: 2),
            LongitudinalAttentionPoint(id: "day_7", dayLabel: "Sun", socialPreferenceScore: 0.58, validFramePercentage: 97.0, sessionCount: 1)
        ]
    }

    /// Caregiver-editable placeholders for common daily supplements. Their
    /// presence here is a tracking aid, not a prescription.
    private static var defaultMedications: [MedicationItem] {
        [
            MedicationItem(
                id: "med_vitamin_d",
                name: "Vitamin D3 Drops",
                dose: "400 IU",
                instructions: "Give with morning meal.",
                schedule: ["08:00"]
            ),
            MedicationItem(
                id: "med_omega_3",
                name: "Omega-3 Support",
                dose: "500 mg",
                instructions: "Half with breakfast, half with dinner.",
                schedule: ["08:00", "20:00"]
            ),
            MedicationItem(
                id: "med_melatonin",
                name: "Melatonin",
                dose: "2 mg",
                instructions: "If advised by clinician; 30 min before bedtime routine.",
                schedule: ["19:45"]
            )
        ]
    }

    private static var defaultMilestones: [MilestoneItem] {
        [
            MilestoneItem(
                id: "ms_18m",
                ageMonths: 18,
                title: "18-Month Early Milestones",
                dueDateString: "Completed May 15, 2026",
                isCompleted: true,
                checklist: [
                    "Maintains reciprocal eye contact during peek-a-boo",
                    "Points to show something interesting (declarative pointing)",
                    "Follows parent's pointing gesture to target",
                    "Responds consistently to name within 2 calls"
                ]
            ),
            MilestoneItem(
                id: "ms_24m",
                ageMonths: 24,
                title: "24-Month Follow-Up Check",
                dueDateString: "Due Window: Nov 2026",
                isCompleted: false,
                checklist: [
                    "Notices when others are hurt or upset",
                    "Uses 2-word phrases autonomously",
                    "Engages in simple pretend play (feeding a doll)",
                    "Prefers social interaction over repetitive visual objects"
                ]
            ),
            MilestoneItem(
                id: "ms_36m",
                ageMonths: 36,
                title: "36-Month Longitudinal Review",
                dueDateString: "Scheduled: Nov 2027",
                isCompleted: false,
                checklist: [
                    "Engages in cooperative group play with peers",
                    "Understands conversational turn-taking",
                    "Displays varied emotional facial expressions matching context",
                    "Stable social gaze preference (>50% visual attention)"
                ]
            )
        ]
    }
}
