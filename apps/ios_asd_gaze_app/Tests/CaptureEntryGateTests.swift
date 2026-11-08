@testable import AsdGazeCollector
import CurvAsdGazeKit
import XCTest

/// SIM-2 / SIM-7 / CNS-1: the entry gate and the unsupported-device copy.
@MainActor
final class CaptureEntryGateTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!

    override func setUp() async throws {
        try await super.setUp()
        suiteName = "CaptureEntryGateTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
    }

    override func tearDown() async throws {
        defaults.removePersistentDomain(forName: suiteName)
        try await super.tearDown()
    }

    private func consentedStore() -> ChildProfileStore {
        let store = ChildProfileStore(userDefaults: defaults)
        store.profile.hasConsentedPIA = true
        return store
    }

    func testUnsupportedDeviceIsBlockedEvenWithConsent() {
        let gate = CaptureEntryGate.evaluate(store: consentedStore(), faceTrackingSupported: false)
        XCTAssertEqual(gate, .blocked(.faceTrackingUnsupported))
    }

    func testMissingConsentIsBlocked() {
        let store = ChildProfileStore(userDefaults: defaults)
        let gate = CaptureEntryGate.evaluate(store: store, faceTrackingSupported: true)
        XCTAssertEqual(gate, .blocked(.consentRequired))
    }

    private func consentRecords(agreed: Bool) -> ConsentRecordStore {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("CaptureEntryGateTests_consent_\(UUID().uuidString).json")
        let records = ConsentRecordStore(fileURL: url)
        if agreed { records.agree(guardianName: "Sam Guardian", relationship: .parent) }
        return records
    }

    func testSupportedAndConsentedIsReady() {
        let gate = CaptureEntryGate.evaluate(
            store: consentedStore(), consentRecords: consentRecords(agreed: true), faceTrackingSupported: true)
        guard case .ready = gate else { return XCTFail("expected ready, got \(gate)") }
    }

    /// S-10: the legacy onboarding switch alone is not enough; a signed record for the
    /// current document is required (once after upgrade, and after each version bump).
    func testLegacyConsentWithoutCurrentRecordIsOutdated() {
        let gate = CaptureEntryGate.evaluate(
            store: consentedStore(), consentRecords: consentRecords(agreed: false), faceTrackingSupported: true)
        XCTAssertEqual(gate, .blocked(.consentOutdated))
    }

    func testRecordForOlderDocumentVersionIsOutdated() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("CaptureEntryGateTests_consent_\(UUID().uuidString).json")
        let old = ConsentDocument(version: "2000-01-01.1", sections: ConsentDocument.current.sections)
        ConsentRecordStore(fileURL: url, document: old).agree(guardianName: "Sam Guardian", relationship: .parent)
        let gate = CaptureEntryGate.evaluate(
            store: consentedStore(), consentRecords: ConsentRecordStore(fileURL: url), faceTrackingSupported: true)
        XCTAssertEqual(gate, .blocked(.consentOutdated))
    }

    func testBlockReasonMapsFromTrackingUnavailableReason() {
        XCTAssertEqual(DeviceBlockReason(.faceTrackingUnsupported), .faceTrackingUnsupported)
        XCTAssertEqual(DeviceBlockReason(.cameraPermissionDenied), .cameraPermissionDenied)
        XCTAssertEqual(DeviceBlockReason(.sessionFailed), .sessionFailed)
        XCTAssertEqual(DeviceBlockReason(.interrupted), .interrupted)
    }

    func testStartWithoutConsentDoesNotStartOrFlipRecording() {
        let controller = GazeSessionController()
        let store = ChildProfileStore(userDefaults: defaults)
        XCTAssertFalse(controller.startSessionWithConsent(store: store))
        XCTAssertFalse(controller.isRecording)
    }

    func testCopyIsPlainAndNeverOffersSimulation() {
        for reason in DeviceBlockReason.allCases {
            let c = UnsupportedDeviceContent.make(for: reason)
            XCTAssertFalse(c.title.isEmpty)
            XCTAssertFalse(c.message.isEmpty)
            let text = (c.title + c.message).lowercased()
            XCTAssertFalse(text.contains("simulat"))
            XCTAssertFalse(text.contains("diagnos") && !text.contains("not"))
        }
        XCTAssertEqual(UnsupportedDeviceContent.make(for: .faceTrackingUnsupported).title,
                       "This device cannot run gaze observations")
        XCTAssertTrue(UnsupportedDeviceContent.make(for: .faceTrackingUnsupported).message.contains("Face ID"))
    }
}
