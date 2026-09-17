import XCTest
import CoreGraphics
import CurvPalmOverlay
@testable import PalmScanner

final class SimplePalmReadingTests: XCTestCase {
    private typealias LineReports = SimplePalmReading.LineReports

    private func spline(_ type: CreaseLineType) -> CreaseSpline {
        CreaseSpline(id: type.rawValue, lineType: type, points: [.zero, CGPoint(x: 1, y: 1)],
                     confidence: 0.8, tortuosity: 1.05, arcLengthPx: 140)
    }

    private func reports(_ json: String) throws -> LineReports {
        let payload = try JSONDecoder().decode(PalmDomainPayload.self, from: Data(json.utf8))
        return .from(payload: payload)
    }

    func testEmptyAndFailureInputsYieldNoReadingOrScanSummary() throws {
        XCTAssertEqual(SimplePalmReading.reading(for: [], handSide: .right), [])
        let failed = try XCTUnwrap(PalmAnalysisResponse.from(jsonString: #"{"is_usable":false}"#))
        let reports = LineReports.from(payload: failed.domainPayload)
        XCTAssertTrue(reports.lines.isEmpty)
        XCTAssertNil(reports.fate)
        XCTAssertNil(SimplePalmReading.scanSummary(from: reports))
        XCTAssertEqual(SimplePalmReading.reading(for: [], handSide: .right, reports: reports), [])
    }

    func testMissingReportsDoNotAppendMeasuredOrScanClaims() {
        for type in CreaseLineType.allCases {
            let text = SimplePalmReading.readingText(for: spline(type), handSide: .right)
            XCTAssertEqual(text, SimplePalmReading.readingText(for: spline(type), handSide: .right, reports: LineReports()))
            XCTAssertFalse(text.contains("Measured:"))
            XCTAssertFalse(text.contains("Scan:"))
        }
    }

    func testHandSideContextOnlyAnnotatesHeartAndLife() {
        for type in [CreaseLineType.heart, .life] {
            XCTAssertTrue(SimplePalmReading.readingText(for: spline(type), handSide: .left).contains("Left (inherited inclination)"))
            XCTAssertTrue(SimplePalmReading.readingText(for: spline(type), handSide: .right).contains("Right (active expression)"))
            XCTAssertTrue(SimplePalmReading.readingText(for: spline(type), handSide: .auto).contains("Active hand"))
        }
        for type in [CreaseLineType.head, .fate] {
            XCTAssertEqual(SimplePalmReading.readingText(for: spline(type), handSide: .left),
                           SimplePalmReading.readingText(for: spline(type), handSide: .right))
        }
    }

    func testAbsentAndUnconfirmedReportsCannotAppendMeasuredClauses() throws {
        let reports = try reports(#"{"heart_analysis":{"is_short":true},"fate_analysis":{"present":false,"is_short":true}}"#)
        for type in [CreaseLineType.heart, .fate] {
            XCTAssertFalse(SimplePalmReading.readingText(for: spline(type), handSide: .right, reports: reports).contains("Measured:"))
        }
    }

    func testPresentReportAppendsGeometryAndDevelopmentQualification() throws {
        let reports = try reports(#"{"heart_analysis":{"present":true,"is_short":true,"broken":true}}"#)
        let text = SimplePalmReading.readingText(for: spline(.heart), handSide: .right, reports: reports)
        for fragment in ["Measured:", "short span", "interrupted", "(development thresholds)"] {
            XCTAssertTrue(text.contains(fragment), fragment)
        }
        XCTAssertFalse(SimplePalmReading.readingText(for: spline(.head), handSide: .right, reports: reports).contains("Measured:"))
    }

    func testIncompleteFingertipEvidenceDoesNotInventSummary() throws {
        let reports = try reports(#"{"mount_relief":{},"minor_ridges":[],"fingertip_pattern":{"digit":"index"}}"#)
        XCTAssertNil(SimplePalmReading.scanSummary(from: reports))
        XCTAssertFalse(SimplePalmReading.readingText(for: spline(.head), handSide: .right, reports: reports).contains("Scan:"))
    }

    func testScanEvidenceMapsToSummaryAndRelevantLinesOnly() throws {
        let reports = try reports(#"{"minor_ridges":[{"segment_id":1},{"segment_id":2}],"fingertip_pattern":{"digit":"index","pattern":"whorl"}}"#)
        XCTAssertEqual(reports.enrichment.minorRidgeCount, 2)
        XCTAssertEqual(SimplePalmReading.scanSummary(from: reports), "2 fine crease segments captured. Dermatoglyphics — index: whorl.")
        let head = SimplePalmReading.readingText(for: spline(.head), handSide: .right, reports: reports)
        XCTAssertTrue(head.contains("2 minor crease segments mapped"))
        XCTAssertTrue(head.contains("finger pads: index whorl"))
        XCTAssertFalse(SimplePalmReading.readingText(for: spline(.life), handSide: .right, reports: reports).contains("Scan:"))
    }

    func testReadingPreservesInputOrderAndDefaultsToRightHand() {
        let input = [spline(.life), spline(.head)]
        XCTAssertEqual(SimplePalmReading.reading(for: input), input.map { SimplePalmReading.readingText(for: $0, handSide: .right) })
    }
}
