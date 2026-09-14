import XCTest
import CurvAsdGazeKit
@testable import AsdGazeCollector

final class StimulusPlaylistTests: XCTestCase {

    func testStandard30sBatteryIntegrity() {
        let battery = StimulusPlaylistBattery.standard30s
        XCTAssertEqual(battery.count, 4, "30s express battery should contain exactly 4 clinical stimuli")

        let totalDuration = battery.reduce(0.0) { $0 + $1.targetDurationSec }
        XCTAssertEqual(totalDuration, 30.0, accuracy: 2.0, "Total duration should be approximately 30 seconds")

        // Validate each stimulus has an associated AOI tracker
        for item in battery {
            let tracker = StimulusAOITracker.tracker(forVideoNamed: item.videoName)
            XCTAssertFalse(tracker.aois.isEmpty, "Stimulus \(item.videoName) must have active AOIs registered")
        }
    }

    func testComprehensive60sBatteryIntegrity() {
        let battery = StimulusPlaylistBattery.comprehensive60s
        XCTAssertEqual(battery.count, 8, "60s full battery should contain exactly 8 clinical stimuli")

        let totalDuration = battery.reduce(0.0) { $0 + $1.targetDurationSec }
        XCTAssertEqual(totalDuration, 56.0, accuracy: 5.0, "Total duration should span ~55-60 seconds")

        // Validate each stimulus has an associated AOI tracker
        for item in battery {
            let tracker = StimulusAOITracker.tracker(forVideoNamed: item.videoName)
            XCTAssertFalse(tracker.aois.isEmpty, "Stimulus \(item.videoName) must have active AOIs registered")
        }
    }

    func testPlaylistFactorySelection() {
        let shortBattery = StimulusPlaylistBattery.playlist(forTargetDuration: 30.0)
        XCTAssertEqual(shortBattery.count, 4)
        XCTAssertEqual(shortBattery.first?.videoName, "stim_05_geopref_split_10s")

        let longBattery = StimulusPlaylistBattery.playlist(forTargetDuration: 60.0)
        XCTAssertEqual(longBattery.count, 8)
        XCTAssertEqual(longBattery.first?.videoName, "stim_05_geopref_split_10s")
    }

    @MainActor
    func testChildProfileDurationPersistence() {
        let store = ChildProfileStore.shared
        let original = store.profile

        var modified = original
        modified.sessionDurationSec = 60
        store.profile = modified
        XCTAssertEqual(ChildProfileStore.shared.profile.sessionDurationSec, 60)

        modified.sessionDurationSec = 30
        store.profile = modified
        XCTAssertEqual(ChildProfileStore.shared.profile.sessionDurationSec, 30)

        // Restore original
        store.profile = original
    }
}
