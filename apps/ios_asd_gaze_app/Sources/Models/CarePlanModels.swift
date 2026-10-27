import CurvAsdGazeKit
import Foundation
import SwiftUI

// MARK: - Care Plan Store
// Domain models (CareTaskCategory, CarePlanTask, ChildMood, DailyDiaryEntry, MedicationItem, LongitudinalAttentionPoint)
// are defined in CarePlanDomainModels.swift.

/// Observable store managing daily CareKit tasks, diary, streak, and longitudinal analytics.
@MainActor
public final class CarePlanStore: ObservableObject {
    public static let shared = CarePlanStore()

    /// Injected clinical-record backend (`.shared` in production; an in-memory
    /// store in tests so unit tests never touch the on-disk record).
    private let clinicalStore: CareKitClinicalStore
    /// Injected research-record backend running in parallel to CareKit for study tracking.
    private let researchStore: ResearchKitStudyStore
    /// Persistence for care-plan state (`.standard` in production; a suite in tests).
    private let defaults: UserDefaults

    @Published public var tasks: [CarePlanTask] = []
    @Published public var diaryEntry: DailyDiaryEntry = DailyDiaryEntry()
    @Published public var diaryHistory: [DailyDiaryEntry] = []
    @Published public var diaryReminderSlots: [CareReminderSlot] = []
    @Published public var medications: [MedicationItem] = []
    @Published public var streakDays: Int = 5
    @Published public var weeklyHistory: [LongitudinalAttentionPoint] = []
    @Published public var milestones: [MilestoneItem] = []
    /// In-flight CareKit mirror write counts per care-plan task id. A counter
    /// (not a set) so rapid double-taps can't clear the spinner early.
    @Published public var pendingCareKitCounts: [String: Int] = [:]

    private let tasksKey = "org.curv.asd.careplan.tasks"
    private let diaryKey = "org.curv.asd.careplan.diary"
    private let diaryHistoryKey = "org.curv.asd.careplan.diary.history"
    private let diaryRemindersKey = "org.curv.asd.careplan.diary.reminders"
    private let medicationsKey = "org.curv.asd.careplan.medications"
    private let streakKey = "org.curv.asd.careplan.streak"
    private let historyKey = "org.curv.asd.careplan.history"
    private let milestonesKey = "org.curv.asd.careplan.milestones.v2"

    /// Prefix shared by every persisted care-plan key.
    public static let keyPrefix = "org.curv.asd.careplan."

