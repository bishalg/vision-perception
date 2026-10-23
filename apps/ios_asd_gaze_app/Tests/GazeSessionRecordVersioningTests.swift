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
        let record = GazeSessionRecord(
            summary: .fixture(
                sessionId: "SES-15",
                durationSec: 32.0,
                totalFrames: 1920,
                validFrames: 1824,
                validRatio: 0.95,
                socialPreferenceScore: 0.62,
                socialDwellSec: 12.4,
                geometricDwellSec: 2.1
            ),
            participantToken: "STUDY-ASD-TEST01",
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

        XCTAssertTrue(NormCollectionGate.isActive(enrollment: enrollment, captureSource: .trueDepth))
        // Simulation sessions must never enter a normative distribution.
        XCTAssertFalse(NormCollectionGate.isActive(enrollment: enrollment, captureSource: .simulated))

        enrollment.normCollectionOptIn = false
        XCTAssertFalse(NormCollectionGate.isActive(enrollment: enrollment, captureSource: .trueDepth))

        enrollment.normCollectionOptIn = true
        enrollment.isConsented = false
        XCTAssertFalse(NormCollectionGate.isActive(enrollment: enrollment, captureSource: .trueDepth))

        // Gate closed -> no metadata payload.
        XCTAssertNil(NormCollectionGate.collectionMetadata(enrollment: enrollment, childAgeMonths: 24, captureSource: .trueDepth))
        enrollment.isConsented = true
        let metadata = NormCollectionGate.collectionMetadata(enrollment: enrollment, childAgeMonths: 24, captureSource: .trueDepth)
        XCTAssertNotNil(metadata)
        XCTAssertEqual(metadata?.childAgeMonths, 24)
        XCTAssertFalse(metadata?.isSimulator ?? true)
        XCTAssertNil(
            NormCollectionGate.collectionMetadata(enrollment: enrollment, childAgeMonths: 24, captureSource: .simulated),
            "a simulated capture never yields a norm-collection payload"
        )
    }

    // MARK: - SIM-4: provenance and schema v1.6

    private func legacyJSON(
        schemaVersion: String,
        zeroRawVideoEnforced: Bool?,
        extra: [String: Any] = [:]
    ) -> Data {
        var json: [String: Any] = [
            "id": "rec-legacy",
            "sessionId": "SES-LEG",
            "participantId": "child-SESLEG00",
            "startedAt": 7200.0,
            "endedAt": 10800.0,
            "durationSec": 30.0,
            "socialPreferenceScore": 0.42,
            "validFrameRatio": 0.91,
            "eyeDwellRatio": 0.28,
            "mouthDwellRatio": 0.14,
            "firstEyeLatencyMs": 480.0,
            "calibratedDistanceCm": 45.0,
            "qualityVerdict": "optimal",
            "socialSmileIntensity": 0.51,
            "smileReactivityIndex": 0.34,
            "flatAffectScore": 0.10,
            "surpriseScoreMean": 0.20,
            "schemaVersion": schemaVersion
        ]
        if let zeroRawVideoEnforced { json["zeroRawVideoEnforced"] = zeroRawVideoEnforced }
        json.merge(extra) { _, new in new }
        return try! JSONSerialization.data(withJSONObject: json)
    }

    func testV14RecordDecodesAsLegacyTrueDepth() throws {
        let record = try JSONDecoder().decode(GazeSessionRecord.self, from: makeV14JSON())
        XCTAssertEqual(record.captureSource, .trueDepth)
        XCTAssertEqual(record.legacyProvenance, true)
    }

    func testV15RecordDecodesAsLegacyTrueDepth() throws {
        let data = legacyJSON(schemaVersion: "v1.5", zeroRawVideoEnforced: true)
        let record = try JSONDecoder().decode(GazeSessionRecord.self, from: data)
        XCTAssertEqual(record.captureSource, .trueDepth)
        XCTAssertEqual(record.legacyProvenance, true)
        // Legacy metric values are kept as recorded; the legacy flag lets exports label them.
        XCTAssertEqual(record.firstEyeLatencyMs, 480.0)
    }

    func testLegacyRecordWithoutZeroRawVideoGuaranteeIsNotTrustedAsLive() throws {
        let data = legacyJSON(schemaVersion: "v1.5", zeroRawVideoEnforced: false)
        let record = try JSONDecoder().decode(GazeSessionRecord.self, from: data)
        XCTAssertEqual(record.captureSource, .simulated, "unknown provenance must be quarantined, never assumed live")
        XCTAssertEqual(record.legacyProvenance, true)
    }

    func testV16RecordWithoutCaptureSourceFailsToDecode() {
        let data = legacyJSON(schemaVersion: "v1.6", zeroRawVideoEnforced: true)
        XCTAssertThrowsError(try JSONDecoder().decode(GazeSessionRecord.self, from: data))
    }

    func testV16RecordWithCaptureSourceIsNotLegacy() throws {
        let data = legacyJSON(schemaVersion: "v1.6", zeroRawVideoEnforced: true, extra: ["captureSource": "simulated"])
        let record = try JSONDecoder().decode(GazeSessionRecord.self, from: data)
        XCTAssertEqual(record.captureSource, .simulated)
        XCTAssertNil(record.legacyProvenance)
    }

    func testRecordFromSummaryCarriesProvenanceAndAbsentMetrics() throws {
        let summary = GazeSessionReportSummary.fixture(sessionId: "SES-16", captureSource: .simulated)
        let record = GazeSessionRecord(summary: summary, participantToken: "child-SES-16")
        XCTAssertEqual(record.schemaVersion, "v1.6")
        XCTAssertEqual(record.schemaVersion, GazeSessionRecord.currentSchemaVersion)
        XCTAssertEqual(record.captureSource, .simulated)
        XCTAssertNil(record.legacyProvenance)
        XCTAssertNil(record.eyeDwellRatio)
        XCTAssertNil(record.mouthDwellRatio)
        XCTAssertNil(record.firstEyeLatencyMs)

        let data = try JSONEncoder().encode(record)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(json["captureSource"] as? String, "simulated")
        XCTAssertNil(json["firstEyeLatencyMs"], "absent metrics are absent keys, never a default")
        XCTAssertNil(json["eyeDwellRatio"])
        XCTAssertNil(json["mouthDwellRatio"])

        let decoded = try JSONDecoder().decode(GazeSessionRecord.self, from: data)
        XCTAssertEqual(decoded.captureSource, .simulated)
        XCTAssertNil(decoded.firstEyeLatencyMs)
        XCTAssertNil(decoded.legacyProvenance)
    }

    func testRecordFromSummaryKeepsMeasuredDwellRatios() {
        let summary = GazeSessionReportSummary.fixture(eyeDwellRatio: 0.7, mouthDwellRatio: 0.3)
        let record = GazeSessionRecord(summary: summary, participantToken: "child-TEST0000")
        XCTAssertEqual(record.eyeDwellRatio, 0.7)
        XCTAssertEqual(record.mouthDwellRatio, 0.3)
        XCTAssertEqual(record.captureSource, .trueDepth)
    }

    // MARK: - SIM-5: history store ignores simulated sessions

    @MainActor
    func testHistoryStoreIgnoresSimulatedRecords() {
        let suite = "GazeHistoryTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }

        let store = GazeSessionHistoryStore(defaults: defaults)
        store.record(.fixture(sessionId: "SIM-1", captureSource: .simulated))
        XCTAssertTrue(store.records.isEmpty, "simulated sessions never enter the caregiver history")

        store.record(.fixture(sessionId: "LIVE-1", captureSource: .trueDepth))
        XCTAssertEqual(store.records.map(\.sessionId), ["LIVE-1"])

        let reloaded = GazeSessionHistoryStore(defaults: defaults)
        XCTAssertEqual(reloaded.records.map(\.sessionId), ["LIVE-1"])
    }

    @MainActor
    func testHistoryStoreDropsSimulatedRecordsPersistedEarlier() throws {
        let suite = "GazeHistoryTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }

        let persisted = [
            GazeSessionRecord.fixture(sessionId: "LIVE-A"),
            GazeSessionRecord.fixture(sessionId: "SIM-A", captureSource: .simulated)
        ]
        defaults.set(try JSONEncoder().encode(persisted), forKey: "org.curv.asd.gaze_session_history")

        let store = GazeSessionHistoryStore(defaults: defaults)
        XCTAssertEqual(store.records.map(\.sessionId), ["LIVE-A"])
    }

    // MARK: - CNS-7: pseudonymous participant token

    func testParticipantTokenUsesEnrollmentIdOnlyWhenConsented() {
        var enrollment = ResearchStudyEnrollment(participantId: "STUDY-ASD-ABC123")
        XCTAssertEqual(
            GazeSessionRecord.participantToken(enrollment: enrollment, sessionId: "1234567890ABCDEF"),
            "child-12345678"
        )
        enrollment.isConsented = true
        XCTAssertEqual(
            GazeSessionRecord.participantToken(enrollment: enrollment, sessionId: "1234567890ABCDEF"),
            "STUDY-ASD-ABC123"
        )
        XCTAssertEqual(
            GazeSessionRecord.participantToken(enrollment: nil, sessionId: "ABCD"),
            "child-ABCD"
        )
    }

    func testRecordBuiltWithTokenNeverContainsChildName() throws {
        let childName = "Aarav Sharma"
        let summary = GazeSessionReportSummary.fixture(sessionId: "SESS-NAME-001")
        let token = GazeSessionRecord.participantToken(enrollment: nil, sessionId: summary.sessionId)
        let record = GazeSessionRecord(summary: summary, participantToken: token)

        let json = String(data: try JSONEncoder().encode(record), encoding: .utf8) ?? ""
        XCTAssertFalse(json.contains(childName))
        XCTAssertFalse(json.contains("Aarav"))
        XCTAssertEqual(record.participantId, "child-SESS-NAM")

        let fhir = try XCTUnwrap(record.toFHIRObservation())
        let fhirJSON = String(data: try JSONEncoder().encode(fhir), encoding: .utf8) ?? ""
        XCTAssertFalse(fhirJSON.contains("Aarav"))
        XCTAssertTrue(fhirJSON.contains("Patient\\/child-SESS-NAM") || fhirJSON.contains("Patient/child-SESS-NAM"))
    }
}
