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
}
