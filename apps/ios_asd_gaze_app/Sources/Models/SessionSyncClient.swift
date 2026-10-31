import AsdCore
import CurvAsdGazeKit
import Foundation
import os

// MARK: - Payload (`POST /v1/sessions`, schema `docs/spec/schemas/asd/session-v1.6.json`)

public enum SessionPayloadError: Error, Equatable, Sendable {
    /// Simulated sessions are refused by every sink (SIM-5).
    case simulatedCapture
    /// Pre-v1.6 records with inferred provenance are not uploaded as live data (SIM-4).
    case legacyProvenance
}

/// One flattened per-clip row (`SessionPerVideoScore` in the backend schema).
///
/// Metrics come from the scored rows of the clip's `PerVideoScoreReport`. A metric
/// the clip did not measure, or measured only by imputation, is an absent key: the
/// backend's numeric defaults are never relied on. Dwell values are **ratios** of
/// valid clip samples (`details.dwellUnits == "ratio"`).
public struct SessionPerVideoRow: Codable, Equatable, Sendable {
    public let clipId: String
    public let order: Int
    public let dwellSocial: Double?
    public let dwellGeometric: Double?
    public let dwellEye: Double?
    public let dwellMouth: Double?
    public let orientingLatencyMs: Double?
    public let fixationCount: Int?
    public let meanFixationMs: Double?
    /// Worst row status of the clip; absent when the clip has no scored rows.
    public let status: String?
    public let score: Double
    public let weight: Double
    public let details: Details

    public struct Details: Codable, Equatable, Sendable {
        public let title: String
        public let category: String
        public let subscore: Double
        public let validSampleRatio: Double
        public let lowConfidence: Bool
        public let hasBaseline: Bool
        public let dwellUnits: String
        public let rows: [PerVideoScoreRow]
    }

    enum CodingKeys: String, CodingKey {
        case clipId = "clip_id"
        case order
        case dwellSocial = "dwell_social"
        case dwellGeometric = "dwell_geometric"
        case dwellEye = "dwell_eye"
        case dwellMouth = "dwell_mouth"
        case orientingLatencyMs = "orienting_latency_ms"
        case fixationCount = "fixation_count"
        case meanFixationMs = "mean_fixation_ms"
        case status, score, weight, details
    }

    init(report: PerVideoScoreReport, order: Int) {
        func measured(_ key: PerVideoMetricKey) -> Double? {
            report.rows.first { $0.metricKey == key.rawValue && !$0.imputed }?.observed
        }
        clipId = report.stimulusId
        self.order = order
        dwellSocial = measured(.socialDwellRatio)
        dwellGeometric = measured(.geometricDwellRatio)
        dwellEye = measured(.eyeDwellRatio)
        dwellMouth = measured(.mouthDwellRatio)
        orientingLatencyMs = report.rows.first {
            $0.biomarker == PerVideoMetricKey.firstEntryLatencyMs.rawValue && !$0.imputed
        }?.observed
        fixationCount = measured(.fixationCount).map { Int($0.rounded()) }
        meanFixationMs = measured(.meanFixationDurationMs)
        status = report.rows.map(\.status).max { Self.severity($0) < Self.severity($1) }?.rawValue
        score = report.subscore
        weight = report.weight
        details = Details(
            title: report.title,
            category: report.category,
            subscore: report.subscore,
            validSampleRatio: report.validSampleRatio,
            lowConfidence: report.lowConfidence,
            hasBaseline: report.hasBaseline,
            dwellUnits: "ratio",
            rows: report.rows
        )
    }

    private static func severity(_ status: ExpectationStatus) -> Int {
        switch status {
        case .onTarget: return 0
        case .monitor: return 1
        case .atypical: return 2
        }
    }
}

/// The `POST /v1/sessions` body: the backend's `SessionIngestRequest` fields mapped
/// from a `GazeSessionRecord`, plus the full record in `rawPayload` so nothing the
/// app measured is lost (`02 §4`).
public struct SessionIngestPayload: Codable, Equatable, Sendable {
    public let sessionId: String
    /// Pseudonymous token (CNS-7), never a name.
    public let participantToken: String
    public let captureSource: String
    public let durationSec: Double
    public let validRatio: Double
    public let socialPreferenceScore: Double
    public let eyeDwellRatio: Double?
    public let mouthDwellRatio: Double?
    public let firstEyeLatencyMs: Double?
    public let qualityVerdict: String
    public let calibratedDistanceCm: Double?
    /// Sent as explicit `null` when not computed on the device: the backend
    /// would otherwise default it to "green".
    public let attentionFlag: String?
    /// Sent as explicit `null` when unknown: the backend would otherwise
    /// default it to "express_30s".
    public let battery: String?
    /// Wire form ("1.6"), also sent as `X-Schema-Version`.
    public let schemaVersion: String
    public let perVideo: [SessionPerVideoRow]?
    public let rawPayload: GazeSessionRecord

