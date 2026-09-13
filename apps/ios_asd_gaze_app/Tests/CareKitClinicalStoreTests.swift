import CareKitStore
import XCTest
@testable import AsdGazeCollector

final class CareKitClinicalStoreTests: XCTestCase {
    private func sampleTask() -> CarePlanTask {
        CarePlanTask(
            id: "task_gaze_screen",
            title: "Daily Social Gaze Screening",
            subtitle: "60-second TrueDepth observation",
            icon: "eyes",
            category: .observation
        )
    }

    // MARK: - Mapping

    func testTaskMappingIsStableAndDaily() {
        let startOfDay = Calendar.current.startOfDay(for: Date())
        let task = CareKitClinicalStore.makeTask(from: sampleTask(), startOfDay: startOfDay)
        XCTAssertEqual(task.id, "ock_task_gaze_screen")
        XCTAssertEqual(task.title, "Daily Social Gaze Screening")
        XCTAssertEqual(task.groupIdentifier, CareTaskCategory.observation.rawValue)
        XCTAssertTrue(task.impactsAdherence)
        XCTAssertNotNil(task.schedule)
        // Daily schedule must cover today.
        let tomorrow = Calendar.current.date(byAdding: .day, value: 1, to: startOfDay)!
        XCTAssertFalse(task.schedule.events(from: startOfDay, to: tomorrow).isEmpty)
    }

    func testScreeningOutcomeValues() {
        let values = CareKitClinicalStore.screeningOutcomeValues(
            socialScore: 0.65, validFramePercentage: 94.0, completedAt: Date()
        )
        XCTAssertEqual(values.count, 3)
    }

    func testTaskCompletionValues() {
        XCTAssertEqual(CareKitClinicalStore.taskCompletionValues(completed: true, completedAt: nil).count, 2)
    }

    func testDiaryTaskMapping() {
        let startOfDay = Calendar.current.startOfDay(for: Date())
        let task = CareKitClinicalStore.makeDiaryTask(startOfDay: startOfDay)
        XCTAssertEqual(task.id, CareKitClinicalStore.diaryTaskID)
        XCTAssertFalse(task.impactsAdherence)
        XCTAssertEqual(task.groupIdentifier, "diary")
        let tomorrow = Calendar.current.date(byAdding: .day, value: 1, to: startOfDay)!
        XCTAssertEqual(task.schedule.events(from: startOfDay, to: tomorrow).count, 1)
    }

    func testDiaryOutcomeValues() {
        let entry = DailyDiaryEntry(mood: .engaged, notes: "Great eye contact", eyeContactRating: 5, jointAttentionCount: 4)
        let values = CareKitClinicalStore.diaryOutcomeValues(from: entry)
        XCTAssertEqual(values.count, 4)
    }

    func testMedicationTaskDailyMultiDose() {
        let item = MedicationItem(
            id: "med_omega_3", name: "Omega-3 Support", dose: "500 mg",
            instructions: "With meals", schedule: ["08:00", "20:00"]
        )
        let startOfDay = Calendar.current.startOfDay(for: Date())
        let task = CareKitClinicalStore.makeMedicationTask(from: item, startOfDay: startOfDay)
        XCTAssertEqual(task.id, "ock_med_med_omega_3")
        XCTAssertEqual(task.groupIdentifier, "medication")
        XCTAssertTrue(task.impactsAdherence)
        let tomorrow = Calendar.current.date(byAdding: .day, value: 1, to: startOfDay)!
        let events = task.schedule.events(from: startOfDay, to: tomorrow)
        XCTAssertEqual(events.count, 2)
        XCTAssertEqual(events.map { Calendar.current.component(.hour, from: $0.start) }, [8, 20])
    }

    func testParseClockTime() {
        XCTAssertEqual(CareKitClinicalStore.parseClockTime("08:30")?.hour, 8)
        XCTAssertEqual(CareKitClinicalStore.parseClockTime("08:30")?.minute, 30)
        XCTAssertEqual(CareKitClinicalStore.parseClockTime("  08:30  ")?.hour, 8)
        XCTAssertEqual(CareKitClinicalStore.parseClockTime("  08:30  ")?.minute, 30)
        XCTAssertEqual(CareKitClinicalStore.parseClockTime(" 8:05 \n")?.hour, 8)
        XCTAssertEqual(CareKitClinicalStore.parseClockTime(" 8:05 \n")?.minute, 5)
        XCTAssertNil(CareKitClinicalStore.parseClockTime("25:00"))
        XCTAssertNil(CareKitClinicalStore.parseClockTime("8"))
    }

