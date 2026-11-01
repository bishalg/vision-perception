@testable import AsdGazeCollector
import CurvAsdGazeKit
import Foundation
import XCTest

/// WP-A.4 / WP-G.9 (SCH-2): the `POST /v1/sessions` body built from a `GazeSessionRecord`
/// matches the session contract 1.7 (`docs/spec/schemas/asd/session-v1.7.json`) and the
/// golden `Tests/Fixtures/expected-session-v1.7.json` (the 1.6 golden plus `trackingQuality`
/// and the per-clip tracking keys).
final class SessionIngestPayloadTests: XCTestCase {

    // MARK: - Fixture

    /// Fixed dates and ids so the golden file is deterministic.
    static func goldenRecord() -> GazeSessionRecord {
        let rows = [
            PerVideoScoreRow(
                segmentId: "social_geo_split",
                segmentDescription: "Split screen",
                biomarker: "social_dwell_ratio",
                targetAOI: nil,
                metricKey: "social_dwell_ratio",
                direction: .higherBetter,
                observed: 0.62,
                expected: 0.70,
                acceptableLow: 0.55,
                acceptableHigh: nil,
                status: .onTarget,
                score: 1.0,
                imputed: false,
                citation: "Pierce et al., eLife 2019",
                source: "literature"
            ),
            PerVideoScoreRow(
                segmentId: "social_geo_split",
                segmentDescription: "Split screen",
                biomarker: "geometric_dwell_ratio",
                targetAOI: nil,
                metricKey: "geometric_dwell_ratio",
                direction: .lowerBetter,
                observed: 0.21,
                expected: 0.25,
                acceptableLow: nil,
                acceptableHigh: 0.40,
                status: .onTarget,
                score: 1.0,
                imputed: false,
                citation: "Pierce et al., eLife 2019",
                source: "literature"
            ),
            PerVideoScoreRow(
                segmentId: "faces",
                segmentDescription: "Face region",
                biomarker: "eye_dwell_ratio",
                targetAOI: nil,
                metricKey: "eye_dwell_ratio",
                direction: .higherBetter,
                observed: 0.33,
                expected: 0.40,
                acceptableLow: 0.30,
                acceptableHigh: nil,
                status: .monitor,
                score: 0.7,
                imputed: false,
                citation: "Jones & Klin, Nature 2013",
                source: "literature"
            ),
            PerVideoScoreRow(
                segmentId: "cue",
                segmentDescription: "Gaze cue",
                biomarker: "first_entry_latency_ms",
                targetAOI: "Gaze_Target_Right",
                metricKey: "first_entry_latency_ms::Gaze_Target_Right",
                direction: .lowerBetter,
                observed: 10_000,
                expected: 800,
                acceptableLow: nil,
                acceptableHigh: 1500,
                status: .atypical,
                score: 0.0,
                imputed: true,
                citation: "provisional",
                source: "provisional"
            ),
            PerVideoScoreRow(
                segmentId: "whole_clip",
                segmentDescription: "Whole clip",
                biomarker: "fixation_count",
                targetAOI: nil,
                metricKey: "fixation_count",
                direction: .band,
                observed: 14,
                expected: 12,
                acceptableLow: 6,
                acceptableHigh: 20,
                status: .onTarget,
                score: 1.0,
                imputed: false,
                citation: "provisional",
                source: "provisional"
            ),
            PerVideoScoreRow(
                segmentId: "whole_clip",
                segmentDescription: "Whole clip",
                biomarker: "mean_fixation_duration_ms",
                targetAOI: nil,
                metricKey: "mean_fixation_duration_ms",
                direction: .band,
                observed: 280,
                expected: 300,
                acceptableLow: 150,
                acceptableHigh: 500,
                status: .onTarget,
                score: 1.0,
                imputed: false,
                citation: "provisional",
                source: "provisional"
            )
        ]
        let clipA = PerVideoScoreReport(
            stimulusId: "stim_05_geopref_split_10s",
            title: "Geometric vs Social Preference",
            category: "GeoPref",
            rows: rows,
            subscore: 0.74,
            validSampleRatio: 0.95,
            lowConfidence: false,
            hasBaseline: true,
            weight: 0.30
        ).withTracking(ClipTracking(
            meanSampleConfidence: 0.84,
            meanFixationConfidence: 0.88,
            nearestCheckOffsetDeg: 1.1,
            aoiMinSizeDeg: 4.6
        ))
        let clipB = PerVideoScoreReport(
            stimulusId: "untagged",
            title: "Untagged frames",
            category: "Other",
            rows: [],
            subscore: 0,
            validSampleRatio: 0.40,
            lowConfidence: true,
            hasBaseline: false,
            weight: 0
        ).withTracking(ClipTracking(meanSampleConfidence: 0.41))
        return GazeSessionRecord(
            id: "rec-golden-0001",
            sessionId: "SESS-GOLDEN-0001",
            participantId: "child-SESS-GOL",
            captureSource: .trueDepth,
            startedAt: Date(timeIntervalSince1970: 1_793_000_000),
            endedAt: Date(timeIntervalSince1970: 1_793_000_032.5),
            durationSec: 32.5,
            socialPreferenceScore: 0.62,
            validFrameRatio: 0.93,
            eyeDwellRatio: 0.33,
            mouthDwellRatio: nil,
            firstEyeLatencyMs: nil,
            calibratedDistanceCm: 38.0,
            qualityVerdict: "optimal",
            socialSmileIntensity: 0.41,
            smileReactivityIndex: 0.22,
            flatAffectScore: 0.12,
            surpriseScoreMean: 0.18,
            perVideoComposite: 0.74,
            perVideo: [clipA, clipB],
            trackingQuality: goldenTrackingQuality
        )
    }