    enum CodingKeys: String, CodingKey {
        case sessionId, participantToken, captureSource, durationSec, validRatio, socialPreferenceScore
        case eyeDwellRatio, mouthDwellRatio, firstEyeLatencyMs, qualityVerdict, calibratedDistanceCm
        case attentionFlag, battery, schemaVersion, perVideo, rawPayload
    }

    public init(record: GazeSessionRecord, attentionFlag: AttentionFlag?, battery: String? = nil) throws {
        guard record.captureSource != .simulated else { throw SessionPayloadError.simulatedCapture }
        guard record.legacyProvenance != true else { throw SessionPayloadError.legacyProvenance }
        sessionId = record.sessionId
        participantToken = record.participantId
        captureSource = record.captureSource.rawValue
        durationSec = record.durationSec
        validRatio = record.validFrameRatio
        socialPreferenceScore = record.socialPreferenceScore
        eyeDwellRatio = record.eyeDwellRatio
        mouthDwellRatio = record.mouthDwellRatio
        firstEyeLatencyMs = record.firstEyeLatencyMs
        qualityVerdict = record.qualityVerdict
        calibratedDistanceCm = record.calibratedDistanceCm
        self.attentionFlag = attentionFlag?.rawValue
        self.battery = battery
        schemaVersion = Self.wireSchemaVersion(record.schemaVersion)
        perVideo = record.perVideo.map { reports in
            reports.enumerated().map { SessionPerVideoRow(report: $1, order: $0 + 1) }
        }
        rawPayload = record
    }

    /// `"v1.6"` (record) -> `"1.6"` (wire, `02 ADR-B5`).
    public static func wireSchemaVersion(_ recordVersion: String) -> String {
        recordVersion.hasPrefix("v") || recordVersion.hasPrefix("V")
            ? String(recordVersion.dropFirst())
            : recordVersion
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(sessionId, forKey: .sessionId)
        try c.encode(participantToken, forKey: .participantToken)
        try c.encode(captureSource, forKey: .captureSource)
        try c.encode(durationSec, forKey: .durationSec)
        try c.encode(validRatio, forKey: .validRatio)
        try c.encode(socialPreferenceScore, forKey: .socialPreferenceScore)
        // Unmeasured metrics are absent keys, never a default (PLC-1/2).
        try c.encodeIfPresent(eyeDwellRatio, forKey: .eyeDwellRatio)
        try c.encodeIfPresent(mouthDwellRatio, forKey: .mouthDwellRatio)
        try c.encodeIfPresent(firstEyeLatencyMs, forKey: .firstEyeLatencyMs)
        try c.encode(qualityVerdict, forKey: .qualityVerdict)
        try c.encodeIfPresent(calibratedDistanceCm, forKey: .calibratedDistanceCm)
        // Explicit null overrides a non-null server default.
        if let attentionFlag { try c.encode(attentionFlag, forKey: .attentionFlag) } else { try c.encodeNil(forKey: .attentionFlag) }
        if let battery { try c.encode(battery, forKey: .battery) } else { try c.encodeNil(forKey: .battery) }
        try c.encode(schemaVersion, forKey: .schemaVersion)
        try c.encodeIfPresent(perVideo, forKey: .perVideo)
        try c.encode(rawPayload, forKey: .rawPayload)
    }
}

/// JSON coding for the session payload on the wire and in the offline queue:
/// sorted keys, ISO-8601 UTC dates with milliseconds.
public enum SessionPayloadCoding {
    public static func encoder() -> JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        encoder.dateEncodingStrategy = .custom { date, encoder in
            var container = encoder.singleValueContainer()
            try container.encode(formatter(fractional: true).string(from: date))
        }
        return encoder
    }

    public static func decoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let raw = try container.decode(String.self)
            if let date = formatter(fractional: true).date(from: raw) ?? formatter(fractional: false).date(from: raw) {
                return date
            }
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "not an ISO-8601 date: \(raw)")
        }
        return decoder
    }

    private static func formatter(fractional: Bool) -> ISO8601DateFormatter {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = fractional ? [.withInternetDateTime, .withFractionalSeconds] : [.withInternetDateTime]
        formatter.timeZone = TimeZone(identifier: "UTC")
        return formatter
    }
}