    public init(
        clinicalStore: CareKitClinicalStore = .shared,
        researchStore: ResearchKitStudyStore = .shared,
        defaults: UserDefaults = .standard
    ) {
        self.clinicalStore = clinicalStore
        self.researchStore = researchStore
        self.defaults = defaults
        loadState()
        Task { await resyncAlerts() }
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

    public func toggleMilestoneItem(milestoneId: String, itemId: String) {
        guard let mIdx = milestones.firstIndex(where: { $0.id == milestoneId }),
              let iIdx = milestones[mIdx].items.firstIndex(where: { $0.id == itemId }) else { return }
        milestones[mIdx].items[iIdx].isCompleted.toggle()
        milestones[mIdx].isCompleted = milestones[mIdx].items.allSatisfy(\.isCompleted)
        saveMilestones()
    }

    public func domainCompletion(_ domain: DevelopmentalDomain) -> Double {
        let items = milestones.filter { $0.domain == domain }
        guard !items.isEmpty else { return 0 }
        return items.map(\.completionRatio).reduce(0, +) / Double(items.count)
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

    /// Diary for a calendar day, or a blank template if none is stored.
    public func diary(for date: Date) -> DailyDiaryEntry {
        let day = Calendar.current.startOfDay(for: date)
        if let existing = diaryHistory.first(where: { Calendar.current.isDate($0.date, inSameDayAs: day) }) {
            return existing
        }
        if Calendar.current.isDateInToday(day) {
            return diaryEntry
        }
        return DailyDiaryEntry(date: day)
    }

    /// Recent diary days, newest first (includes today even if blank).
    public var recentDiaryEntries: [DailyDiaryEntry] {
        let today = diary(for: Date())
        var items = diaryHistory.sorted { $0.date > $1.date }
        if !items.contains(where: { Calendar.current.isDateInToday($0.date) }) {
            items.insert(today, at: 0)
        }
        return items
    }

    /// Update mood entry and persist.
    public func updateMood(_ mood: ChildMood) {
        mutateTodayDiary { $0.mood = mood }
    }

    /// Update caregiver diary notes and persist.
    public func updateNotes(_ notes: String) {
        mutateTodayDiary { $0.notes = notes }
    }

    /// Update eye contact rating (1-5) and persist.
    public func updateEyeContactRating(_ rating: Int) {
        mutateTodayDiary { $0.eyeContactRating = max(1, min(5, rating)) }
    }

    /// Update spontaneous joint-attention count for today and persist.
    public func updateJointAttentionCount(_ count: Int) {
        mutateTodayDiary { $0.jointAttentionCount = max(0, count) }
    }

    public func updateSensoryLoad(_ load: Int) {
        mutateTodayDiary { $0.sensoryLoad = max(1, min(5, load)) }
    }

    public func upsertDiary(_ entry: DailyDiaryEntry) {
        var copy = entry
        copy.date = Calendar.current.startOfDay(for: entry.date)
        copy.lastEditedAt = Date()
        if let index = diaryHistory.firstIndex(where: { Calendar.current.isDate($0.date, inSameDayAs: copy.date) }) {
            diaryHistory[index] = copy
        } else {
            diaryHistory.append(copy)
        }
        if Calendar.current.isDateInToday(copy.date) {
            diaryEntry = copy
        }
        persistDiary()
        clinicalStore.recordDiaryEntry(copy, date: copy.date)
        Task { await resyncAlerts() }
    }

    public func deleteDiary(on date: Date) {
        diaryHistory.removeAll { Calendar.current.isDate($0.date, inSameDayAs: date) }
        if Calendar.current.isDateInToday(date) {
            diaryEntry = DailyDiaryEntry(date: Calendar.current.startOfDay(for: date))
        }
        persistDiary()
        Task { await resyncAlerts() }
    }

    public func upsertDiaryReminderSlot(_ slot: CareReminderSlot) {
        if let index = diaryReminderSlots.firstIndex(where: { $0.id == slot.id }) {
            diaryReminderSlots[index] = slot
        } else {
            diaryReminderSlots.append(slot)
        }
        saveDiaryReminders()
        Task { await resyncAlerts() }
    }

    public func deleteDiaryReminderSlot(id: String) {
        diaryReminderSlots.removeAll { $0.id == id }
        saveDiaryReminders()
        Task { await resyncAlerts() }
    }

    private func mutateTodayDiary(_ body: (inout DailyDiaryEntry) -> Void) {
        var entry = diary(for: Date())
        body(&entry)
        upsertDiary(entry)
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
        Task { await resyncAlerts() }
    }

    public func upsertMedication(_ item: MedicationItem) {
        var copy = item
        copy.setSchedule(item.schedule)
        if let index = medications.firstIndex(where: { $0.id == copy.id }) {
            copy.dosesTakenToday = Self.mergeTaken(
                existing: medications[index].dosesTakenToday,
                incoming: copy.dosesTakenToday,
                scheduleCount: copy.schedule.count
            )
            medications[index] = copy
        } else {
            medications.append(copy)
        }
        saveMedications()
        clinicalStore.replaceMedicationTask(from: copy)
        Task { await resyncAlerts() }
    }

    public func deleteMedication(id: String) {
        medications.removeAll { $0.id == id }
        saveMedications()
        clinicalStore.deleteMedicationTask(id: id)
        Task { await resyncAlerts() }
    }

    private static func mergeTaken(existing: [Bool], incoming: [Bool], scheduleCount: Int) -> [Bool] {
        if incoming.count == scheduleCount, incoming.contains(true) { return incoming }
        var fixed = Array(existing.prefix(scheduleCount))
        while fixed.count < scheduleCount { fixed.append(false) }
        return fixed
    }

    public func resyncAlerts() async {
        let prefs = CareAlertPreferences.shared
        let loggedToday = diary(for: Date()).hasCaregiverInput
        let requests = CareAlertScheduler.shared.buildRequests(
            medications: medications,
            diarySlots: diaryReminderSlots,
            diaryLoggedToday: loggedToday
        )
        await CareAlertScheduler.shared.resync(requests: requests, preferences: prefs)
    }

    /// Proportion of today's scheduled doses that have been given (0.0 to 1.0).
    public var medicationCompletionRatio: Double {
        let total = medications.reduce(0) { $0 + $1.dosesTakenToday.count }
        guard total > 0 else { return 0.0 }
        let taken = medications.reduce(0) { $0 + $1.dosesTakenToday.filter { $0 }.count }
        return Double(taken) / Double(total)
    }

    /// Record completion of a gaze screening session in both CareKit and ResearchKit.
    /// Simulated sessions are ignored entirely (SIM-5): no task, trend, CareKit or
    /// ResearchKit state changes.
    public func markScreeningCompleted(
        captureSource: CaptureSource,
        socialScore: Double,
        validFramePercentage: Double = 94.0,
        summary: GazeSessionReportSummary? = nil,
        riskScore: Double? = nil,
        riskStratification: String? = nil
    ) {
        guard captureSource != .simulated, summary?.captureSource != .simulated else { return }
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
        #if ASD_RESEARCH_CLASSIFIER
        clinicalStore.recordScreeningOutcome(
            captureSource: captureSource,
            socialScore: socialScore,
            validFramePercentage: validFramePercentage,
            knownTasks: tasks,
            riskScore: riskScore,
            riskStratification: riskStratification
        ) { [weak self] in
            Task { @MainActor in
                guard let observation = self?.tasks.first(where: { $0.category == .observation }) else { return }
                self?.decrementPending(for: observation.id)
            }
        }
        #else
        clinicalStore.recordScreeningOutcome(
            captureSource: captureSource,
            socialScore: socialScore,
            validFramePercentage: validFramePercentage,
            knownTasks: tasks
        ) { [weak self] in
            Task { @MainActor in
                guard let observation = self?.tasks.first(where: { $0.category == .observation }) else { return }
                self?.decrementPending(for: observation.id)
            }
        }
        #endif
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
                if let idx = self?.milestones.firstIndex(where: { $0.ageMonths == 24 && $0.domain == .languageCommunication }) {
                    self?.milestones[idx].isCompleted = true
                    self?.saveMilestones()
                }
                completion?(result)
            }
        }
    }