    /// The QS-1 record of the golden session. The app never builds one: it carries the Rust
    /// pipeline's document verbatim, so the fixture's own record is the input here.
    static let goldenTrackingQuality: JSONValue = {
        // swiftlint:disable:next force_try
        let data = try! Data(contentsOf: try! FHIRFixtures.url("expected-session-v1.7.json"))
        let object = try! JSONSerialization.jsonObject(with: data) as! [String: Any]
        let raw = object["rawPayload"] as! [String: Any]
        let quality = try! JSONSerialization.data(withJSONObject: raw["trackingQuality"]!)
        return try! JSONDecoder().decode(JSONValue.self, from: quality)
    }()

    private func encodedObject(_ payload: SessionIngestPayload) throws -> [String: Any] {
        let data = try SessionPayloadCoding.encoder().encode(payload)
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    // MARK: - Contract against the WP-D.2 JSON Schema

    func testPayloadValidatesAgainstExportedSessionSchema() throws {
        let schema = try SessionSchemaFile.load()
        let payload = try SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: .yellow)
        let object = try encodedObject(payload)

        let errors = SessionSchemaFile.validate(object, against: schema, root: schema)
        XCTAssertEqual(errors, [], "payload must validate against docs/spec/schemas/asd/session-v1.7.json")
    }

