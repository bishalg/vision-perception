import XCTest
@testable import CurvAsdGazeKit

final class AttentionFlagTests: XCTestCase {
    func testGreenWhenCompositeIsStrongAndRowsStayInRange() {
        let assessment = AttentionFlagAlgorithm.assess(session(composite: 0.82, rows: [.onTarget, .onTarget, .monitor]))
        XCTAssertEqual(assessment.flag, .green)
        XCTAssertTrue(assessment.isScreeningSignalOnly)
        XCTAssertTrue(assessment.detail.contains("not a diagnosis"))
    }

    func testYellowWhenCompositeSitsBetweenTheFloors() {
        let assessment = AttentionFlagAlgorithm.assess(session(composite: 0.50, rows: [.monitor, .onTarget]))
        XCTAssertEqual(assessment.flag, .yellow)
        XCTAssertEqual(assessment.headline, "Some clips differ from the age reference")
    }

    func testRedWhenCompositeFallsBelowTheYellowFloor() {
        let assessment = AttentionFlagAlgorithm.assess(session(composite: 0.20, rows: [.atypical, .monitor]))
        XCTAssertEqual(assessment.flag, .red)
    }

    func testRedWhenAtypicalRowsDominateAHighComposite() {
        let assessment = AttentionFlagAlgorithm.assess(
            session(composite: 0.80, rows: [.atypical, .atypical, .onTarget])
        )
        XCTAssertEqual(assessment.flag, .red)
    }

    func testYellowWhenMostClipsAreLowConfidence() {
        let assessment = AttentionFlagAlgorithm.assess(
            session(composite: 0.90, rows: [.onTarget], lowConfidence: true)
        )
        XCTAssertEqual(assessment.flag, .yellow)
        XCTAssertTrue(assessment.headline.contains("Recheck"))
    }

    func testYellowWhenNothingWasScored() {
        let empty = PerVideoSessionScore(
            ageBracketId: "24",
            ageMonths: 24,
            compositeScore: 0,
            videos: [],
            baselineStatus: "provisional",
            baselineSchemaVersion: "1.0.0",
            scoredAtUtc: "2026-10-20T00:00:00Z"
        )
        XCTAssertEqual(AttentionFlagAlgorithm.assess(empty).flag, .yellow)
    }

    private func session(
        composite: Double,
        rows: [ExpectationStatus],
        lowConfidence: Bool = false
    ) -> PerVideoSessionScore {
        let scoredRows = rows.enumerated().map { index, status in
            PerVideoScoreRow(
                segmentId: "seg-\(index)",
                segmentDescription: "segment",
                biomarker: "social_dwell_ratio",
                targetAOI: nil,
                metricKey: "social_dwell_ratio",
                direction: .higherBetter,
                observed: 0.5,
                expected: 0.5,
                acceptableLow: 0.2,
                acceptableHigh: 0.8,
                status: status,
                score: status == .onTarget ? 1 : 0.4,
                imputed: false,
                citation: "test",
                source: "test"
            )
        }
        let report = PerVideoScoreReport(
            stimulusId: "stim_test",
            title: "Test",
            category: "social",
            rows: scoredRows,
            subscore: composite,
            validSampleRatio: lowConfidence ? 0.2 : 0.95,
            lowConfidence: lowConfidence,
            hasBaseline: true,
            weight: 1
        )
        return PerVideoSessionScore(
            ageBracketId: "24",
            ageMonths: 24,
            compositeScore: composite,
            videos: [report],
            baselineStatus: "provisional",
            baselineSchemaVersion: "1.0.0",
            scoredAtUtc: "2026-10-20T00:00:00Z"
        )
    }
}
