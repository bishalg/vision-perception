import CareKitStore
import CurvAsdGazeKit
import XCTest
@testable import AsdGazeCollector

final class CarePlanStoreTests: XCTestCase {
    /// Hermetic store: mirrors land in an in-memory OCKStore, never the
    /// on-disk clinical record.
    @MainActor
    private func makeStore() -> CarePlanStore {
        CarePlanStore(clinicalStore: CareKitClinicalStore(
            store: OCKStore(name: "TestCarePlan-\(UUID().uuidString)", type: .inMemory)
        ))
    }

    @MainActor
    func testTaskCompletionAndRatioCalculation() {
        let store = makeStore()
        let initialCount = store.tasks.count
        XCTAssertGreaterThan(initialCount, 0)

        // Find an uncompleted task or toggle one
        if let uncompleted = store.tasks.first(where: { !$0.isCompleted }) {
            let initialCompleted = store.completedTaskCount
            store.toggleTask(id: uncompleted.id)
            XCTAssertEqual(store.completedTaskCount, initialCompleted + 1)
            XCTAssertEqual(store.completionRatio, Double(initialCompleted + 1) / Double(store.tasks.count))

            // Toggle back
            store.toggleTask(id: uncompleted.id)
            XCTAssertEqual(store.completedTaskCount, initialCompleted)
        }
    }

    @MainActor
    func testToggleDrainsPendingCounter() {
        let store = makeStore()
        guard let target = store.tasks.first else { return }
        let initialCompleted = target.isCompleted
        store.toggleTask(id: target.id)
        store.toggleTask(id: target.id)
        // Rapid double-toggle restores state; both writes must settle and the
        // counter must drain back to empty (no stuck spinner, no early clear).
        XCTAssertEqual(store.tasks.first(where: { $0.id == target.id })?.isCompleted, initialCompleted)
        var drained = false
        for _ in 0..<60 {
            RunLoop.main.run(until: Date().addingTimeInterval(0.1))
            if store.pendingCareKitCounts.isEmpty {
                drained = true
                break
            }
        }
        XCTAssertTrue(drained, "pending CareKit writes must settle and clear the counter")
    }

    @MainActor
    func testMoodAndNotesUpdates() {
        let store = makeStore()

        store.updateMood(.engaged)
        XCTAssertEqual(store.diaryEntry.mood, .engaged)

        store.updateNotes("Observed high interest in social video.")
        XCTAssertEqual(store.diaryEntry.notes, "Observed high interest in social video.")

        store.updateEyeContactRating(5)
        XCTAssertEqual(store.diaryEntry.eyeContactRating, 5)

        // Test clamping
        store.updateEyeContactRating(10)
        XCTAssertEqual(store.diaryEntry.eyeContactRating, 5)

        store.updateEyeContactRating(-2)
        XCTAssertEqual(store.diaryEntry.eyeContactRating, 1)
    }

    @MainActor
    func testMarkScreeningCompleted() {
        let store = makeStore()
        let initialSessions = store.weeklyHistory.last?.sessionCount ?? 0

        store.markScreeningCompleted(captureSource: .trueDepth, socialScore: 0.65)

        // Observation task should be completed
        let observationTask = store.tasks.first(where: { $0.category == .observation })
        XCTAssertNotNil(observationTask)
        XCTAssertTrue(observationTask?.isCompleted == true)

        // Latest weekly history session count should increment
        let updatedSessions = store.weeklyHistory.last?.sessionCount ?? 0
        XCTAssertEqual(updatedSessions, initialSessions + 1)
    }

    /// SIM-5: a simulated session changes no care-plan state and writes nothing to CareKit.
    @MainActor
    func testMarkScreeningCompletedIgnoresSimulatedSession() {
        let suite = "CarePlanStoreTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let ockStore = OCKStore(name: "TestCarePlanSim-\(UUID().uuidString)", type: .inMemory)
        let store = CarePlanStore(clinicalStore: CareKitClinicalStore(store: ockStore), defaults: defaults)
        if let idx = store.tasks.firstIndex(where: { $0.category == .observation }) {
            store.tasks[idx].isCompleted = false
        }
        let historyBefore = store.weeklyHistory.map(\.sessionCount)

        store.markScreeningCompleted(
            captureSource: .simulated,
            socialScore: 0.9,
            summary: .fixture(captureSource: .simulated)
        )

        XCTAssertEqual(store.tasks.first(where: { $0.category == .observation })?.isCompleted, false)
        XCTAssertEqual(store.weeklyHistory.map(\.sessionCount), historyBefore)
        XCTAssertTrue(store.pendingCareKitCounts.isEmpty, "no CareKit write may be started")
    }

