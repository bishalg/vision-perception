import CareKitStore
import Foundation

/// On-device clinical record for the ASD gaze program, backed by Apple's
/// CareKit `OCKStore`. This is the real SDK integration behind the
/// CareKit-compatible JSON export (`CarePlanStore.exportClinicalCareKitJSON`):
/// local `CarePlanStore` remains the UI source of truth (UserDefaults,
/// synchronous), while every completion also lands in the queryable,
/// pediatrician-shareable OCK record.
///
/// Privacy: on-disk Core Data store, no network sync configured
/// (`remote: nil`), no raw video/frames ever written — consistent with
/// `domains/asd_screening/docs/ETHICS_PROTOCOL.md` (zero raw video storage).
/// All writes are fire-and-forget with error logging; a store failure never
/// blocks or alters UI state.
public final class CareKitClinicalStore {
    public static let shared = CareKitClinicalStore()

    /// Stable task id for the caregiver diary check-in.
    public static let diaryTaskID = "ock_caregiver_diary"

    private let store: OCKStore
    private var hasSeeded = false
    private var knownTasks: [CarePlanTask] = []
    private var knownMedications: [MedicationItem] = []

    /// Production initializer (on-disk store). Tests inject `.inMemory()`.
    public convenience init() {
        self.init(store: OCKStore(name: "CurvAsdGaze", type: .onDisk()))
    }

    public init(store: OCKStore) {
        self.store = store
    }

    // MARK: - Mapping (pure, unit-tested)

    /// Daily all-day task mirroring one `CarePlanTask`. Stable id mapping:
    /// `ock_<carePlanTask.id>` so re-seeds never duplicate.
    public static func makeTask(from task: CarePlanTask, startOfDay: Date) -> OCKTask {
        var ockTask = OCKTask(
            id: "ock_\(task.id)",
            title: task.title,
            carePlanUUID: nil,
            schedule: OCKSchedule.dailyAtTime(
                hour: 9, minutes: 0,
                start: startOfDay, end: nil,
                text: "Daily"
            )
        )
        ockTask.instructions = task.subtitle
        ockTask.impactsAdherence = true
        ockTask.groupIdentifier = task.category.rawValue
        return ockTask
    }

    /// Daily evening caregiver diary check-in task (`ock_caregiver_diary`).
    public static func makeDiaryTask(startOfDay: Date, hour: Int = 20, minutes: Int = 0) -> OCKTask {
        var task = OCKTask(
            id: diaryTaskID,
            title: "Daily Caregiver Diary",
            carePlanUUID: nil,
            schedule: OCKSchedule.dailyAtTime(
                hour: hour, minutes: minutes,
                start: startOfDay, end: nil,
                text: "Evening check-in"
            )
        )
        task.instructions = "Mood, spontaneous eye contact rating, joint attention count, and notes."
        task.impactsAdherence = false
        task.groupIdentifier = "diary"
        return task
    }

    /// Daily multi-dose medication task (`ock_med_<item.id>`), one daily
    /// schedule element per dose time. Occurrence order matches `schedule`.
    public static func makeMedicationTask(from item: MedicationItem, startOfDay: Date) -> OCKTask {
        let elements: [OCKScheduleElement] = item.schedule.compactMap { time in
            guard let (hour, minute) = parseClockTime(time),
                  let start = Calendar.current.date(bySettingHour: hour, minute: minute, second: 0, of: startOfDay) else { return nil }
            return OCKScheduleElement(
                start: start, end: nil,
                interval: DateComponents(day: 1),
                text: "Dose",
                duration: .hours(1)
            )
        }
        var task = OCKTask(
            id: "ock_med_\(item.id)",
            title: item.name,
            carePlanUUID: nil,
            schedule: OCKSchedule(composing: elements)
        )
        task.instructions = "\(item.dose) — \(item.instructions)"
        task.impactsAdherence = true
        task.groupIdentifier = "medication"
        return task
    }

    /// Parse a "HH:MM" clock string into hour/minute, or nil.
    public static func parseClockTime(_ time: String) -> (hour: Int, minute: Int)? {
        let parts = time.split(separator: ":")
        guard parts.count == 2,
              let hour = Int(parts[0]), let minute = Int(parts[1]),
              (0...23).contains(hour), (0...59).contains(minute) else { return nil }
        return (hour, minute)
    }

    /// Outcome values for a completed screening session.
    public static func screeningOutcomeValues(socialScore: Double, validFramePercentage: Double,
                                              completedAt: Date) -> [OCKOutcomeValue] {
        [
            OCKOutcomeValue(socialScore),
            OCKOutcomeValue(validFramePercentage),
            OCKOutcomeValue(completedAt),
        ]
    }

    /// Outcome values for a toggled daily task.
    public static func taskCompletionValues(completed: Bool, completedAt: Date?) -> [OCKOutcomeValue] {
        [OCKOutcomeValue(completed), OCKOutcomeValue(completedAt ?? Date())]
    }