    func testPayloadSendsOnlyKeysTheSchemaDeclares() throws {
        let schema = try SessionSchemaFile.load()
        let properties = try XCTUnwrap(schema["properties"] as? [String: Any])
        let object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil))
        XCTAssertEqual(Set(object.keys).subtracting(properties.keys), [], "unknown top-level keys")

        let defs = try XCTUnwrap(schema["$defs"] as? [String: Any])
        let row = try XCTUnwrap(defs["SessionPerVideoScore"] as? [String: Any])
        let rowProperties = try XCTUnwrap(row["properties"] as? [String: Any])
        let rows = try XCTUnwrap(object["perVideo"] as? [[String: Any]])
        for clip in rows {
            XCTAssertEqual(Set(clip.keys).subtracting(rowProperties.keys), [], "unknown per-video keys")
        }
    }

    func testSchemaValidatorRejectsAPayloadWithoutSessionId() throws {
        // Guards the guard: the Swift validator must fail a malformed body.
        let schema = try SessionSchemaFile.load()
        var object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil))
        object.removeValue(forKey: "sessionId")
        object["durationSec"] = "thirty"
        let errors = SessionSchemaFile.validate(object, against: schema, root: schema)
        XCTAssertTrue(errors.contains("missing required sessionId"), "\(errors)")
        XCTAssertTrue(errors.contains { $0.hasPrefix("durationSec") }, "\(errors)")
    }

    func testPayloadMatchesGoldenFile() throws {
        let payload = try SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: .yellow)
        let actual = try SessionPayloadCoding.encoder().encode(payload)
        let expected = try Data(contentsOf: try FHIRFixtures.url("expected-session-v1.7.json"))

        let actualObject = try JSONSerialization.jsonObject(with: actual) as? NSDictionary
        let expectedObject = try JSONSerialization.jsonObject(with: expected) as? NSDictionary
        XCTAssertNotNil(expectedObject)
        if actualObject != expectedObject {
            let out = FileManager.default.temporaryDirectory.appendingPathComponent("actual-session-v1.7.json")
            try? actual.write(to: out)
            XCTFail("payload differs from Tests/Fixtures/expected-session-v1.7.json; actual written to \(out.path)")
        }
    }

    /// CON-7: the body asd-sync sends is the Swift payload document (decoded with the
    /// contract types and re-encoded canonically): same keys and values, nothing dropped
    /// (rawPayload keeps fields the contract does not name), dates unchanged.
    func testSessionRequestBodyIsTheSwiftPayloadInCanonicalForm() throws {
        let client = SessionSyncClient(
            apiBaseURL: URL(string: "https://api.example.org")!,
            transport: StubSyncTransport { _ in (200, [:], Data()) },
            registration: DeviceRegistrationService(
                apiBaseURL: URL(string: "https://api.example.org")!,
                transport: StubSyncTransport { _ in (200, [:], Data()) },
                store: InMemoryDeviceCredentialStore(),
                clientInfo: SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")
            ),
            clientInfo: SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")
        )
        for flag in [AttentionFlag.yellow, nil] {
            let payload = try SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: flag)
            let request = try client.makeSessionRequest(payload, token: "tok")
            let body = try XCTUnwrap(request.httpBody)
            let sent = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? NSDictionary)
            let swift = try XCTUnwrap(JSONSerialization.jsonObject(with: SessionPayloadCoding.encoder().encode(payload)) as? NSDictionary)
            XCTAssertEqual(sent, swift)
            XCTAssertEqual(request.url?.absoluteString, "https://api.example.org/v1/sessions")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.timeoutInterval, 20)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            // The queue decodes what it stored and asd-sync re-encodes it: still the same document.
            let reloaded = try SessionPayloadCoding.decoder().decode(SessionIngestPayload.self, from: body)
            XCTAssertEqual(reloaded, payload)
        }
    }

    func testEmptyTokenIsNotRegistered() throws {
        let client = SessionSyncClient(
            apiBaseURL: URL(string: "https://api.example.org")!,
            transport: StubSyncTransport { _ in (200, [:], Data()) },
            registration: DeviceRegistrationService(
                apiBaseURL: URL(string: "https://api.example.org")!,
                transport: StubSyncTransport { _ in (200, [:], Data()) },
                store: InMemoryDeviceCredentialStore(),
                clientInfo: SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")
            ),
            clientInfo: SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")
        )
        let payload = try SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil)
        XCTAssertThrowsError(try client.makeSessionRequest(payload, token: "  ")) { error in
            XCTAssertEqual(error as? SessionSyncError, .notRegistered)
        }
    }

    // MARK: - Field mapping

    func testTopLevelFieldsMapFromRecord() throws {
        let object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: .green))
        XCTAssertEqual(object["sessionId"] as? String, "SESS-GOLDEN-0001")
        XCTAssertEqual(object["participantToken"] as? String, "child-SESS-GOL")
        XCTAssertEqual(object["captureSource"] as? String, "truedepth")
        XCTAssertEqual(object["validRatio"] as? Double, 0.93)
        XCTAssertEqual(object["durationSec"] as? Double, 32.5)
        XCTAssertEqual(object["calibratedDistanceCm"] as? Double, 38.0)
        XCTAssertEqual(object["qualityVerdict"] as? String, "optimal")
        XCTAssertEqual(object["attentionFlag"] as? String, "green")
        XCTAssertEqual(object["schemaVersion"] as? String, "1.7")
    }

    func testWireSchemaVersionDropsTheRecordPrefix() {
        XCTAssertEqual(SessionIngestPayload.wireSchemaVersion("v1.6"), "1.6")
        XCTAssertEqual(SessionIngestPayload.wireSchemaVersion("1.6"), "1.6")
        XCTAssertEqual(SessionIngestPayload.wireSchemaVersion(GazeSessionRecord.currentSchemaVersion), "1.7")
    }

    func testUnmeasuredMetricsAreAbsentKeysNotDefaults() throws {
        let object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil))
        XCTAssertNil(object["mouthDwellRatio"])
        XCTAssertNil(object["firstEyeLatencyMs"])
        XCTAssertNil(object["socialDwellSec"])
        XCTAssertNil(object["geometricDwellSec"])
        XCTAssertEqual(object["eyeDwellRatio"] as? Double, 0.33)
    }

    /// The backend defaults an absent `attentionFlag` to "green" and an absent `battery`
    /// to "express_30s". The client sends explicit `null` so nothing is assumed.
    func testUnknownAttentionFlagAndBatteryAreExplicitNull() throws {
        let object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil))
        XCTAssertTrue(object.keys.contains("attentionFlag"))
        XCTAssertTrue(object["attentionFlag"] is NSNull)
        XCTAssertTrue(object.keys.contains("battery"))
        XCTAssertTrue(object["battery"] is NSNull)
    }

    func testPerVideoRowsAreFlattenedFromTheScoreMatrix() throws {
        let object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil))
        let rows = try XCTUnwrap(object["perVideo"] as? [[String: Any]])
        XCTAssertEqual(rows.count, 2)

        let first = rows[0]
        XCTAssertEqual(first["clip_id"] as? String, "stim_05_geopref_split_10s")
        XCTAssertEqual(first["order"] as? Int, 1)
        XCTAssertEqual(first["dwell_social"] as? Double, 0.62)
        XCTAssertEqual(first["dwell_geometric"] as? Double, 0.21)
        XCTAssertEqual(first["dwell_eye"] as? Double, 0.33)
        XCTAssertNil(first["dwell_mouth"], "not measured: absent, never 0")
        XCTAssertNil(first["orienting_latency_ms"], "imputed latency is not sent as a measurement")
        XCTAssertEqual(first["fixation_count"] as? Int, 14)
        XCTAssertEqual(first["mean_fixation_ms"] as? Double, 280)
        XCTAssertEqual(first["status"] as? String, "atypical", "worst row status")
        XCTAssertEqual(first["score"] as? Double, 0.74)
        XCTAssertEqual(first["weight"] as? Double, 0.30)
        let details = try XCTUnwrap(first["details"] as? [String: Any])
        XCTAssertEqual(details["dwellUnits"] as? String, "ratio")
        XCTAssertEqual((details["rows"] as? [Any])?.count, 6)

        let second = rows[1]
        XCTAssertEqual(second["clip_id"] as? String, "untagged")
        XCTAssertEqual(second["order"] as? Int, 2)
        XCTAssertNil(second["status"], "no scored rows: no status is invented")
        XCTAssertEqual((second["details"] as? [String: Any])?["hasBaseline"] as? Bool, false)
    }

    func testRawPayloadCarriesTheFullRecordWithISODates() throws {
        let record = Self.goldenRecord()
        let object = try encodedObject(SessionIngestPayload(record: record, attentionFlag: nil))
        let raw = try XCTUnwrap(object["rawPayload"] as? [String: Any])
        XCTAssertEqual(raw["sessionId"] as? String, record.sessionId)
        XCTAssertEqual(raw["schemaVersion"] as? String, "v1.7")
        XCTAssertEqual(raw["captureSource"] as? String, "truedepth")
        XCTAssertEqual(raw["startedAt"] as? String, "2026-10-26T07:33:20.000Z")
        XCTAssertNotNil(raw["perVideo"])
    }

    func testPayloadRoundTripsThroughTheQueueCoding() throws {
        let payload = try SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: .red)
        let data = try SessionPayloadCoding.encoder().encode(payload)
        let decoded = try SessionPayloadCoding.decoder().decode(SessionIngestPayload.self, from: data)
        XCTAssertEqual(decoded.sessionId, payload.sessionId)
        XCTAssertEqual(decoded.attentionFlag, "red")
        XCTAssertEqual(decoded.rawPayload.startedAt, payload.rawPayload.startedAt)
        XCTAssertEqual(decoded.perVideo?.count, 2)
    }

    // MARK: - 1.7 tracking (SCH-2, QS-1, QS-3, QS-5)

    func testPerClipTrackingIsInDetailsAndRawPayload() throws {
        let object = try encodedObject(SessionIngestPayload(record: Self.goldenRecord(), attentionFlag: nil))
        let rows = try XCTUnwrap(object["perVideo"] as? [[String: Any]])
        let details = try XCTUnwrap(rows[0]["details"] as? [String: Any])
        XCTAssertEqual(details["meanSampleConfidence"] as? Double, 0.84)
        XCTAssertEqual(details["aoiMinSizeDeg"] as? Double, 4.6)
        let second = try XCTUnwrap(rows[1]["details"] as? [String: Any])
        XCTAssertNil(second["nearestCheckOffsetDeg"], "not measured: absent")
        let raw = try XCTUnwrap(object["rawPayload"] as? [String: Any])
        let rawClips = try XCTUnwrap(raw["perVideo"] as? [[String: Any]])
        XCTAssertEqual(rawClips[0]["meanFixationConfidence"] as? Double, 0.88)
        XCTAssertNotNil((raw["trackingQuality"] as? [String: Any])?["verdict"])
    }

    /// QS-5 / acceptance 16: a record built from a session without its own calibration has no
    /// `calibratedDistanceCm` at all (no constant 35 cm).
    func testRecordFromSummaryWithoutCalibrationHasNoDistance() throws {
        let summary = GazeSessionReportSummary.fixture(sessionId: "NO-CAL")
        let record = GazeSessionRecord(summary: summary, participantToken: "child-NO-CAL")
        XCTAssertNil(record.calibratedDistanceCm)
        XCTAssertEqual(record.schemaVersion, "v1.7")
        let object = try encodedObject(SessionIngestPayload(record: record, attentionFlag: nil))
        XCTAssertNil(object["calibratedDistanceCm"])
        XCTAssertNil((object["rawPayload"] as? [String: Any])?["calibratedDistanceCm"])
        XCTAssertEqual(SessionSchemaFile.validate(object, against: try SessionSchemaFile.load(), root: try SessionSchemaFile.load()), [])
    }

    /// The summary's pipeline documents reach the record: trackingQuality verbatim, the
    /// distance from the calibration pose, and each clip's values on its score report.
    func testRecordFromSummaryCarriesThePipelineDocuments() throws {
        let quality = try JSONValue(json: Self.goldenTrackingQuality.jsonString())
        let summary = GazeSessionReportSummary.fixture(
            sessionId: "WITH-CAL",
            trackingQuality: quality,
            clipTracking: [ClipTrackingEntry(clipId: "stim_05_geopref_split_10s", order: 1,
                                             tracking: ClipTracking(meanSampleConfidence: 0.7))]
        )
        let golden = Self.goldenRecord()
        let record = GazeSessionRecord(
            summary: summary,
            participantToken: "child-WITH-CAL",
            perVideoComposite: golden.perVideoComposite,
            perVideo: golden.perVideo?.map { r in
                PerVideoScoreReport(stimulusId: r.stimulusId, title: r.title, category: r.category, rows: r.rows,
                                    subscore: r.subscore, validSampleRatio: r.validSampleRatio,
                                    lowConfidence: r.lowConfidence, hasBaseline: r.hasBaseline, weight: r.weight)
            }
        )
        XCTAssertEqual(record.trackingQuality, quality)
        XCTAssertEqual(record.calibratedDistanceCm, 38, "the fixture's calibration pose distance")
        XCTAssertEqual(record.perVideo?[0].meanSampleConfidence, 0.7)
        XCTAssertNil(record.perVideo?[1].meanSampleConfidence)
    }

    // MARK: - Refusals (SIM-5)

    func testSimulatedRecordIsRefused() {
        let record = GazeSessionRecord.fixture(sessionId: "SIM-REFUSE", captureSource: .simulated)
        XCTAssertThrowsError(try SessionIngestPayload(record: record, attentionFlag: nil)) { error in
            XCTAssertEqual(error as? SessionPayloadError, .simulatedCapture)
        }
    }

    func testLegacyProvenanceRecordIsRefused() {
        let record = GazeSessionRecord.fixture(sessionId: "LEGACY-REFUSE", legacyProvenance: true)
        XCTAssertThrowsError(try SessionIngestPayload(record: record, attentionFlag: nil)) { error in
            XCTAssertEqual(error as? SessionPayloadError, .legacyProvenance)
        }
    }
}

