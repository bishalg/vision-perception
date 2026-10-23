import XCTest
@testable import CurvAsdGazeKit

/// SIM-1, SIM-4: capture provenance is a required, non-defaulted field that survives
/// JSON round-trips; legacy vault summaries still decode.
final class CaptureSourceTests: XCTestCase {
    func testRawValuesAreStable() {
        XCTAssertEqual(CaptureSource.trueDepth.rawValue, "truedepth")
        XCTAssertEqual(CaptureSource.simulated.rawValue, "simulated")
    }

    func testHeaderRoundTripsCaptureSource() throws {
        for source in [CaptureSource.trueDepth, .simulated] {
            let header = SessionHeader.fixture(captureSource: source)
            let data = try JSONEncoder().encode(header)
            let decoded = try JSONDecoder().decode(SessionHeader.self, from: data)
            XCTAssertEqual(decoded.captureSource, source)
            XCTAssertEqual(decoded.sessionId, header.sessionId)
        }
    }

    func testHeaderDecodeWithoutCaptureSourceFails() throws {
        let header = SessionHeader.fixture()
        var json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(header)) as? [String: Any]
        )
        XCTAssertNotNil(json.removeValue(forKey: "captureSource"))
        let stripped = try JSONSerialization.data(withJSONObject: json)
        XCTAssertThrowsError(try JSONDecoder().decode(SessionHeader.self, from: stripped))
    }

    func testSummaryRoundTripsCaptureSourceAndOptionalMetrics() throws {
        let summary = GazeSessionReportSummary.fixture(
            captureSource: .simulated,
            eyeDwellRatio: 0.4,
            mouthDwellRatio: nil,
            firstEyeLatencyMs: nil,
            interruptionCount: 2,
            interruptedSec: 3.5
        )
        let data = try JSONEncoder().encode(summary)
        let decoded = try JSONDecoder().decode(GazeSessionReportSummary.self, from: data)
        XCTAssertEqual(decoded, summary)
        XCTAssertEqual(decoded.captureSource, .simulated)
        XCTAssertNil(decoded.mouthDwellRatio)
        XCTAssertNil(decoded.firstEyeLatencyMs)
        XCTAssertEqual(decoded.interruptionCount, 2)
    }

    func testSummaryDecodeToleratesMissingOptionalMetrics() throws {
        // Shape of a summary line written by a build that predates this change minus
        // the placeholder metrics: no captureSource, no interruption fields, no
        // latency/dwell ratios, no subject-age signal.
        let legacy = """
        {"sessionId":"OLD00001","durationSec":12.5,"totalFrames":750,"validFrames":700,
         "validRatio":0.93,"socialPreferenceScore":0.2,"socialDwellSec":6.0,
         "geometricDwellSec":4.0,"socialSmileIntensityMean":0.3,"smileReactivityIndex":0.36,
         "flatAffectScore":0.1,"surpriseScoreMean":0.2,"qualityVerdict":"optimal"}
        """
        let decoded = try JSONDecoder().decode(GazeSessionReportSummary.self, from: Data(legacy.utf8))
        XCTAssertEqual(decoded.sessionId, "OLD00001")
        XCTAssertNil(decoded.firstEyeLatencyMs)
        XCTAssertNil(decoded.eyeDwellRatio)
        XCTAssertNil(decoded.mouthDwellRatio)
        XCTAssertNil(decoded.subjectAgeSignal)
        XCTAssertEqual(decoded.interruptionCount, 0)
        XCTAssertEqual(decoded.interruptedSec, 0)
        // Pre-provenance vault files can only have come from the live pipeline's
        // schema; same legacy rule as GazeSessionRecord (SIM-4).
        XCTAssertEqual(decoded.captureSource, .trueDepth)
    }

    func testSummaryDecodeRejectsMissingRequiredField() {
        let broken = #"{"sessionId":"X","qualityVerdict":"optimal"}"#
        XCTAssertThrowsError(try JSONDecoder().decode(GazeSessionReportSummary.self, from: Data(broken.utf8)))
    }

    func testTrackingStageUnavailableProperties() {
        let stage = TrackingStage.unavailable(reason: .faceTrackingUnsupported)
        XCTAssertFalse(stage.hasFace)
        XCTAssertFalse(stage.hasEyes)
        XCTAssertFalse(stage.isGazeActive)
        XCTAssertFalse(stage.label.isEmpty)
        XCTAssertNotEqual(stage, .unavailable(reason: .cameraPermissionDenied))
        XCTAssertEqual(TrackingUnavailableReason(rawValue: "interrupted"), .interrupted)
    }
}
