import CareKitStore
import Foundation
import os

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
public final class CareKitClinicalStore: @unchecked Sendable {
    public static let shared = CareKitClinicalStore()

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "CareKitClinicalStore")

    /// Stable task id for the caregiver diary check-in.
    public static let diaryTaskID = "ock_caregiver_diary"

    private let store: OCKStore

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
        let trimmed = time.trimmingCharacters(in: .whitespacesAndNewlines)
        let parts = trimmed.split(separator: ":")
        guard parts.count == 2,
              let hour = Int(parts[0].trimmingCharacters(in: .whitespacesAndNewlines)),
              let minute = Int(parts[1].trimmingCharacters(in: .whitespacesAndNewlines)),
              (0...23).contains(hour), (0...59).contains(minute) else { return nil }
        return (hour, minute)
    }

    /// Outcome values for a completed screening session.
    public static func screeningOutcomeValues(
        socialScore: Double,
        validFramePercentage: Double,
        completedAt: Date,
        riskScore: Double? = nil,
        riskStratification: String? = nil
    ) -> [OCKOutcomeValue] {
        var values = [
            OCKOutcomeValue(socialScore),
            OCKOutcomeValue(validFramePercentage),
            OCKOutcomeValue(completedAt),
        ]
        if let risk = riskScore {
            values.append(OCKOutcomeValue(risk))
        }
        if let strat = riskStratification {
            values.append(OCKOutcomeValue(strat))
        }
        return values
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
            OCKOutcomeValue(entry.sensoryLoad),
        ]
    }

    /// Outcome values for an administered medication dose: [dose, givenAt].
    public static func medicationOutcomeValues(item: MedicationItem, markedAt: Date) -> [OCKOutcomeValue] {
        [OCKOutcomeValue(item.dose), OCKOutcomeValue(markedAt)]
    }

    // MARK: - Seeding

    /// Cumulative merge: every call inserts whatever ids are absent, so
    /// re-seeds (every launch, every record path) and partial-then-full
    /// sequences are all safe. Arguments are captured into the queued block,
    /// so concurrent callers can never observe each other's mutable state.
    /// Runs on `mutationQueue`; recording calls that arrive before seeding
    /// completes queue behind it (FIFO), so seed-then-write ordering holds.
    public func seedTasks(_ tasks: [CarePlanTask], medications: [MedicationItem] = [],
                          on date: Date = Date()) {
        mutationQueue.async { self.performSeed(tasks: tasks, medications: medications, on: date) }
    }

    // MARK: - Recording (fire-and-forget; never throws into UI)

    /// Records a daily-task toggle against today's occurrence.
    /// `completion` fires on the main actor after the outcome is written
    /// (successful or not), so the UI can clear its "syncing" indicator.
    public func recordTaskCompletion(_ task: CarePlanTask, knownTasks: [CarePlanTask],
                                     completion: (@Sendable () -> Void)? = nil) {
        mutationQueue.async {
            self.performSeed(tasks: knownTasks, on: Date())
            self.ensureTask(Self.makeTask(from: task, startOfDay: Calendar.current.startOfDay(for: Date())))
            let values = Self.taskCompletionValues(completed: task.isCompleted, completedAt: task.completedAt)
            self.upsertOutcome(taskID: "ock_\(task.id)", date: Date(), values: values)
            Self.dispatch(completion)
        }
    }

    /// Records a completed gaze screening session with its scores and ML risk stratification.
    /// `completion` fires on the main actor once the outcome is written.
    public func recordScreeningOutcome(
        socialScore: Double,
        validFramePercentage: Double,
        knownTasks: [CarePlanTask],
        riskScore: Double? = nil,
        riskStratification: String? = nil,
        date: Date = Date(),
        completion: (@Sendable () -> Void)? = nil
    ) {
        mutationQueue.async {
            self.performSeed(tasks: knownTasks, on: date)
            guard let observation = knownTasks.first(where: { $0.category == .observation }) else {
                Self.dispatch(completion)
                return
            }
            self.ensureTask(Self.makeTask(from: observation, startOfDay: Calendar.current.startOfDay(for: date)))
            let values = Self.screeningOutcomeValues(
                socialScore: socialScore,
                validFramePercentage: validFramePercentage,
                completedAt: date,
                riskScore: riskScore,
                riskStratification: riskStratification
            )
            self.upsertOutcome(taskID: "ock_\(observation.id)", date: date, values: values)
            Self.dispatch(completion)
        }
    }

    /// Records the caregiver diary entry against the diary task occurrence.
    /// `completion` fires on the main actor once the outcome is written.
    public func recordDiaryEntry(_ entry: DailyDiaryEntry, date: Date = Date(),
                                 completion: (@Sendable () -> Void)? = nil) {
        mutationQueue.async {
            self.performSeed(tasks: [], on: date)
            self.ensureTask(Self.makeDiaryTask(startOfDay: Calendar.current.startOfDay(for: date)))
            self.upsertOutcome(taskID: Self.diaryTaskID, date: date, values: Self.diaryOutcomeValues(from: entry))
            Self.dispatch(completion)
        }
    }

    /// Records (or clears, via `deleteOutcome`) one scheduled dose of a
    /// medication. The written occurrence is resolved from today's schedule
    /// events matched to the dose clock time — CareKit occurrences are
    /// cumulative since the schedule start, never per-day indexes.
    /// `completion` fires on the main actor once the write settles.
    public func recordMedicationTaken(_ item: MedicationItem, doseIndex: Int,
                                      markedAt: Date = Date(),
                                      completion: (@Sendable () -> Void)? = nil) {
        mutationQueue.async {
            self.performSeed(tasks: [], medications: [item], on: markedAt)
            guard item.dosesTakenToday.indices.contains(doseIndex),
                  item.schedule.indices.contains(doseIndex),
                  let doseTime = Self.parseClockTime(item.schedule[doseIndex]) else {
                Self.dispatch(completion)
                return
            }
            self.ensureTask(Self.makeMedicationTask(from: item, startOfDay: Calendar.current.startOfDay(for: markedAt)))
            let taskID = "ock_med_\(item.id)"
            if item.dosesTakenToday[doseIndex] {
                self.upsertOutcome(taskID: taskID, date: markedAt,
                                   values: Self.medicationOutcomeValues(item: item, markedAt: markedAt),
                                   doseTime: doseTime)
            } else {
                // Resolve the same occurrence the taken-path would use, then delete it.
                if let occurrence = self.occurrenceForDose(taskID: taskID, doseTime: doseTime, date: markedAt) {
                    self.deleteOutcomeIfPresent(taskID: taskID, occurrence: occurrence, date: markedAt)
                }
            }
            Self.dispatch(completion)
        }
    }

    public func replaceMedicationTask(from item: MedicationItem, date: Date = Date(),
                                      completion: (@Sendable () -> Void)? = nil) {
        mutationQueue.async {
            let taskID = "ock_med_\(item.id)"
            if let existing = self.fetchTasksNow().first(where: { $0.id == taskID }) {
                self.deleteTasksNow([existing])
            }
            self.addTasksNow([Self.makeMedicationTask(from: item, startOfDay: Calendar.current.startOfDay(for: date))])
            Self.dispatch(completion)
        }
    }

    public func deleteMedicationTask(id: String, completion: (@Sendable () -> Void)? = nil) {
        mutationQueue.async {
            let taskID = "ock_med_\(id)"
            if let existing = self.fetchTasksNow().first(where: { $0.id == taskID }) {
                self.deleteTasksNow([existing])
            }
            Self.dispatch(completion)
        }
    }

    // MARK: - Private serialized plumbing

    /// Runs `completion` on the main actor, if provided.
    private static func dispatch(_ completion: (@Sendable () -> Void)?) {
        guard let completion else { return }
        DispatchQueue.main.async(execute: completion)
    }

    /// Serial queue that owns every read/write below. Each public call enqueues
    /// one unit of work, so mutations never interleave and seeding always
    /// precedes the outcome writes it should back.
    private let mutationQueue = DispatchQueue(label: "org.curv.carekit.mutations", qos: .userInitiated)

    private func performSeed(tasks: [CarePlanTask], medications: [MedicationItem] = [], on date: Date) {
        let startOfDay = Calendar.current.startOfDay(for: date)
        var desired = tasks.map { Self.makeTask(from: $0, startOfDay: startOfDay) }
        desired.append(Self.makeDiaryTask(startOfDay: startOfDay))
        desired.append(contentsOf: medications.map { Self.makeMedicationTask(from: $0, startOfDay: startOfDay) })
        let existingIDs = Set(fetchTasksNow().map(\.id))
        let missing = desired.filter { !existingIDs.contains($0.id) }
        guard !missing.isEmpty else { return }
        addTasksNow(missing)
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
    /// When `doseTime` is given, the occurrence is resolved by matching today's
    /// schedule event closest to that clock time — required because CareKit
    /// occurrence indexes are cumulative since the schedule start, not per-day.
    private func upsertOutcome(taskID: String, date: Date, values: [OCKOutcomeValue],
                               doseTime: (hour: Int, minute: Int)? = nil) {
        guard let task = fetchTasksNow().first(where: { $0.id == taskID }) else {
            Self.logger.warning("no task \(taskID, privacy: .public) for outcome")
            return
        }
        let calendar = Calendar.current
        let startOfDay = calendar.startOfDay(for: date)
        guard let endOfDay = calendar.date(byAdding: .day, value: 1, to: startOfDay) else { return }
        let events = task.schedule.events(from: startOfDay, to: endOfDay)
        let occurrence: Int
        if let doseTime,
           let doseStart = calendar.date(bySettingHour: doseTime.hour, minute: doseTime.minute, second: 0, of: startOfDay),
           let event = events.min(by: { abs($0.start.timeIntervalSince(doseStart)) < abs($1.start.timeIntervalSince(doseStart)) }) {
            occurrence = event.occurrence
        } else if doseTime != nil {
            Self.logger.warning("no event matching dose time for \(taskID, privacy: .public)")
            return
        } else if let first = events.first {
            occurrence = first.occurrence
        } else {
            Self.logger.warning("no occurrence for \(taskID, privacy: .public) today")
            return
        }
        if let existing = fetchOutcomeNow(task: task, occurrence: occurrence, startOfDay: startOfDay, endOfDay: endOfDay) {
            var updated = existing
            updated.values = values
            updateOutcomeNow(updated)
        } else {
            addOutcomeNow(OCKOutcome(taskUUID: task.uuid, taskOccurrenceIndex: occurrence, values: values))
        }
    }

    /// Resolve the cumulative occurrence index of the schedule event closest
    /// to `doseTime` on `date`, or nil when the task/event is absent.
    private func occurrenceForDose(taskID: String, doseTime: (hour: Int, minute: Int), date: Date) -> Int? {
        guard let task = fetchTasksNow().first(where: { $0.id == taskID }) else { return nil }
        let calendar = Calendar.current
        let startOfDay = calendar.startOfDay(for: date)
        guard let endOfDay = calendar.date(byAdding: .day, value: 1, to: startOfDay),
              let doseStart = calendar.date(bySettingHour: doseTime.hour, minute: doseTime.minute, second: 0, of: startOfDay) else { return nil }
        let events = task.schedule.events(from: startOfDay, to: endOfDay)
        return events.min(by: { abs($0.start.timeIntervalSince(doseStart)) < abs($1.start.timeIntervalSince(doseStart)) })?.occurrence
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
                Self.logger.error("deleteOutcome failed: \(error.localizedDescription, privacy: .public)")
            }
            semaphore.signal()
        }
        semaphore.wait()
    }

    // MARK: - Synchronous store shims (safe: called only on `mutationQueue`)

    private final class Box<T>: @unchecked Sendable {
        var value: T
        init(_ value: T) { self.value = value }
    }

    /// Block until CareKit answers, then return the result. The caller must be
    /// on `mutationQueue` (never the main thread) so the main-queue callbacks
    /// below cannot deadlock.
    private func fetchTasksNow() -> [OCKTask] {
        let box = Box<[OCKTask]>([])
        let semaphore = DispatchSemaphore(value: 0)
        store.fetchTasks(callbackQueue: .main) { outcome in
            switch outcome {
            case .success(let tasks):
                box.value = tasks
            case .failure(let error):
                Self.logger.error("fetchTasks failed: \(error.localizedDescription, privacy: .public)")
            }
            semaphore.signal()
        }
        semaphore.wait()
        return box.value
    }

    @discardableResult
    private func addTasksNow(_ tasks: [OCKTask]) -> Bool {
        let box = Box(false)
        let semaphore = DispatchSemaphore(value: 0)
        store.addTasks(tasks, callbackQueue: .main) { outcome in
            if case .failure(let error) = outcome {
                Self.logger.error("addTasks failed: \(error.localizedDescription, privacy: .public)")
            } else {
                box.value = true
            }
            semaphore.signal()
        }
        semaphore.wait()
        return box.value
    }

    @discardableResult
    private func deleteTasksNow(_ tasks: [OCKTask]) -> Bool {
        let box = Box(false)
        let semaphore = DispatchSemaphore(value: 0)
        store.deleteTasks(tasks, callbackQueue: .main) { outcome in
            if case .failure(let error) = outcome {
                Self.logger.error("deleteTasks failed: \(error.localizedDescription, privacy: .public)")
            } else {
                box.value = true
            }
            semaphore.signal()
        }
        semaphore.wait()
        return box.value
    }

    private func fetchOutcomeNow(task: OCKTask, occurrence: Int,
                                 startOfDay: Date, endOfDay: Date) -> OCKOutcome? {
        let box = Box<OCKOutcome?>(nil)
        let semaphore = DispatchSemaphore(value: 0)
        var query = OCKOutcomeQuery(dateInterval: DateInterval(start: startOfDay, end: endOfDay))
        query.taskIDs = [task.id]
        store.fetchOutcomes(query: query, callbackQueue: .main) { outcome in
            switch outcome {
            case .success(let outcomes):
                box.value = outcomes.first(where: { $0.taskUUID == task.uuid && $0.taskOccurrenceIndex == occurrence })
            case .failure(let error):
                Self.logger.error("fetchOutcomes failed: \(error.localizedDescription, privacy: .public)")
            }
            semaphore.signal()
        }
        semaphore.wait()
        return box.value
    }

    private func addOutcomeNow(_ outcome: OCKOutcome) {
        let semaphore = DispatchSemaphore(value: 0)
        store.addOutcome(outcome, callbackQueue: .main) { outcomeResult in
            if case .failure(let error) = outcomeResult {
                Self.logger.error("addOutcome failed: \(error.localizedDescription, privacy: .public)")
            }
            semaphore.signal()
        }
        semaphore.wait()
    }

    private func updateOutcomeNow(_ outcome: OCKOutcome) {
        let semaphore = DispatchSemaphore(value: 0)
        store.updateOutcomes([outcome], callbackQueue: .main) { outcomeResult in
            if case .failure(let error) = outcomeResult {
                Self.logger.error("updateOutcome failed: \(error.localizedDescription, privacy: .public)")
            }
            semaphore.signal()
        }
        semaphore.wait()
    }
}