// MARK: - Minimal JSON Schema reader for the WP-D.2 contract

/// Reads the exported schema from the source tree (the simulator can read it) and checks
/// the subset of JSON Schema the Pydantic export uses: `type`, `anyOf`, `$ref`, `items`,
/// `required`, `properties`.
enum SessionSchemaFile {
    struct Missing: Error { let path: String }

    static var url: URL {
        // Tests/<file> -> apps/ios_asd_gaze_app -> apps -> repo root
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("docs/spec/schemas/asd/session-v1.7.json")
    }

    static func load() throws -> [String: Any] {
        guard let data = try? Data(contentsOf: url),
              let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Missing(path: url.path)
        }
        return object
    }

    static func validate(_ value: Any, against schema: [String: Any], root: [String: Any], path: String = "") -> [String] {
        if let ref = schema["$ref"] as? String {
            let name = ref.components(separatedBy: "/").last ?? ""
            guard let defs = root["$defs"] as? [String: Any], let target = defs[name] as? [String: Any] else {
                return ["\(path): unresolved \(ref)"]
            }
            return validate(value, against: target, root: root, path: path)
        }
        if let anyOf = schema["anyOf"] as? [[String: Any]] {
            for option in anyOf where validate(value, against: option, root: root, path: path).isEmpty {
                return []
            }
            return ["\(path) matches no anyOf option"]
        }
        guard let type = schema["type"] as? String else { return [] }
        switch type {
        case "null":
            return value is NSNull ? [] : ["\(path) expected null"]
        case "string":
            return value is String ? [] : ["\(path) expected string"]
        case "boolean":
            return isBool(value) ? [] : ["\(path) expected boolean"]
        case "integer":
            guard let number = value as? NSNumber, !isBool(value) else { return ["\(path) expected integer"] }
            return number.doubleValue.rounded() == number.doubleValue ? [] : ["\(path) expected integer"]
        case "number":
            return (value is NSNumber && !isBool(value)) ? [] : ["\(path) expected number"]
        case "array":
            guard let array = value as? [Any] else { return ["\(path) expected array"] }
            guard let items = schema["items"] as? [String: Any] else { return [] }
            return array.enumerated().flatMap { index, item in
                validate(item, against: items, root: root, path: "\(path)[\(index)]")
            }
        case "object":
            guard let object = value as? [String: Any] else { return ["\(path) expected object"] }
            var errors: [String] = []
            for key in (schema["required"] as? [String]) ?? [] where object[key] == nil {
                errors.append("missing required \(path.isEmpty ? "" : path + ".")\(key)")
            }
            let properties = (schema["properties"] as? [String: Any]) ?? [:]
            for (key, child) in object.sorted(by: { $0.key < $1.key }) {
                guard let childSchema = properties[key] as? [String: Any] else { continue }
                errors += validate(child, against: childSchema, root: root, path: path.isEmpty ? key : "\(path).\(key)")
            }
            return errors
        default:
            return []
        }
    }

    private static func isBool(_ value: Any) -> Bool {
        guard let number = value as? NSNumber else { return false }
        return CFGetTypeID(number) == CFBooleanGetTypeID()
    }
}
