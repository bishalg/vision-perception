import XCTest
@testable import CurvAsdGazeKit

/// CLM-8 (ADR-0010 d3): the Swift types were renamed `Biomarker*` -> `Engagement*`, but every
/// wire name keeps `biomarker`. These tests pin the JSON property names the vault writes.
final class WireNamePinningTests: XCTestCase {
    private func makeScore() -> PerVideoSessionScore {
        let row = PerVideoScoreRow(
            segmentId: "seg-1", segmentDescription: "Face region", biomarker: "socialPreferenceScore",
            targetAOI: "Social_Actor", metricKey: "dwell", direction: .higherBetter,
            observed: 0.5, expected: 0.6, acceptableLow: 0.4, acceptableHigh: 0.8,
            status: .onTarget, score: 0.9, imputed: false, citation: "test", source: "test"
        )
        return PerVideoSessionScore(
            ageBracketId: "default", ageMonths: 30, compositeScore: 0.9,
            videos: [PerVideoScoreReport(
                stimulusId: "a", title: "A", category: "GeoPref", rows: [row], subscore: 0.9,
                validSampleRatio: 0.95, lowConfidence: false, hasBaseline: true, weight: 1
            )],
            baselineStatus: "provisional", baselineSchemaVersion: "1", scoredAtUtc: ""
        )
    }

    func testResearchKitJsonKeepsBiomarkerWireNames() throws {
        let report = GazeSessionReportSummary.fixture(
            sessionId: "WIRE0001", socialPreferenceScore: 0.55, socialDwellSec: 6.0, geometricDwellSec: 4.0
        )
        let json = SessionVaultManager.makeResearchKitJson(report: report, perVideoScore: makeScore())
        XCTAssertNotNil(json["biomarkers"], "the `biomarkers` wire key must not be renamed")
        let data = try JSONSerialization.data(withJSONObject: json, options: [.sortedKeys])
        let text = try XCTUnwrap(String(data: data, encoding: .utf8))
        XCTAssertTrue(text.contains("\"biomarkers\":"))
        XCTAssertTrue(text.contains("\"biomarker\":\"socialPreferenceScore\""))
        XCTAssertFalse(text.contains("\"engagement"))
    }

    func testVaultFileKeepsBiomarkerWireName() throws {
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let vault = SessionVaultManager(vaultDirectory: tempDir, keyData: Data(repeating: 5, count: 32))
        let report = GazeSessionReportSummary.fixture(
            sessionId: "WIRE0002", socialPreferenceScore: 0.55, socialDwellSec: 6.0, geometricDwellSec: 4.0
        )
        let url = try vault.saveSession(
            header: SessionHeader.fixture(sessionId: "WIRE0002"), frames: [], report: report,
            perVideoScore: makeScore()
        )
        let text = try vault.readSessionPlainText(from: url)
        XCTAssertTrue(text.contains("\"biomarker\":\"socialPreferenceScore\""))
        XCTAssertFalse(text.contains("\"engagement\""))
    }
}