/// `SessionIngestResponse` (`02 §4`). Every field is optional so a sparse or
/// problem+json body never fails an accepted upload.
public struct SessionIngestResponse: Decodable, Equatable, Sendable {
    public let sessionId: String?
    public let status: String?
    public let duplicate: Bool?
    public let framesUploadURL: String?
    public let reportUploadURL: String?

    public init(sessionId: String?, status: String?, duplicate: Bool?, framesUploadURL: String?, reportUploadURL: String?) {
        self.sessionId = sessionId
        self.status = status
        self.duplicate = duplicate
        self.framesUploadURL = framesUploadURL
        self.reportUploadURL = reportUploadURL
    }

    enum CodingKeys: String, CodingKey {
        case sessionId, status, duplicate
        case framesUploadURL = "frames_upload_url"
        case reportUploadURL = "report_upload_url"
    }
}

// MARK: - Client

public enum SessionSyncError: Error, Equatable, Sendable {
    /// No device token for the configured host: the device needs enrolment.
    case notRegistered
    /// 401/403 even after one re-registration.
    case unauthorized
    case http(status: Int, retryAfter: TimeInterval?)
    /// An upload URL that is not https with a DNS host.
    case insecureURL
    /// The payload can never be sent (asd-sync refused it: unsupported schema version,
    /// simulated or legacy provenance, or a body that breaks the session contract).
    case invalidPayload
}

