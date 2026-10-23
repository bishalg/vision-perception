import Foundation
import XCTest
@testable import AsdGazeCollector
import CurvAsdGazeKit

final class MoodDiaryModuleTests: XCTestCase {
    func testDistressHeuristicRequiresBrowAndFrown() {
        XCTAssertFalse(
            DistressBlendShapeHeuristic.isDistressFrame(
                browInnerUp: 0.8,
                mouthFrownLeft: 0.05,
                mouthFrownRight: 0.05
            )
        )
        XCTAssertTrue(
            DistressBlendShapeHeuristic.isDistressFrame(
                browInnerUp: 0.5,
                mouthFrownLeft: 0.4,
                mouthFrownRight: 0.4
            )
        )
    }

    func testPediatricEmotionValenceMapping() {
        XCTAssertGreaterThan(PediatricEmotion.joy.healthValence, 0)
        XCTAssertLessThan(PediatricEmotion.meltdown.healthValence, 0)
        XCTAssertEqual(PediatricEmotion.fromHealthValence(0.7), .joy)
        XCTAssertEqual(PediatricEmotion.fromHealthValence(-0.8), .meltdown)
        XCTAssertEqual(PediatricEmotion.meltdown.childMood, .overwhelmed)
        XCTAssertEqual(PediatricEmotion.engaged.childMood, .engaged)
    }

    @MainActor
    func testFusionPromptsAfterTwoMinutesOfDistress() {
        let fusion = ARKitMoodFusionController()
        let start = Date()
        let affect = FacialAffectMetrics(
            browInnerUp: 0.6,
            mouthFrownLeft: 0.5,
            mouthFrownRight: 0.5
        )
        fusion.ingestFrame(at: start, affect: affect, isRecording: true)
        for offset in stride(from: 1.0, through: 121, by: 1.0) {
            fusion.ingestFrame(
                at: start.addingTimeInterval(offset),
                affect: affect,
                isRecording: true
            )
        }
        XCTAssertNotNil(fusion.pendingSuggestion)
        XCTAssertEqual(
            fusion.pendingSuggestion?.prompt,
            "We noticed some distress earlier. Would you like to log this?"
        )
        XCTAssertEqual(fusion.pendingSuggestion?.suggestedEmotion, .meltdown)
    }

    @MainActor
    func testFusionPromptsAtSessionEndAfterEightSeconds() {
        let fusion = ARKitMoodFusionController()
        let start = Date()
        let affect = FacialAffectMetrics(
            browInnerUp: 0.5,
            mouthFrownLeft: 0.4,
            mouthFrownRight: 0.4
        )
        fusion.ingestFrame(at: start, affect: affect, isRecording: true)
        for offset in stride(from: 1.0, through: 9, by: 1.0) {
            fusion.ingestFrame(
                at: start.addingTimeInterval(offset),
                affect: affect,
                isRecording: true
            )
        }
        XCTAssertNil(fusion.pendingSuggestion)
        fusion.handleRecordingDidStop()
        XCTAssertNotNil(fusion.pendingSuggestion)
        XCTAssertEqual(fusion.pendingSuggestion?.suggestedEmotion, .overstimulated)
    }

    #if ASD_ML_DATA_EXPORT // CNS-2: the logger only exists in ML data-export builds.
    func testCSVHeaderAndRowIncludeBlendShapeLocations() {
        let logger = BlendShapeCSVLogger()
        XCTAssertTrue(logger.headerLine.contains("browInnerUp"))
        XCTAssertTrue(logger.headerLine.contains("mouthFrownLeft"))
        XCTAssertEqual(FaceBlendShapeCSV.columns.count, 52)
        let line = logger.csvLine(
            timestamp: Date(timeIntervalSince1970: 0),
            sessionId: "s1",
            isRecording: true,
            shapes: ["browInnerUp": 0.42, "mouthFrownLeft": 0.31]
        )
        let parts = line.split(separator: ",")
        XCTAssertEqual(parts.count, 3 + FaceBlendShapeCSV.columns.count)
        XCTAssertEqual(parts[2], "1")
        let browIndex = FaceBlendShapeCSV.columns.firstIndex(of: "browInnerUp")!
        XCTAssertEqual(parts[3 + browIndex], "0.42000")
    }
    #endif

    @MainActor
    func testMoodDiaryStorePersistsInIsolatedDefaults() {
        let suite = UserDefaults(suiteName: "MoodDiaryTests-\(UUID().uuidString)")!
        let store = MoodDiaryStore(defaults: suite, healthKit: HealthKitMoodStore.shared, carePlan: nil)
        let exp = expectation(description: "save")
        Task {
            await store.upsert(
                MoodDiaryEntry(emotion: .joy, intensity: 0.8, durationMinutes: 12, tags: [.postSchool]),
                syncHealth: false
            )
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)
        XCTAssertEqual(store.entries.count, 1)
        XCTAssertEqual(store.entries.first?.emotion, .joy)
        XCTAssertEqual(store.entries.first?.tags, [.postSchool])
        XCTAssertNil(store.entries.first?.recognition)
    }

    func testDiaryRecognitionRoundTripsAndOldEntriesStayReadable() throws {
        let entry = MoodDiaryEntry(
            emotion: .engaged,
            recognition: .sharedAttention,
            intensity: 0.4,
            durationMinutes: 6,
            notes: ""
        )
        let data = try JSONEncoder().encode([entry])
        let decoded = try JSONDecoder().decode([MoodDiaryEntry].self, from: data)
        XCTAssertEqual(decoded.first?.recognition, .sharedAttention)

        var object = try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
        object[0].removeValue(forKey: "recognition")
        let stripped = try JSONSerialization.data(withJSONObject: object)
        let legacy = try JSONDecoder().decode([MoodDiaryEntry].self, from: stripped)
        XCTAssertNil(legacy.first?.recognition)
        XCTAssertEqual(legacy.first?.emotion, .engaged)
    }
}
