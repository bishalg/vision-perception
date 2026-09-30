import XCTest
import CurvAsdGazeKit
@testable import AsdGazeCollector

final class GazeSessionRecordVersioningTests: XCTestCase {

    private func makeV14JSON() -> Data {
        // A v1.4 record as persisted before the per-stimulus matrix (no perVideo fields).
        let json: [String: Any] = [
            "id": "rec-1",
            "sessionId": "SES-14",
            "participantId": "STUDY-ASD-TEST01",
            "startedAt": 7200.0,
            "endedAt": 10800.0,
            "durationSec": 30.0,
            "socialPreferenceScore": 0.42,
            "validFrameRatio": 0.91,
            "eyeDwellRatio": 0.31,
            "mouthDwellRatio": 0.12,
            "firstEyeLatencyMs": 430.0,
            "calibratedDistanceCm": 45.0,
            "qualityVerdict": "optimal",
            "socialSmileIntensity": 0.51,
            "smileReactivityIndex": 0.34,
            "flatAffectScore": 0.10,
            "surpriseScoreMean": 0.20,
            "notes": "legacy",
            "schemaVersion": "v1.4",
            "zeroRawVideoEnforced": true
        ]
        return try! JSONSerialization.data(withJSONObject: json)
    }

    func testV14RecordStillDecodes() throws {
        let record = try JSONDecoder().decode(GazeSessionRecord.self, from: makeV14JSON())
        XCTAssertEqual(record.schemaVersion, "v1.4")
        XCTAssertNil(record.perVideo)
        XCTAssertNil(record.perVideoComposite)
        XCTAssertEqual(record.socialPreferenceScore, 0.42, accuracy: 1e-9)
    }

    func testV15RecordRoundTripWithPerVideoMatrix() throws {
        let report = PerVideoScoreReport(
            stimulusId: "stim_05_geopref_split_10s",
            title: "Geometric vs Social Preference",
            category: "GeoPref",
            rows: [
                PerVideoScoreRow(
                    segmentId: "social_geo_split",
                    segmentDescription: "Split screen",
                    biomarker: "social_dwell_ratio",
                    targetAOI: nil,
                    metricKey: "social_dwell_ratio",
                    direction: .higherBetter,
                    observed: 0.82,
                    expected: 0.70,
                    acceptableLow: 0.55,
                    acceptableHigh: nil,
                    status: .onTarget,
                    score: 1.0,
                    imputed: false,
                    citation: "Pierce et al., eLife 2019",
                    source: "literature"
                )
            ],
            subscore: 1.0,
            validSampleRatio: 0.95,
            lowConfidence: false,
            hasBaseline: true,
            weight: 0.30
        )
        var record = GazeSessionRecord(
            summary: GazeSessionReportSummary(
                sessionId: "SES-15",
                durationSec: 32.0,
                totalFrames: 1920,
                validFrames: 1824,
                validRatio: 0.95,
                socialPreferenceScore: 0.62,
                socialDwellSec: 12.4,
                geometricDwellSec: 2.1
            ),
            participantId: "STUDY-ASD-TEST01",
            perVideoComposite: 1.0,
            perVideo: [report]
        )
        XCTAssertFalse(record.schemaVersion.isEmpty)
        XCTAssertTrue(record.schemaVersion.hasPrefix("v1."))

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .secondsSince1970
        let data = try encoder.encode([record])

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        let decoded = try decoder.decode([GazeSessionRecord].self, from: data).first!
        // Field-level assertions (whole-struct equality is float-noise sensitive).
        XCTAssertEqual(decoded.sessionId, record.sessionId)
        XCTAssertEqual(decoded.startedAt.timeIntervalSince1970, record.startedAt.timeIntervalSince1970, accuracy: 1e-6)
        XCTAssertEqual(decoded.perVideoComposite, 1.0)
        let row = try XCTUnwrap(decoded.perVideo?.first?.rows.first)
        XCTAssertEqual(row.citation, "Pierce et al., eLife 2019")
        XCTAssertEqual(row.observed, 0.82, accuracy: 1e-9)
        XCTAssertEqual(row.status, .onTarget)
        XCTAssertFalse(row.imputed)
        XCTAssertEqual(decoded.perVideo?.first?.hasBaseline, true)
    }

    func testNormCollectionOptInDecodesLegacyEnrollmentWithoutTheKey() throws {
        let legacy = """
        {"participantId":"STUDY-ASD-TEST01","isConsented":true,"childAgeMonths":24,"cloudSyncOptIn":true,"totalScreeningsSynced":3}
        """.data(using: .utf8)!
        let enrollment = try JSONDecoder().decode(ResearchStudyEnrollment.self, from: legacy)
        XCTAssertNil(enrollment.normCollectionOptIn)
        XCTAssertTrue(enrollment.isConsented)
    }

    func testNormCollectionGateRequiresConsentOptInAndLiveCapture() {
        var enrollment = ResearchStudyEnrollment()
        enrollment.isConsented = true
        enrollment.normCollectionOptIn = true

        XCTAssertTrue(NormCollectionGate.isActive(enrollment: enrollment, isSimulationMode: false))
        // Simulation sessions must never enter a normative distribution.
        XCTAssertFalse(NormCollectionGate.isActive(enrollment: enrollment, isSimulationMode: true))

        enrollment.normCollectionOptIn = false
        XCTAssertFalse(NormCollectionGate.isActive(enrollment: enrollment, isSimulationMode: false))

        enrollment.normCollectionOptIn = true
        enrollment.isConsented = false
        XCTAssertFalse(NormCollectionGate.isActive(enrollment: enrollment, isSimulationMode: false))

        // Gate closed -> no metadata payload.
        XCTAssertNil(NormCollectionGate.collectionMetadata(enrollment: enrollment, childAgeMonths: 24, isSimulationMode: false))
        enrollment.isConsented = true
        let metadata = NormCollectionGate.collectionMetadata(enrollment: enrollment, childAgeMonths: 24, isSimulationMode: false)
        XCTAssertNotNil(metadata)
        XCTAssertEqual(metadata?.childAgeMonths, 24)
        XCTAssertFalse(metadata?.isSimulator ?? true)
    }
}
