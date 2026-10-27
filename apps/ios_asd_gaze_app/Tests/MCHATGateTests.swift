@testable import AsdGazeCollector
import XCTest

/// H3: the M-CHAT-R/F questionnaire is OFF unless the build is made with
/// `MCHAT_LICENSED=1` (xcodegen) which defines `ASD_MCHAT_LICENSED`. The default
/// build must say so honestly and must not offer the questionnaire, while stored
/// results remain readable.
final class MCHATGateTests: XCTestCase {
    #if ASD_MCHAT_LICENSED
    func testLicensedBuildOffersQuestionnaire() {
        XCTAssertTrue(MCHATAvailability.isLicensed)
        XCTAssertTrue(MCHATAvailability.canStartQuestionnaire)
        XCTAssertNil(MCHATAvailability.unavailableMessage)
    }
    #else
    func testDefaultBuildReportsMCHATUnavailable() {
        XCTAssertFalse(MCHATAvailability.isLicensed)
        XCTAssertFalse(MCHATAvailability.canStartQuestionnaire)
    }

    func testDefaultBuildHasHonestUnavailableMessage() throws {
        let message = try XCTUnwrap(MCHATAvailability.unavailableMessage)
        XCTAssertTrue(message.contains("not available in this version"))
        XCTAssertFalse(message.lowercased().contains("complete the"))
    }
    #endif

    func testPresentersExposeTheGate() {
        XCTAssertEqual(CaregiverDashboardView.offersMCHATEntry, MCHATAvailability.canStartQuestionnaire)
        XCTAssertEqual(ResearchStudyDashboardCard.offersMCHATEntry, MCHATAvailability.canStartQuestionnaire)
    }

    func testStoredResultsStillDecodeAndDisplayWithoutLicense() throws {
        let json = Data(#"["Low Risk","Medium Risk","High Risk"]"#.utf8)
        let levels = try JSONDecoder().decode([MCHATRiskLevel].self, from: json)
        XCTAssertEqual(levels.count, 3)
        _ = MCHATHistoryView()
    }
}