/// Device-plane HTTP client for sessions (`02 §4`, `ADR-B4`, `ADR-B5`).
///
/// Every session request carries `Authorization: Bearer <device token>`,
/// `X-Idempotency-Key: <sessionId>`, `X-Schema-Version` and `User-Agent`. The
/// bearer goes only to the API host; pre-signed blob URLs on other hosts get no
/// credentials.
///
/// The protocol decisions (headers, canonical body, URL rules, what a status means,
/// `Retry-After`) come from the Rust `asd-sync` crate (CON-7); this type owns only the
/// transport, the credential and the one re-registration.
public struct SessionSyncClient: Sendable {
    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "SessionSyncClient")

    public let apiBaseURL: URL
    public let registration: DeviceRegistrationService
    private let transport: SyncTransport
    private let clientInfo: SyncClientInfo

    public init(apiBaseURL: URL, transport: SyncTransport, registration: DeviceRegistrationService, clientInfo: SyncClientInfo) {
        self.apiBaseURL = apiBaseURL
        self.transport = transport
        self.registration = registration
        self.clientInfo = clientInfo
    }

    public func makeSessionRequest(_ payload: SessionIngestPayload, token: String) throws -> URLRequest {
        let body = try SessionPayloadCoding.encoder().encode(payload)
        let built: AsdSyncRequest
        do {
            built = try syncBuildSessionRequestJson(
                payloadJson: String(decoding: body, as: UTF8.self),
                deviceToken: token,
                client: clientInfo.asdClientInfo
            )
        } catch AsdError.NotRegistered {
            throw SessionSyncError.notRegistered
        } catch let error as AsdError {
            Self.logger.error("Session \(payload.sessionId, privacy: .public) refused by asd-sync: \(String(describing: error), privacy: .public)")
            throw SessionSyncError.invalidPayload
        }
        return try URLRequest(built, apiBaseURL: apiBaseURL)
    }

    /// Posts one session. A 401/403 re-registers once while `mayReRegister` is
    /// true (the caller owns that budget per flush) and retries; a 409 is an
    /// idempotent replay and counts as accepted.
    public func postSession(_ payload: SessionIngestPayload, mayReRegister: inout Bool) async throws -> SessionIngestResponse {
        guard var credential = registration.currentCredential() else { throw SessionSyncError.notRegistered }
        var (status, outcome) = try await send(makeSessionRequest(payload, token: credential.deviceToken))

        if case .reauthenticate = outcome {
            guard mayReRegister else {
                registration.clear()
                throw SessionSyncError.unauthorized
            }
            mayReRegister = false
            Self.logger.info("Session upload got HTTP \(status, privacy: .public); re-registering once")
            do {
                credential = try await registration.reRegister()
            } catch is DeviceRegistrationError {
                throw SessionSyncError.unauthorized
            }
            (status, outcome) = try await send(makeSessionRequest(payload, token: credential.deviceToken))
            if case .reauthenticate = outcome {
                registration.clear()
                throw SessionSyncError.unauthorized
            }
        }

        switch outcome {
        case .accepted(let ack):
            return SessionIngestResponse(
                // A body that is not a session response still means "accepted".
                sessionId: ack.sessionId ?? payload.sessionId,
                status: ack.status,
                duplicate: ack.duplicate,
                framesUploadURL: ack.framesUploadUrl,
                reportUploadURL: ack.reportUploadUrl
            )
        case .duplicate:
            return SessionIngestResponse(sessionId: payload.sessionId, status: "duplicate", duplicate: true, framesUploadURL: nil, reportUploadURL: nil)
        case .retryAfter(let seconds):
            throw SessionSyncError.http(status: status, retryAfter: seconds)
        case .reauthenticate, .fatal:
            throw SessionSyncError.http(status: status, retryAfter: nil)
        }
    }

    /// Absolute https URLs are used as given; relative ones resolve against the
    /// API base. Either way the https/DNS rule applies (asd-sync, F7).
    public func resolveUploadURL(_ raw: String) throws -> URL {
        guard let target = try? syncValidateUploadUrl(url: raw, apiBase: apiBaseURL.absoluteString),
              let url = URL(string: target.url) else {
            throw SessionSyncError.insecureURL
        }
        return url
    }

    /// True only for the API host: the one place the device bearer may go (asd-sync).
    public func isAPIHost(_ url: URL) -> Bool {
        (try? syncValidateUploadUrl(url: url.absoluteString, apiBase: apiBaseURL.absoluteString))?.attachBearer ?? false
    }

    /// PUTs one blob. The bearer is attached only for the API host.
    public func upload(_ data: Data, to url: URL, contentType: String) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        request.timeoutInterval = 60
        request.setValue(contentType, forHTTPHeaderField: SyncHeader.contentType)
        request.setValue(clientInfo.userAgent, forHTTPHeaderField: SyncHeader.userAgent)
        if isAPIHost(url), let credential = registration.currentCredential() {
            request.setValue("Bearer \(credential.deviceToken)", forHTTPHeaderField: SyncHeader.authorization)
        }
        request.httpBody = data
        let (status, outcome) = try await send(request)
        switch outcome {
        case .accepted:
            return
        case .retryAfter(let seconds):
            throw SessionSyncError.http(status: status, retryAfter: seconds)
        case .duplicate, .reauthenticate, .fatal:
            throw SessionSyncError.http(status: status, retryAfter: nil)
        }
    }

    /// Sends a request and lets asd-sync classify the answer.
    private func send(_ request: URLRequest) async throws -> (status: Int, outcome: AsdSyncOutcome) {
        let (data, response) = try await transport.send(request)
        let headers = response.allHeaderFields.compactMap { name, value -> AsdHttpHeader? in
            guard let name = name as? String else { return nil }
            return AsdHttpHeader(name: name, value: "\(value)")
        }
        let status = response.statusCode
        // HTTP statuses are three digits; anything else is not an answer asd-sync can accept.
        let wire = UInt16(exactly: status) ?? 0
        return (status, syncClassifyResponse(status: wire, headers: headers, body: data))
    }
}

extension SyncClientInfo {
    /// The identity asd-sync puts in `User-Agent` (`EyeGaze-iOS/<version> (<build>; iOS <os>)`).
    var asdClientInfo: AsdClientInfo {
        AsdClientInfo(product: "EyeGaze-iOS", appVersion: appVersion, build: build, osName: "iOS", osVersion: osVersion)
    }
}

extension URLRequest {
    /// The request asd-sync built, addressed under the API base (whose URL rules it applies).
    init(_ built: AsdSyncRequest, apiBaseURL: URL) throws {
        guard let raw = try? syncEndpointUrl(apiBase: apiBaseURL.absoluteString, path: built.path),
              let url = URL(string: raw) else {
            throw SessionSyncError.insecureURL
        }
        self.init(url: url)
        httpMethod = built.method
        timeoutInterval = TimeInterval(built.timeoutSecs)
        for header in built.headers {
            setValue(header.value, forHTTPHeaderField: header.name)
        }
        httpBody = built.body
    }
}