    func testMedicationOutcomeValues() {
        let item = MedicationItem(
            id: "med_vitamin_d", name: "Vitamin D3 Drops", dose: "400 IU",
            instructions: "Morning", schedule: ["08:00"]
        )
        XCTAssertEqual(CareKitClinicalStore.medicationOutcomeValues(item: item, markedAt: Date()).count, 2)
    }

    // MARK: - In-memory round trip (no disk, no UI)

    func testSeedAndOutcomeRoundTrip() throws {
        let store = OCKStore(name: "TestCurvAsdGaze", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        recorder.seedTasks([sampleTask()])
        // Seeding is async; poll briefly for the task instead of asserting blindly.
        var found = false
        for _ in 0..<40 {
            let exp = expectation(description: "fetch")
            var fetched: [OCKTask] = []
            store.fetchTasks(query: OCKTaskQuery(for: Date()), callbackQueue: .main) { result in
                fetched = (try? result.get()) ?? []
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if fetched.contains(where: { $0.id == "ock_task_gaze_screen" }) {
                found = true
                break
            }
        }
        XCTAssertTrue(found, "seeded task must be fetchable from the in-memory store")
    }

    func testSeedIncludesDiaryAndMedicationTasks() throws {
        let store = OCKStore(name: "TestDiaryMeds", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let med = MedicationItem(
            id: "med_vitamin_d", name: "Vitamin D3 Drops", dose: "400 IU",
            instructions: "Morning", schedule: ["08:00"]
        )
        recorder.seedTasks([sampleTask()], medications: [med])
        var found = false
        for _ in 0..<40 {
            let exp = expectation(description: "fetch all")
            var ids: [String] = []
            store.fetchTasks(callbackQueue: .main) { result in
                ids = (try? result.get().map(\.id)) ?? []
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if ids.contains("ock_caregiver_diary") && ids.contains("ock_med_med_vitamin_d") {
                found = true
                break
            }
        }
        XCTAssertTrue(found, "seeding must add the diary and medication tasks")
    }

    func testDiaryOutcomeRoundTrip() throws {
        let store = OCKStore(name: "TestDiaryOutcome", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        recorder.recordDiaryEntry(
            DailyDiaryEntry(mood: .engaged, notes: "Great eye contact", eyeContactRating: 5, jointAttentionCount: 4)
        )
        var found = false
        for _ in 0..<60 {
            let exp = expectation(description: "fetch diary outcome")
            var valueCount = 0
            store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
                let outcomes = (try? result.get()) ?? []
                valueCount = outcomes.reduce(0) { $0 + $1.values.count }
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if valueCount == 4 {
                found = true
                break
            }
        }
        XCTAssertTrue(found, "diary entry must land as a 4-value outcome")
    }

    func testRecordMedicationPrunesAndReTogglesSameDoseWithoutDuplicates() throws {
        let store = OCKStore(name: "TestMedOutcome", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        var med = MedicationItem(
            id: "med_vitamin_d", name: "Vitamin D3 Drops", dose: "400 IU",
            instructions: "Morning", schedule: ["08:00"]
        )
        med.dosesTakenToday = [true]
        recorder.recordMedicationTaken(med, doseIndex: 0, markedAt: Date())
        var count = 0
        for _ in 0..<60 {
            let exp = expectation(description: "fetch med outcome")
            store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
                count = (try? result.get().count) ?? 0
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if count == 1 { break }
        }
        XCTAssertEqual(count, 1)

        // Same dose toggled off: the outcome must be removed (no duplicate).
        med.dosesTakenToday = [false]
        recorder.recordMedicationTaken(med, doseIndex: 0, markedAt: Date())
        var cleared = -1
        for _ in 0..<60 {
            let exp = expectation(description: "fetch cleared med outcome")
            store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
                cleared = (try? result.get().count) ?? -1
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if cleared == 0 { break }
        }
        XCTAssertEqual(cleared, 0, "toggling a dose off must delete its outcome")
    }

    // MARK: - Audit-driven regression (cumulative occurrence, merge seeding, upserts)

    /// CareKit occurrences are cumulative since the schedule start — a dose
    /// recorded days after seeding must land on today's cumulative index,
    /// never on the per-day index (which would corrupt an older outcome).
    func testMedicationOutcomeUsesCumulativeOccurrence() throws {
        let store = OCKStore(name: "TestMedCumulative", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let threeDaysAgo = Calendar.current.date(byAdding: .day, value: -3, to: Date())!
        var med = MedicationItem(
            id: "med_omega_3", name: "Omega-3 Support", dose: "500 mg",
            instructions: "With meals", schedule: ["08:00", "20:00"]
        )
        recorder.seedTasks([], medications: [med], on: threeDaysAgo)
        med.dosesTakenToday = [false, true]
        recorder.recordMedicationTaken(med, doseIndex: 1, markedAt: Date())

        // Twice-daily for 3 full days + today's 08:00 dose = occurrence 7 for 20:00 today.
        var index = -1
        var count = 0
        for _ in 0..<60 {
            let exp = expectation(description: "fetch cumulative med outcome")
            store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
                let outcomes = (try? result.get()) ?? []
                count = outcomes.count
                index = outcomes.first?.taskOccurrenceIndex ?? -1
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if count == 1 { break }
        }
        XCTAssertEqual(count, 1)
        XCTAssertEqual(index, 7, "evening dose three days after seeding must use cumulative occurrence 7")
    }

    /// A record that arrives before any full seed must not lock the store into
    /// a partial task set: the later full seed still merges everything.
    func testPartialSeedThenFullSeedMergesAll() throws {
        let store = OCKStore(name: "TestPartialSeed", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let med = MedicationItem(
            id: "med_vitamin_d", name: "Vitamin D3 Drops", dose: "400 IU",
            instructions: "Morning", schedule: ["08:00"]
        )
        // Diary-only write first (seeds just the diary task).
        let diaryDone = expectation(description: "diary recorded")
        recorder.recordDiaryEntry(DailyDiaryEntry()) { diaryDone.fulfill() }
        wait(for: [diaryDone], timeout: 10.0)
        // Full launch seed afterwards must still add care-plan + med tasks.
        recorder.seedTasks([sampleTask()], medications: [med])
        var found = false
        for _ in 0..<60 {
            let exp = expectation(description: "fetch merged tasks")
            var ids: [String] = []
            store.fetchTasks(callbackQueue: .main) { result in
                ids = (try? result.get().map(\.id)) ?? []
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if ids.contains("ock_task_gaze_screen") && ids.contains("ock_caregiver_diary")
                && ids.contains("ock_med_med_vitamin_d") {
                found = true
                break
            }
        }
        XCTAssertTrue(found, "late full seed must merge all tasks even after a partial seed")
    }

    /// Two concurrent diary writes must serialize into a single outcome.
    func testConcurrentDiaryWritesKeepSingleOutcome() throws {
        let store = OCKStore(name: "TestConcurrentDiary", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let first = expectation(description: "first diary write")
        let second = expectation(description: "second diary write")
        DispatchQueue.global().async {
            recorder.recordDiaryEntry(DailyDiaryEntry(notes: "first")) { first.fulfill() }
        }
        DispatchQueue.global().async {
            recorder.recordDiaryEntry(DailyDiaryEntry(notes: "second")) { second.fulfill() }
        }
        wait(for: [first, second], timeout: 15.0)
        let exp = expectation(description: "fetch single diary outcome")
        var count = -1
        var valueCount = 0
        store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
            let outcomes = (try? result.get()) ?? []
            count = outcomes.count
            valueCount = outcomes.reduce(0) { $0 + $1.values.count }
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
        XCTAssertEqual(count, 1, "concurrent diary writes must upsert into one outcome")
        XCTAssertEqual(valueCount, 4)
    }

    /// Toggle on then off must keep exactly one outcome and reflect the
    /// latest state (completed=false), proving the update path.
    func testTaskToggleTwiceKeepsSingleOutcome() throws {
        let store = OCKStore(name: "TestToggleTwice", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        recorder.seedTasks([sampleTask()])
        var onTask = sampleTask()
        onTask.isCompleted = true
        onTask.completedAt = Date()
        let onDone = expectation(description: "toggle on recorded")
        recorder.recordTaskCompletion(onTask, knownTasks: [onTask]) { onDone.fulfill() }
        wait(for: [onDone], timeout: 10.0)
        var offTask = sampleTask()
        offTask.isCompleted = false
        offTask.completedAt = nil
        let offDone = expectation(description: "toggle off recorded")
        recorder.recordTaskCompletion(offTask, knownTasks: [offTask]) { offDone.fulfill() }
        wait(for: [offDone], timeout: 10.0)
        let exp = expectation(description: "fetch toggled outcome")
        var count = -1
        var completed = true
        store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
            let outcomes = (try? result.get()) ?? []
            count = outcomes.count
            completed = outcomes.first?.values.compactMap(\.booleanValue).first ?? true
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
        XCTAssertEqual(count, 1, "toggle on/off must not duplicate the occurrence outcome")
        XCTAssertFalse(completed, "latest toggle state (off) must win")
    }

    /// Daily care plan task completion 4 days after schedule start must land on
    /// cumulative occurrence index 4, not 0.
    func testDailyTaskCumulativeOccurrenceOverMultipleDays() throws {
        let store = OCKStore(name: "TestDailyCumulative", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let fourDaysAgo = Calendar.current.date(byAdding: .day, value: -4, to: Date())!
        let task = sampleTask()
        recorder.seedTasks([task], on: fourDaysAgo)

        var completedTask = task
        completedTask.isCompleted = true
        completedTask.completedAt = Date()
        let done = expectation(description: "task completion recorded")
        recorder.recordTaskCompletion(completedTask, knownTasks: [task]) { done.fulfill() }
        wait(for: [done], timeout: 10.0)

        let exp = expectation(description: "fetch cumulative task outcome")
        var occurrence = -1
        store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
            let outcomes = (try? result.get()) ?? []
            occurrence = outcomes.first?.taskOccurrenceIndex ?? -1
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
        XCTAssertEqual(occurrence, 4, "completion 4 days after seed must use cumulative occurrence 4")
    }

    /// Screening outcome with social score and valid frames must record correctly.
    func testRecordScreeningOutcomeRoundTrip() throws {
        let store = OCKStore(name: "TestScreeningOutcome", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let task = sampleTask()
        let done = expectation(description: "screening outcome recorded")
        recorder.recordScreeningOutcome(
            socialScore: 0.72,
            validFramePercentage: 96.0,
            knownTasks: [task]
        ) { done.fulfill() }
        wait(for: [done], timeout: 10.0)

        let exp = expectation(description: "fetch screening outcome")
        var valueCount = 0
        var doubles: [Double] = []
        store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
            let outcomes = (try? result.get()) ?? []
            let values = outcomes.first?.values ?? []
            valueCount = values.count
            doubles = values.compactMap(\.doubleValue)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
        XCTAssertEqual(valueCount, 3, "screening outcome must persist social score, frame %, and date")
        XCTAssertTrue(doubles.contains(where: { abs($0 - 0.72) < 1e-6 }), "outcome must contain social score 0.72")
        XCTAssertTrue(doubles.contains(where: { abs($0 - 96.0) < 1e-6 }), "outcome must contain valid frame % 96.0")
    }

    /// Concurrent diary, task completion, and medication writes must serialize safely.
    func testConcurrentTaskCompletionAndMedicationWrites() throws {
        let store = OCKStore(name: "TestConcurrentMulti", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let task = sampleTask()
        var med = MedicationItem(
            id: "med_d3", name: "Vitamin D3", dose: "400 IU",
            instructions: "Morning", schedule: ["08:00"]
        )
        med.dosesTakenToday = [true]

        recorder.seedTasks([task], medications: [med])

        let diaryDone = expectation(description: "diary done")
        let taskDone = expectation(description: "task done")
        let medDone = expectation(description: "med done")

        DispatchQueue.global().async {
            recorder.recordDiaryEntry(DailyDiaryEntry(notes: "concurrent notes")) { diaryDone.fulfill() }
        }
        DispatchQueue.global().async {
            var completed = task
            completed.isCompleted = true
            recorder.recordTaskCompletion(completed, knownTasks: [task]) { taskDone.fulfill() }
        }
        DispatchQueue.global().async {
            recorder.recordMedicationTaken(med, doseIndex: 0) { medDone.fulfill() }
        }

        wait(for: [diaryDone, taskDone, medDone], timeout: 15.0)

        let exp = expectation(description: "fetch all outcomes")
        var outcomeCount = 0
        store.fetchOutcomes(query: OCKOutcomeQuery(for: Date()), callbackQueue: .main) { result in
            let outcomes = (try? result.get()) ?? []
            outcomeCount = outcomes.count
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
        XCTAssertEqual(outcomeCount, 3, "diary, task, and med outcomes must all settle concurrently")
    }

    /// Seeding medications first and care plan tasks second merges all tasks properly.
    func testMedicationSeedThenCarePlanSeedMergesAll() throws {
        let store = OCKStore(name: "TestMedThenTaskSeed", type: .inMemory)
        let recorder = CareKitClinicalStore(store: store)
        let med = MedicationItem(
            id: "med_multi", name: "Multivitamin", dose: "1 chewable",
            instructions: "Morning", schedule: ["08:00"]
        )
        // 1. Partial seed with medication only
        recorder.seedTasks([], medications: [med])
        // 2. Full seed with task
        recorder.seedTasks([sampleTask()], medications: [med])

        var found = false
        for _ in 0..<40 {
            let exp = expectation(description: "fetch merged")
            var ids: [String] = []
            store.fetchTasks(callbackQueue: .main) { result in
                ids = (try? result.get().map(\.id)) ?? []
                exp.fulfill()
            }
            wait(for: [exp], timeout: 2.0)
            if ids.contains("ock_med_med_multi") && ids.contains("ock_task_gaze_screen") && ids.contains("ock_caregiver_diary") {
                found = true
                break
            }
        }
        XCTAssertTrue(found, "all tasks must merge when medication is seeded first")
    }
}
