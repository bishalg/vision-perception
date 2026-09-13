import XCTest
@testable import AsdGazeCollector

final class CarePlanStoreTests: XCTestCase {
    @MainActor
    func testTaskCompletionAndRatioCalculation() {
        let store = CarePlanStore()
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
    func testMoodAndNotesUpdates() {
        let store = CarePlanStore()

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
        let store = CarePlanStore()
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
    func testClinicalCareKitJSONExport() {
        let store = CarePlanStore()
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

        XCTAssertEqual(dict["protocolVersion"] as? String, "CareKit-ASD-v1.2")
        XCTAssertNotNil(dict["activeTasks"])
        XCTAssertNotNil(dict["todayDiary"])
        XCTAssertNotNil(dict["weeklyAttentionTrend"])
        XCTAssertNotNil(dict["milestonesStatus"])
    }
}