    /// Delete-all removes every persisted care-plan key, the in-memory state and the CareKit mirror.
    @MainActor
    func testDeleteAllCarePlanDataClearsStateKeysAndCareKitMirror() {
        let suite = "CarePlanStoreTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let ockStore = OCKStore(name: "TestCarePlanDelete-\(UUID().uuidString)", type: .inMemory)
        let store = CarePlanStore(clinicalStore: CareKitClinicalStore(store: ockStore), defaults: defaults)
        store.updateNotes("Private caregiver note")
        if let first = store.tasks.first { store.toggleTask(id: first.id) }
        XCTAssertFalse(defaults.dictionaryRepresentation().keys.filter { $0.hasPrefix("org.curv.asd.careplan.") }.isEmpty)

        let done = expectation(description: "care plan data deleted")
        store.deleteAllCarePlanData { done.fulfill() }
        wait(for: [done], timeout: 10.0)

        XCTAssertTrue(store.tasks.isEmpty)
        XCTAssertTrue(store.medications.isEmpty)
        XCTAssertTrue(store.diaryHistory.isEmpty)
        XCTAssertTrue(store.diaryReminderSlots.isEmpty)
        XCTAssertTrue(store.milestones.isEmpty)
        XCTAssertTrue(store.weeklyHistory.isEmpty)
        XCTAssertEqual(store.diaryEntry.notes, "")
        XCTAssertTrue(
            defaults.dictionaryRepresentation().keys.filter { $0.hasPrefix("org.curv.asd.careplan.") }.isEmpty,
            "no care-plan key may survive deletion"
        )

        let fetched = expectation(description: "fetch tasks")
        var remaining: [OCKTask] = []
        var query = OCKTaskQuery(dateInterval: DateInterval(start: .distantPast, end: Date().addingTimeInterval(86_400 * 365)))
        query.excludesTasksWithNoEvents = false
        ockStore.fetchTasks(query: query, callbackQueue: .main) { result in
            remaining = (try? result.get()) ?? []
            fetched.fulfill()
        }
        wait(for: [fetched], timeout: 5.0)
        XCTAssertTrue(remaining.isEmpty, "the CareKit mirror must be emptied too")
    }

    @MainActor
    func testMedicationToggleAndRatio() {
        let store = makeStore()
        XCTAssertFalse(store.medications.isEmpty)
        guard let med = store.medications.first else { return }
        let ratioBefore = store.medicationCompletionRatio
        store.toggleMedicationTaken(id: med.id, doseIndex: 0)
        XCTAssertTrue(store.medications.first(where: { $0.id == med.id })?.dosesTakenToday[0] == true)
        XCTAssertGreaterThan(store.medicationCompletionRatio, ratioBefore)
        // Restore state for hermetic reruns.
        store.toggleMedicationTaken(id: med.id, doseIndex: 0)
        XCTAssertEqual(store.medicationCompletionRatio, ratioBefore, accuracy: 1e-9)
    }

