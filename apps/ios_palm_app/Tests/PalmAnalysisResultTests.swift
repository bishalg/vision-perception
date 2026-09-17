import XCTest
import CoreGraphics
@testable import PalmScanner

final class PalmAnalysisResultTests: XCTestCase {
    func testEmptyObjectDecodesWithoutInventingEvidence() throws {
        let response = try XCTUnwrap(PalmAnalysisResponse.from(jsonString: "{}"))
        XCTAssertNil(response.isUsable)
        XCTAssertNil(response.totalDurationUs)
        XCTAssertNil(response.domainPayload)
        XCTAssertNil(response.creaseSplinesFromPolylines())
    }

    func testFailureResponsePreservesUnusableFlagAndHasNoSplines() throws {
        let response = try XCTUnwrap(PalmAnalysisResponse.from(jsonString:
            #"{"is_usable":false,"total_duration_us":17,"domain_payload":{}}"#))
        XCTAssertEqual(response.isUsable, false)
        XCTAssertEqual(response.totalDurationUs, 17)
        XCTAssertNil(response.creaseSplinesFromPolylines())
    }

    func testMalformedAndWronglyTypedPayloadsFailBothEntryPoints() {
        for json in ["", "not json", "[]", "null", #"{"is_usable":"yes"}"#,
                     #"{"domain_payload":{"major_lines":{"heart_line":{"present":true}}}}"#] {
            XCTAssertNil(PalmAnalysisResponse.from(jsonString: json), json)
            XCTAssertNil(PalmAnalysisResponse.from(data: Data(json.utf8)), json)
        }
    }

    func testOlderOptionalReportsDecodeAndUnknownFieldsAreIgnored() throws {
        let response = try XCTUnwrap(PalmAnalysisResponse.from(jsonString:
            #"{"future_field":1,"domain_payload":{"heart_analysis":{},"fate_analysis":{},"mount_relief":{},"minor_ridges":[],"fingertip_pattern":{}}}"#))
        let payload = try XCTUnwrap(response.domainPayload)
        XCTAssertNotNil(payload.heartAnalysis)
        XCTAssertNil(payload.heartAnalysis?.present)
        XCTAssertNil(payload.fateAnalysis?.present)
        XCTAssertEqual(payload.minorRidges?.count, 0)
        XCTAssertNil(response.creaseSplinesFromPolylines())
    }

    func testOnlyPresentKnownLinesWithEnoughPointsBecomeSplines() throws {
        let lines: [String: Any] = [
            "heart_line": line(present: true, points: [[0, 0], [0.25, 0.25], [0.5, 0.5], [0.75, 0.75], [1, 1]]),
            "head_line": line(present: false, points: [[0, 0], [1, 1]]),
            "life_line": line(present: true, points: [[0, 0]]),
            "fate_line": line(present: true, points: [[], [0.5], [0.25, 0.5], [0.75, 1]]),
            "unknown_line": line(present: true, points: [[0, 0], [1, 1]])
        ]
        let data = try JSONSerialization.data(withJSONObject: ["domain_payload": ["major_lines": lines]])
        let response = try XCTUnwrap(PalmAnalysisResponse.from(data: data))
        let splines = try XCTUnwrap(response.creaseSplinesFromPolylines())
        XCTAssertEqual(splines.map(\.id), ["heart_line", "fate_line"])
        XCTAssertEqual(splines.map(\.lineType), [.heart, .fate])
        XCTAssertEqual(splines[0].points.count, 4)
        XCTAssertEqual(splines[0].points.first, CGPoint.zero)
        XCTAssertEqual(splines[0].points.last, CGPoint(x: 1, y: 1))
        XCTAssertEqual(splines[0].confidence, 0.75)
        XCTAssertEqual(splines[0].arcLengthPx, 140)
        XCTAssertFalse(splines[0].isUserAdjusted)
        XCTAssertEqual(splines[1].points, [CGPoint(x: 0.25, y: 0.5), CGPoint(x: 0.75, y: 1)])
    }

    func testAbsentOptionalPolylineProducesNoOverlay() throws {
        var info = line(present: true, points: [])
        info.removeValue(forKey: "points_image_norm")
        let data = try JSONSerialization.data(withJSONObject: ["domain_payload": ["major_lines": ["heart_line": info]]])
        let response = try XCTUnwrap(PalmAnalysisResponse.from(data: data))
        XCTAssertEqual(response.domainPayload?.majorLines?["heart_line"]?.imagePoints, [])
        XCTAssertNil(response.creaseSplinesFromPolylines())
    }

    private func line(present: Bool, points: [[Double]]) -> [String: Any] {
        ["name": "fixture", "present": present, "confidence": 0.75,
         "segment_ids": [1], "arc_length_px": 140, "chord_length_px": 120,
         "tortuosity": 1.2, "mean_angle_deg": 45, "start_zone": "a", "end_zone": "b",
         "gap_count": 0, "fork_count": 0, "mean_width_px": 2, "mean_intensity": 100,
         "points_image_norm": points]
    }
}