    /// Outcome values for a diary entry: [mood, eye contact rating,
    /// joint attention count, notes].
    public static func diaryOutcomeValues(from entry: DailyDiaryEntry) -> [OCKOutcomeValue] {
        [
            OCKOutcomeValue(entry.mood.rawValue),
            OCKOutcomeValue(entry.eyeContactRating),
            OCKOutcomeValue(entry.jointAttentionCount),
            OCKOutcomeValue(entry.notes),
        ]
    }

    /// Outcome values for an administered medication dose: [dose, givenAt].
    public static func medicationOutcomeValues(item: MedicationItem, markedAt: Date) -> [OCKOutcomeValue] {
        [OCKOutcomeValue(item.dose), OCKOutcomeValue(markedAt)]
    }

    // MARK: - Seeding

    /// Idempotent: only tasks whose ids are absent are inserted, so re-seeds
    /// (e.g. every launch) are safe. Seeds the care-plan tasks plus the
    /// caregiver diary check-in and one multi-dose task per medication.
    /// Runs on `mutationQueue`; recording calls that arrive before seeding
    /// completes queue behind it (FIFO), so seed-then-write ordering holds.
    public func seedTasks(_ tasks: [CarePlanTask], medications: [MedicationItem] = [],
                          on date: Date = Date()) {
        knownTasks = tasks
        knownMedications = medications
        mutationQueue.async { self.performSeed(on: date) }
    }

    // MARK: - Recording (fire-and-forget; never throws into UI)

    /// Records a daily-task toggle against today's occurrence.
    public func recordTaskCompletion(_ task: CarePlanTask, knownTasks: [CarePlanTask]) {
        mutationQueue.async {
            self.performSeed(on: Date())
            let values = Self.taskCompletionValues(completed: task.isCompleted, completedAt: task.completedAt)
            self.upsertOutcome(taskID: "ock_\(task.id)", date: Date(), values: values)
        }
    }

    /// Records a completed gaze screening session with its scores.
    public func recordScreeningOutcome(socialScore: Double, validFramePercentage: Double,
                                       knownTasks: [CarePlanTask], date: Date = Date()) {
        mutationQueue.async {
            self.performSeed(on: date)
            guard let observation = knownTasks.first(where: { $0.category == .observation }) else { return }
            let values = Self.screeningOutcomeValues(socialScore: socialScore,
                                                     validFramePercentage: validFramePercentage,
                                                     completedAt: date)
            self.upsertOutcome(taskID: "ock_\(observation.id)", date: date, values: values)
        }
    }

    /// Records the caregiver diary entry against the diary task occurrence.
    public func recordDiaryEntry(_ entry: DailyDiaryEntry, date: Date = Date()) {
        mutationQueue.async {
            self.performSeed(on: date)
            self.ensureTask(Self.makeDiaryTask(startOfDay: Calendar.current.startOfDay(for: date)))
            self.upsertOutcome(taskID: Self.diaryTaskID, date: date, values: Self.diaryOutcomeValues(from: entry))
        }
    }

    /// Records (or clears, via `deleteOutcome`) one scheduled dose of a
    /// medication. The CareKit occurrence for a composed multi-dose schedule
    /// equals the dose's index into `item.schedule`.
    public func recordMedicationTaken(_ item: MedicationItem, doseIndex: Int,
                                      markedAt: Date = Date()) {
        mutationQueue.async {
            self.performSeed(on: markedAt)
            guard item.dosesTakenToday.indices.contains(doseIndex),
                  item.schedule.indices.contains(doseIndex),
                  Self.parseClockTime(item.schedule[doseIndex]) != nil else { return }
            self.ensureTask(Self.makeMedicationTask(from: item, startOfDay: Calendar.current.startOfDay(for: markedAt)))
            let taskID = "ock_med_\(item.id)"
            if item.dosesTakenToday[doseIndex] {
                self.upsertOutcome(taskID: taskID, date: markedAt,
                                   values: Self.medicationOutcomeValues(item: item, markedAt: markedAt),
                                   occurrenceHint: doseIndex)
            } else {
                self.deleteOutcomeIfPresent(taskID: taskID, occurrence: doseIndex, date: markedAt)
            }
        }
    }

    // MARK: - Private serialized plumbing

    /// Serial queue that owns every read/write below. Each public call enqueues
    /// one unit of work, so mutations never interleave and seeding always
    /// precedes the outcome writes it should back.
    private let mutationQueue = DispatchQueue(label: "org.curv.carekit.mutations", qos: .userInitiated)

    private func performSeed(on date: Date) {
        guard !hasSeeded else { return }
        let startOfDay = Calendar.current.startOfDay(for: date)
        var desired = knownTasks.map { Self.makeTask(from: $0, startOfDay: startOfDay) }
        desired.append(Self.makeDiaryTask(startOfDay: startOfDay))
        desired.append(contentsOf: knownMedications.map { Self.makeMedicationTask(from: $0, startOfDay: startOfDay) })
        let existingIDs = Set(fetchTasksNow().map(\.id))
        let missing = desired.filter { !existingIDs.contains($0.id) }
        if missing.isEmpty {
            hasSeeded = true
            return
        }
        guard addTasksNow(missing) else { return }
        hasSeeded = true
    }