    @MainActor
    func testMedicationPartialTogglePreservesLastTakenAt() {
        let store = makeStore()
        guard let multiDoseMed = store.medications.first(where: { $0.schedule.count >= 2 }) else {
            XCTFail("Must have at least one multi-dose medication")
            return
        }

        // Initially ensure both are false
        if multiDoseMed.dosesTakenToday[0] {
            store.toggleMedicationTaken(id: multiDoseMed.id, doseIndex: 0)
        }
        if multiDoseMed.dosesTakenToday[1] {
            store.toggleMedicationTaken(id: multiDoseMed.id, doseIndex: 1)
        }

        // 1. Take dose 0
        store.toggleMedicationTaken(id: multiDoseMed.id, doseIndex: 0)
        let medAfterDose0 = store.medications.first(where: { $0.id == multiDoseMed.id })!
        XCTAssertTrue(medAfterDose0.dosesTakenToday[0])
        XCTAssertNotNil(medAfterDose0.lastTakenAt)
        let dose0Time = medAfterDose0.lastTakenAt

        // 2. Take dose 1
        store.toggleMedicationTaken(id: multiDoseMed.id, doseIndex: 1)
        let medAfterBoth = store.medications.first(where: { $0.id == multiDoseMed.id })!
        XCTAssertTrue(medAfterBoth.dosesTakenToday[0])
        XCTAssertTrue(medAfterBoth.dosesTakenToday[1])
        XCTAssertNotNil(medAfterBoth.lastTakenAt)

        // 3. Untoggle dose 1 (dose 0 remains taken) -> lastTakenAt must NOT be wiped to nil!
        store.toggleMedicationTaken(id: multiDoseMed.id, doseIndex: 1)
        let medAfterUntoggle1 = store.medications.first(where: { $0.id == multiDoseMed.id })!
        XCTAssertTrue(medAfterUntoggle1.dosesTakenToday[0])
        XCTAssertFalse(medAfterUntoggle1.dosesTakenToday[1])
        XCTAssertNotNil(medAfterUntoggle1.lastTakenAt, "lastTakenAt must NOT be cleared while dose 0 remains active")

        // 4. Untoggle dose 0 (now no doses taken) -> lastTakenAt should be nil
        store.toggleMedicationTaken(id: multiDoseMed.id, doseIndex: 0)
        let medAfterBothUntoggled = store.medications.first(where: { $0.id == multiDoseMed.id })!
        XCTAssertFalse(medAfterBothUntoggled.dosesTakenToday[0])
        XCTAssertFalse(medAfterBothUntoggled.dosesTakenToday[1])
        XCTAssertNil(medAfterBothUntoggled.lastTakenAt, "lastTakenAt must be nil when all doses are untoggled")
    }

    @MainActor
    func testMedicationDecodeNormalizesDoseArray() throws {
        let json = """
        {"id":"m","name":"N","dose":"1","instructions":"I","schedule":["08:00","20:00"],"dosesTakenToday":[true],"lastTakenAt":null}
        """.data(using: .utf8)!
        let item = try JSONDecoder().decode(MedicationItem.self, from: json)
        XCTAssertEqual(item.dosesTakenToday, [true, false])
    }

    @MainActor
    func testMedicationInitNormalizesTruncatedAndOversizedDoses() throws {
        // 1. Padding: schedule has 3 entries, 0 provided -> 3 falses
        let padded = MedicationItem(
            id: "m_pad", name: "Pad", dose: "10mg", instructions: "Daily",
            schedule: ["08:00", "14:00", "20:00"], dosesTakenToday: []
        )
        XCTAssertEqual(padded.dosesTakenToday, [false, false, false])

        // 2. Truncation: schedule has 2 entries, 4 provided -> prefix 2 kept
        let truncated = MedicationItem(
            id: "m_trunc", name: "Trunc", dose: "5mg", instructions: "Bid",
            schedule: ["09:00", "21:00"], dosesTakenToday: [true, false, true, true]
        )
        XCTAssertEqual(truncated.dosesTakenToday, [true, false])
    }

    @MainActor
    func testMedicationUpsertEditAndDelete() {
        let store = makeStore()
        let id = "med_test_\(UUID().uuidString)"
        var item = MedicationItem(
            id: id, name: "Iron", dose: "10 mg", instructions: "With food",
            schedule: ["09:00"],
            reminderSlots: [CareReminderSlot(clockTime: "09:00", remindersEnabled: true)]
        )
        store.upsertMedication(item)
        XCTAssertTrue(store.medications.contains(where: { $0.id == id }))
        item.setSchedule(["09:00", "21:00"])
        item.name = "Iron drops"
        store.upsertMedication(item)
        let saved = store.medications.first(where: { $0.id == id })
        XCTAssertEqual(saved?.name, "Iron drops")
        XCTAssertEqual(saved?.schedule, ["09:00", "21:00"])
        XCTAssertEqual(saved?.reminderSlots.count, 2)
        store.deleteMedication(id: id)
        XCTAssertFalse(store.medications.contains(where: { $0.id == id }))
    }

