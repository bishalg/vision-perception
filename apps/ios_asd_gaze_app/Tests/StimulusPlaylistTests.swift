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

    func testShippedClipChecklistCoversBothBatteries() {
        let clips = StimulusPlaylistBattery.shippedClips
        let names = clips.map(\.videoName)
        XCTAssertEqual(Set(names).count, names.count)
        let required = (StimulusPlaylistBattery.standard30s + StimulusPlaylistBattery.comprehensive60s).map(\.videoName)
        for name in required {
            XCTAssertTrue(names.contains(name), name)
        }
        XCTAssertEqual(clips.count, 9)
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
    func testOnboardingPackTagResolution() {
        let manager = OnDemandStimulusManager.shared
        // Age band midpoints from the onboarding profile step...
        XCTAssertEqual(manager.resolveTag(ageMonths: 14, localeCode: "en"), "pack_age_12_24_en")
        XCTAssertEqual(manager.resolveTag(ageMonths: 28, localeCode: "en"), "pack_age_24_36_en")
        XCTAssertEqual(manager.resolveTag(ageMonths: 54, localeCode: "en"), "pack_age_36_60_en")
        // ...crossed with the onboarding language/region pick.
        XCTAssertEqual(manager.resolveTag(ageMonths: 20, localeCode: "hi"), "pack_age_12_24_hi")
        XCTAssertEqual(manager.resolveTag(ageMonths: 20, localeCode: "ne"), "pack_age_12_24_ne")
        XCTAssertEqual(manager.resolveTag(ageMonths: 40, localeCode: "zh"), "pack_age_36_60_zh")
        // Unknown regions fall back to English packs, never to nothing.
        XCTAssertEqual(manager.resolveTag(ageMonths: 20, localeCode: "xx"), "pack_age_12_24_en")
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