    /// Insert `task` only if its id is absent. Lets a record path back its own
    /// task even when launching mid-flight (before the seed has run).
    private func ensureTask(_ task: OCKTask) {
        let existingIDs = Set(fetchTasksNow().map(\.id))
        guard !existingIDs.contains(task.id) else { return }
        addTasksNow([task])
    }

    /// Upsert today's outcome for `taskID` (update if one already exists for
    /// the occurrence, otherwise insert) so each occurrence holds one outcome.
    private func upsertOutcome(taskID: String, date: Date, values: [OCKOutcomeValue],
                               occurrenceHint: Int? = nil) {
        guard let task = fetchTasksNow().first(where: { $0.id == taskID }) else {
            print("[CareKitClinicalStore] no task \(taskID) for outcome")
            return
        }
        let calendar = Calendar.current
        let startOfDay = calendar.startOfDay(for: date)
        guard let endOfDay = calendar.date(byAdding: .day, value: 1, to: startOfDay) else { return }
        let occurrence: Int
        if let hint = occurrenceHint {
            occurrence = hint
        } else {
            guard let first = task.schedule.events(from: startOfDay, to: endOfDay).first else {
                print("[CareKitClinicalStore] no occurrence for \(taskID) today")
                return
            }
            occurrence = first.occurrence
        }
        if let existing = fetchOutcomeNow(task: task, occurrence: occurrence, startOfDay: startOfDay, endOfDay: endOfDay) {
            var updated = existing
            updated.values = values
            updateOutcomeNow(updated)
        } else {
            addOutcomeNow(OCKOutcome(taskUUID: task.uuid, taskOccurrenceIndex: occurrence, values: values))
        }
    }

    /// Remove any recorded outcome for the given task occurrence (toggle-off).
    private func deleteOutcomeIfPresent(taskID: String, occurrence: Int, date: Date) {
        guard let task = fetchTasksNow().first(where: { $0.id == taskID }) else { return }
        let calendar = Calendar.current
        let startOfDay = calendar.startOfDay(for: date)
        guard let endOfDay = calendar.date(byAdding: .day, value: 1, to: startOfDay) else { return }
        guard let existing = fetchOutcomeNow(task: task, occurrence: occurrence, startOfDay: startOfDay, endOfDay: endOfDay) else { return }
        let semaphore = DispatchSemaphore(value: 0)
        store.deleteOutcomes([existing], callbackQueue: .main) { result in
            if case .failure(let error) = result {
                print("[CareKitClinicalStore] deleteOutcome failed: \(error)")
            }
            semaphore.signal()
        }
        semaphore.wait()
    }

    // MARK: - Synchronous store shims (safe: called only on `mutationQueue`)

    /// Block until CareKit answers, then return the result. The caller must be
    /// on `mutationQueue` (never the main thread) so the main-queue callbacks
    /// below cannot deadlock.
    private func fetchTasksNow() -> [OCKTask] {
        var result: [OCKTask] = []
        let semaphore = DispatchSemaphore(value: 0)
        store.fetchTasks(callbackQueue: .main) { outcome in
            result = (try? outcome.get()) ?? []
            semaphore.signal()
        }
        semaphore.wait()
        return result
    }

    @discardableResult
    private func addTasksNow(_ tasks: [OCKTask]) -> Bool {
        var ok = false
        let semaphore = DispatchSemaphore(value: 0)
        store.addTasks(tasks, callbackQueue: .main) { outcome in
            if case .failure(let error) = outcome {
                print("[CareKitClinicalStore] addTasks failed: \(error)")
            } else {
                ok = true
            }
            semaphore.signal()
        }
        semaphore.wait()
        return ok
    }

    private func fetchOutcomeNow(task: OCKTask, occurrence: Int,
                                 startOfDay: Date, endOfDay: Date) -> OCKOutcome? {
        var result: OCKOutcome?
        let semaphore = DispatchSemaphore(value: 0)
        var query = OCKOutcomeQuery(dateInterval: DateInterval(start: startOfDay, end: endOfDay))
        query.taskIDs = [task.id]
        store.fetchOutcomes(query: query, callbackQueue: .main) { outcome in
            let outcomes = (try? outcome.get()) ?? []
            result = outcomes.first(where: { $0.taskUUID == task.uuid && $0.taskOccurrenceIndex == occurrence })
            semaphore.signal()
        }
        semaphore.wait()
        return result
    }

    private func addOutcomeNow(_ outcome: OCKOutcome) {
        let semaphore = DispatchSemaphore(value: 0)
        store.addOutcome(outcome, callbackQueue: .main) { outcomeResult in
            if case .failure(let error) = outcomeResult {
                print("[CareKitClinicalStore] addOutcome failed: \(error)")
            }
            semaphore.signal()
        }
        semaphore.wait()
    }

    private func updateOutcomeNow(_ outcome: OCKOutcome) {
        let semaphore = DispatchSemaphore(value: 0)
        store.updateOutcomes([outcome], callbackQueue: .main) { outcomeResult in
            if case .failure(let error) = outcomeResult {
                print("[CareKitClinicalStore] updateOutcome failed: \(error)")
            }
            semaphore.signal()
        }
        semaphore.wait()
    }
}