    @MainActor
    func testMedicationLinkedConditionPersists() {
        let store = makeStore()
        let id = "med_cond_\(UUID().uuidString)"
        store.upsertMedication(MedicationItem(
            id: id, name: "Melatonin", dose: "1 mg", instructions: "Bedtime",
            schedule: ["19:30"], linkedConditionId: "cond_sleep"
        ))
        XCTAssertEqual(store.medications.first(where: { $0.id == id })?.linkedConditionId, "cond_sleep")
    }

    @MainActor
    func testDiaryHistoryUpsertPastDayAndReminders() {
        let store = makeStore()
        let yesterday = Calendar.current.date(byAdding: .day, value: -1, to: Date())!
        store.upsertDiary(DailyDiaryEntry(
            date: yesterday, mood: .overwhelmed, notes: "Loud environment",
            eyeContactRating: 2, jointAttentionCount: 1, sensoryLoad: 5
        ))
        let loaded = store.diary(for: yesterday)
        XCTAssertEqual(loaded.mood, .overwhelmed)
        XCTAssertEqual(loaded.sensoryLoad, 5)
        XCTAssertTrue(loaded.hasCaregiverInput)
        XCTAssertTrue(store.recentDiaryEntries.contains(where: { Calendar.current.isDate($0.date, inSameDayAs: yesterday) }))

        let slot = CareReminderSlot(id: "diary_noon", clockTime: "12:30", remindersEnabled: true, liveActivityEnabled: true)
        store.upsertDiaryReminderSlot(slot)
        XCTAssertTrue(store.diaryReminderSlots.contains(where: { $0.id == "diary_noon" }))
        store.deleteDiaryReminderSlot(id: "diary_noon")
        XCTAssertFalse(store.diaryReminderSlots.contains(where: { $0.id == "diary_noon" }))
    }

    @MainActor
    func testSharedAlertRequestsCoverMedsAndDiary() {
        let med = MedicationItem(
            id: "m1", name: "Vit D", dose: "400 IU", instructions: "AM",
            schedule: ["08:00"],
            reminderSlots: [CareReminderSlot(clockTime: "08:00", remindersEnabled: true, alarmsEnabled: true)]
        )
        let slots = [CareReminderSlot(id: "d1", clockTime: "20:00", remindersEnabled: true, liveActivityEnabled: true)]
        let requests = CareAlertScheduler.shared.buildRequests(
            medications: [med], diarySlots: slots, diaryLoggedToday: false
        )
        XCTAssertEqual(requests.count, 2)
        XCTAssertEqual(requests.first?.kind, .medicationDose)
        XCTAssertEqual(requests.last?.kind, .diaryCheckIn)
        var taken = med
        taken.dosesTakenToday = [true]
        let skipped = CareAlertScheduler.shared.buildRequests(
            medications: [taken], diarySlots: [], diaryLoggedToday: true
        )
        XCTAssertTrue(skipped[0].skipToday)
        XCTAssertEqual(CareClockTime.parse("08:00")?.hour, 8)
        XCTAssertNil(CareClockTime.parse("25:99"))
        let next = CareClockTime.nextDate(clockTime: "00:00", from: Date(), skipToday: true)
        XCTAssertNotNil(next)
    }

    @MainActor
    func testClinicalCareKitJSONExport() {
        let store = makeStore()
        let jsonString = store.exportClinicalCareKitJSON()

        XCTAssertFalse(jsonString.isEmpty)
        guard let data = jsonString.data(using: .utf8) else {
            XCTFail("Failed to convert JSON string to data")
            return
        }

        guard let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            XCTFail("Failed to parse CareKit export JSON")
            return
        }

        XCTAssertEqual(dict["protocolVersion"] as? String, "CareKit-ASD-v1.3")
        XCTAssertNotNil(dict["activeTasks"])
        XCTAssertNotNil(dict["medications"])
        XCTAssertNotNil(dict["todayDiary"])
        XCTAssertNotNil(dict["diaryHistory"])
        XCTAssertNotNil(dict["weeklyAttentionTrend"])
        XCTAssertNotNil(dict["milestonesStatus"])
    }
}
