@testable import AsdGazeCollector
import XCTest

/// Store build routing and copy (store-collection-build.md S-1, S-2, S-4, S-8).
@MainActor
final class StoreBuildTests: XCTestCase {
    func testStoreBuildTabsHaveNoDashboard() {
        XCTAssertEqual(RootTab.visibleTabs(storeBuild: true), [.capture, .sessions, .settings])
    }

    func testOtherBuildsKeepTheDashboardAndHaveNoSessionsTab() {
        XCTAssertEqual(RootTab.visibleTabs(storeBuild: false), [.dashboard, .capture, .settings])
    }

    func testDisclaimerCarriesBothMandatorySentences() {
        XCTAssertTrue(StoreBuild.disclaimer.contains("not a diagnostic tool"))
        XCTAssertTrue(StoreBuild.disclaimer.hasPrefix("This is not a diagnostic tool."))
        XCTAssertTrue(StoreBuild.disclaimer.contains("It does not measure, screen for or diagnose any condition; your clinic reviews each session."))
    }

    func testFlagMatchesConfiguration() {
        #if ASD_STORE_BUILD
        XCTAssertTrue(StoreBuild.isStoreBuild)
        #else
        // App tests run in Debug, which never defines the store flag (S-1).
        XCTAssertFalse(StoreBuild.isStoreBuild)
        XCTAssertEqual(RootTab.visibleTabs(), [.dashboard, .capture, .settings])
        #endif
    }

    func testHTTPSURLRejectsOtherSchemesAndJunk() {
        XCTAssertNil(StoreBuild.httpsURL(nil))
        XCTAssertNil(StoreBuild.httpsURL(42))
        XCTAssertNil(StoreBuild.httpsURL("http://curv.engineering/support"))
        XCTAssertNil(StoreBuild.httpsURL("not a url"))
        XCTAssertEqual(StoreBuild.httpsURL(" https://curv.engineering/x ")?.absoluteString, "https://curv.engineering/x")
    }

    // MARK: - Session storage state (S-4)

    private let ended = Date(timeIntervalSince1970: 1_000_000)

    func testWithoutSyncASessionIsOnTheDevice() {
        XCTAssertEqual(SessionStorageState.resolve(endedAt: ended, syncConfigured: false, summary: nil), .onDevice)
        let summary = SyncStatusSummary(waiting: 0, uploading: 0, needsAttention: 0, lastSyncedAt: ended)
        XCTAssertEqual(SessionStorageState.resolve(endedAt: ended, syncConfigured: false, summary: summary), .onDevice)
    }

    func testUploadedOnlyWhenQueueEmptyAndSyncedAfterTheSession() {
        let later = SyncStatusSummary(waiting: 0, uploading: 0, needsAttention: 0, lastSyncedAt: ended.addingTimeInterval(5))
        XCTAssertEqual(SessionStorageState.resolve(endedAt: ended, syncConfigured: true, summary: later), .uploaded)

        let earlier = SyncStatusSummary(waiting: 0, uploading: 0, needsAttention: 0, lastSyncedAt: ended.addingTimeInterval(-5))
        XCTAssertEqual(SessionStorageState.resolve(endedAt: ended, syncConfigured: true, summary: earlier), .waiting)

        let queued = SyncStatusSummary(waiting: 1, uploading: 0, needsAttention: 0, lastSyncedAt: ended.addingTimeInterval(5))
        XCTAssertEqual(SessionStorageState.resolve(endedAt: ended, syncConfigured: true, summary: queued), .waiting)
    }

    func testDeadLetteredQueueNeedsAttention() {
        let stuck = SyncStatusSummary(waiting: 0, uploading: 0, needsAttention: 2, lastSyncedAt: nil)
        XCTAssertEqual(SessionStorageState.resolve(endedAt: ended, syncConfigured: true, summary: stuck), .needsAttention)
    }

    /// The Sessions tab and the result screen never word a verdict (S-3, ADR-0004).
    func testStorageCopyHasNoVerdictWording() {
        let all: [SessionStorageState] = [.onDevice, .waiting, .needsAttention, .uploaded]
        for state in all {
            for word in ["score", "flag", "risk", "diagnos", "screen"] {
                XCTAssertFalse(state.text.lowercased().contains(word), "\(state.text) contains \(word)")
            }
        }
        XCTAssertEqual(StoreBuild.localOnlyNotice, "Sessions are stored on this device until your clinic enrols it.")
    }
}