    /// Deletes every care-plan record (CNS-6, after its own confirmation): all
    /// persisted `org.curv.asd.careplan.*` keys, the in-memory state, and the
    /// CareKit mirror (`CareKitClinicalStore.deleteAllRecords`). Defaults are
    /// re-seeded on the next launch. `completion` fires on the main actor once the
    /// CareKit mirror has been reset.
    public func deleteAllCarePlanData(completion: (@Sendable () -> Void)? = nil) {
        for key in defaults.dictionaryRepresentation().keys where key.hasPrefix(Self.keyPrefix) {
            defaults.removeObject(forKey: key)
        }
        tasks = []
        diaryEntry = DailyDiaryEntry()
        diaryHistory = []
        diaryReminderSlots = []
        medications = []
        weeklyHistory = []
        milestones = []
        pendingCareKitCounts = [:]
        clinicalStore.deleteAllRecords { _ in completion?() }
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
            let diaryHistory: [DailyDiaryEntry]
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
            diaryHistory: diaryHistory,
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
        if let data = defaults.data(forKey: tasksKey),
           let saved = try? JSONDecoder().decode([CarePlanTask].self, from: data) {
            self.tasks = saved
        } else {
            self.tasks = Self.defaultTasks
        }

        // Load diary (legacy single entry + history)
        if let data = defaults.data(forKey: diaryHistoryKey),
           let saved = try? JSONDecoder().decode([DailyDiaryEntry].self, from: data),
           !saved.isEmpty {
            self.diaryHistory = saved
            self.diaryEntry = diary(for: Date())
        } else if let data = defaults.data(forKey: diaryKey),
                  let saved = try? JSONDecoder().decode(DailyDiaryEntry.self, from: data) {
            self.diaryEntry = saved
            self.diaryHistory = [saved]
        } else {
            self.diaryEntry = DailyDiaryEntry()
            self.diaryHistory = [self.diaryEntry]
        }

        if let data = defaults.data(forKey: diaryRemindersKey),
           let saved = try? JSONDecoder().decode([CareReminderSlot].self, from: data),
           !saved.isEmpty {
            self.diaryReminderSlots = saved
        } else {
            self.diaryReminderSlots = [
                CareReminderSlot(id: "diary_evening", clockTime: "20:00", remindersEnabled: true)
            ]
        }

        // Load medications (reset per-dose taken state when crossing a new day)
        if let data = defaults.data(forKey: medicationsKey),
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
        let savedStreak = defaults.integer(forKey: streakKey)
        self.streakDays = savedStreak > 0 ? savedStreak : 5

        // Load weekly history or populate standard 7-day observation points
        if let data = defaults.data(forKey: historyKey),
           let saved = try? JSONDecoder().decode([LongitudinalAttentionPoint].self, from: data) {
            self.weeklyHistory = saved
        } else {
            self.weeklyHistory = Self.defaultWeeklyHistory
        }

        // Milestones
        // Milestones (CDC / AAP catalog)
        if let data = defaults.data(forKey: milestonesKey),
           let saved = try? JSONDecoder().decode([Milestone].self, from: data),
           saved.count >= 20 {
            self.milestones = saved
        } else {
            self.milestones = Self.defaultMilestones
        }
    }

    private func saveTasks() {
        if let data = try? JSONEncoder().encode(tasks) {
            defaults.set(data, forKey: tasksKey)
        }
    }

    private func saveMilestones() {
        if let data = try? JSONEncoder().encode(milestones) {
            defaults.set(data, forKey: milestonesKey)
        }
    }

    private func persistDiary() {
        if let data = try? JSONEncoder().encode(diaryEntry) {
            defaults.set(data, forKey: diaryKey)
        }
        if let data = try? JSONEncoder().encode(diaryHistory) {
            defaults.set(data, forKey: diaryHistoryKey)
        }
    }

    private func saveDiaryReminders() {
        if let data = try? JSONEncoder().encode(diaryReminderSlots) {
            defaults.set(data, forKey: diaryRemindersKey)
        }
    }

    private func saveMedications() {
        if let data = try? JSONEncoder().encode(medications) {
            defaults.set(data, forKey: medicationsKey)
        }
    }

    private func saveHistory() {
        if let data = try? JSONEncoder().encode(weeklyHistory) {
            defaults.set(data, forKey: historyKey)
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

    private static var defaultMilestones: [Milestone] {
        ClinicalMilestoneCatalog.all(completedAges: [9, 12, 18])
    }
}
