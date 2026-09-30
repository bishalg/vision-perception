import XCTest
@testable import AsdGazeCollector

@MainActor
final class AppModeStoreTests: XCTestCase {

    private func makeStore(suiteName: String) -> AppModeStore {
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return AppModeStore(defaults: defaults)
    }

    func testDefaultsToParentModeUntilSelectionIsMade() {
        let store = makeStore(suiteName: "appmode-default")
        XCTAssertEqual(store.mode, .parentHome)
        XCTAssertFalse(store.hasSelectedMode)
    }

    func testSelectionPersistsAcrossInstances() {
        let suite = "appmode-select-\(UUID().uuidString)"
        let first = makeStore(suiteName: suite)
        first.select(.clinic)
        XCTAssertEqual(first.mode, .clinic)
        XCTAssertTrue(first.hasSelectedMode)

        // A brand-new instance (like a fresh app launch) reads the persisted choice.
        let second = AppModeStore(defaults: UserDefaults(suiteName: suite)!)
        XCTAssertEqual(second.mode, .clinic)
        XCTAssertTrue(second.hasSelectedMode)
        UserDefaults().removePersistentDomain(forName: suite)
    }

    func testModeSelectionChoiceIsSurvivableAndSwitchable() {
        let suite = "appmode-switch-\(UUID().uuidString)"
        let store = makeStore(suiteName: suite)
        store.select(.parentHome)
        XCTAssertEqual(store.mode, .parentHome)
        store.select(.clinic)
        XCTAssertEqual(store.mode, .clinic)
        UserDefaults().removePersistentDomain(forName: suite)
    }
}
