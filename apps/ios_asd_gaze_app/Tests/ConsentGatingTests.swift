@testable import AsdGazeCollector
import CurvAsdGazeKit
import XCTest

/// CNS-1: capture is gated by onboarding consent through `ConsentToken`.
@MainActor
final class ConsentGatingTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!

    override func setUp() {
        super.setUp()
        suiteName = "ConsentGatingTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suiteName)
        defaults = nil
        suiteName = nil
        super.tearDown()
    }

    func testMintReturnsNilWithoutConsent() {
        let store = ChildProfileStore(userDefaults: defaults)
        XCTAssertFalse(store.profile.hasConsentedPIA)
        XCTAssertNil(ConsentToken.mint(from: store), "No token may exist before the privacy consent step")
    }

    func testMintSucceedsAfterConsent() {
        let store = ChildProfileStore(userDefaults: defaults)
        let consentedAt = Date(timeIntervalSince1970: 1_800_000_000)
        store.profile.hasConsentedPIA = true
        store.profile.consentTimestamp = consentedAt

        let token = ConsentToken.mint(from: store)
        XCTAssertNotNil(token)
        XCTAssertEqual(token?.consentedAt, consentedAt)
    }

    func testStoreWithInjectedDefaultsDoesNotTouchStandardDefaults() {
        let store = ChildProfileStore(userDefaults: defaults)
        store.profile.childName = "Injected"
        XCTAssertNotNil(defaults.data(forKey: ChildProfileStore.storageKey))
    }

    func testConsentedSessionStartsWithValidToken() async throws {
        let store = ChildProfileStore(userDefaults: defaults)
        store.profile.hasConsentedPIA = true
        let token = try XCTUnwrap(ConsentToken.mint(from: store))

        let controller = GazeSessionController()
        XCTAssertTrue(controller.startConsentedSession(token: token, store: store))
        _ = await controller.stopSession()
    }

    func testConsentedSessionRefusedAfterConsentWithdrawn() throws {
        let store = ChildProfileStore(userDefaults: defaults)
        store.profile.hasConsentedPIA = true
        let token = try XCTUnwrap(ConsentToken.mint(from: store))

        store.reset()
        XCTAssertFalse(store.profile.hasConsentedPIA)

        let controller = GazeSessionController()
        XCTAssertFalse(
            controller.startConsentedSession(token: token, store: store),
            "A token minted before consent was withdrawn must not start capture"
        )
        XCTAssertFalse(controller.isRecording)
    }
}
