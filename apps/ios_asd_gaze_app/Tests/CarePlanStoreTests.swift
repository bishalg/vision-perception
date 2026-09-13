import CareKitStore
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

        store.markScreeningCompleted(socialScore: 0.65)

        // Observation task should be completed
        let observationTask = store.tasks.first(where: { $0.category == .observation })
        XCTAssertNotNil(observationTask)
        XCTAssertTrue(observationTask?.isCompleted == true)

        // Latest weekly history session count should increment
        let updatedSessions = store.weeklyHistory.last?.sessionCount ?? 0
        XCTAssertEqual(updatedSessions, initialSessions + 1)
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
        XCTAssertNotNil(dict["weeklyAttentionTrend"])
        XCTAssertNotNil(dict["milestonesStatus"])
    }
}
