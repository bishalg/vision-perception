import AsdCore
import CryptoKit
import CurvAsdGazeKit
import Foundation
import os

// MARK: - Payload (`POST /v1/sessions`, schema `docs/spec/schemas/asd/session-v1.8.json`)

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
        /// QS-3 per-clip tracking values (1.7); absent keys when not measured (CON-14).
        public let meanSampleConfidence: Double?
        public let meanFixationConfidence: Double?
        public let nearestCheckOffsetDeg: Double?
        public let aoiMinSizeDeg: Double?
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
            rows: report.rows,
            meanSampleConfidence: report.meanSampleConfidence,
            meanFixationConfidence: report.meanFixationConfidence,
            nearestCheckOffsetDeg: report.nearestCheckOffsetDeg,
            aoiMinSizeDeg: report.aoiMinSizeDeg
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
///
/// Schema 1.8 (SCH-5): `scoringStatus` (and `unscorableReason`) at the top level equal the
/// record's; an `unscorable` session has no `socialPreferenceScore`, no affect values in
/// `rawPayload` and no device `attentionFlag` (explicit `null`). A record of an earlier
/// version (or a payload queued by an earlier build) keeps its own schema version and shape.
public struct SessionIngestPayload: Codable, Equatable, Sendable {
    /// Wire schema version of the records this build writes (`X-Schema-Version`).
    public static let currentSchemaVersion = "1.8"

    public let sessionId: String
    /// Pseudonymous token (CNS-7), never a name.
    public let participantToken: String
    public let captureSource: String
    public let durationSec: Double
    public let validRatio: Double
    /// `scored` or `unscorable` (1.8); `nil` only for a payload queued as 1.6/1.7.
    public let scoringStatus: String?
    public let unscorableReason: String?
    /// `nil` when unscorable.
    public let socialPreferenceScore: Double?
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
    /// Wire form ("1.8"), also sent as `X-Schema-Version`.
    public let schemaVersion: String
    public let perVideo: [SessionPerVideoRow]?
    public let rawPayload: GazeSessionRecord

    enum CodingKeys: String, CodingKey {
        case sessionId, participantToken, captureSource, durationSec, validRatio, socialPreferenceScore
        case scoringStatus, unscorableReason
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
        // The record's own version decides the wire version (asd-sync refuses unsupported
        // ones); the 1.8 scoring keys exist only on a 1.8 body.
        let wire = Self.wireSchemaVersion(record.schemaVersion)
        scoringStatus = wire == Self.currentSchemaVersion ? record.scoringStatus.rawValue : nil
        unscorableReason = wire == Self.currentSchemaVersion ? record.unscorableReason?.rawValue : nil
        socialPreferenceScore = record.socialPreferenceScore
        eyeDwellRatio = record.eyeDwellRatio
        mouthDwellRatio = record.mouthDwellRatio
        firstEyeLatencyMs = record.firstEyeLatencyMs
        qualityVerdict = record.qualityVerdict
        calibratedDistanceCm = record.calibratedDistanceCm
        // SCH-5: an unscorable session carries no device flag.
        self.attentionFlag = record.scoringStatus == .scored ? attentionFlag?.rawValue : nil
        self.battery = battery
        schemaVersion = wire
        perVideo = record.perVideo.map { reports in
            reports.enumerated().map { SessionPerVideoRow(report: $1, order: $0 + 1) }
        }
        rawPayload = record
    }

    /// `"v1.7"` (record) -> `"1.7"` (wire, `02 ADR-B5`).
    public static func wireSchemaVersion(_ recordVersion: String) -> String {
        recordVersion.hasPrefix("v") || recordVersion.hasPrefix("V")
            ? String(recordVersion.dropFirst())
            : recordVersion
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        sessionId = try c.decode(String.self, forKey: .sessionId)
        participantToken = try c.decode(String.self, forKey: .participantToken)
        captureSource = try c.decode(String.self, forKey: .captureSource)
        durationSec = try c.decode(Double.self, forKey: .durationSec)
        validRatio = try c.decode(Double.self, forKey: .validRatio)
        scoringStatus = try c.decodeIfPresent(String.self, forKey: .scoringStatus)
        unscorableReason = try c.decodeIfPresent(String.self, forKey: .unscorableReason)
        socialPreferenceScore = try c.decodeIfPresent(Double.self, forKey: .socialPreferenceScore)
        eyeDwellRatio = try c.decodeIfPresent(Double.self, forKey: .eyeDwellRatio)
        mouthDwellRatio = try c.decodeIfPresent(Double.self, forKey: .mouthDwellRatio)
        firstEyeLatencyMs = try c.decodeIfPresent(Double.self, forKey: .firstEyeLatencyMs)
        qualityVerdict = try c.decode(String.self, forKey: .qualityVerdict)
        calibratedDistanceCm = try c.decodeIfPresent(Double.self, forKey: .calibratedDistanceCm)
        attentionFlag = try c.decodeIfPresent(String.self, forKey: .attentionFlag)
        battery = try c.decodeIfPresent(String.self, forKey: .battery)
        schemaVersion = try c.decode(String.self, forKey: .schemaVersion)
        perVideo = try c.decodeIfPresent([SessionPerVideoRow].self, forKey: .perVideo)
        rawPayload = try c.decode(GazeSessionRecord.self, forKey: .rawPayload)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(sessionId, forKey: .sessionId)
        try c.encode(participantToken, forKey: .participantToken)
        try c.encode(captureSource, forKey: .captureSource)
        try c.encode(durationSec, forKey: .durationSec)
        try c.encode(validRatio, forKey: .validRatio)
        try c.encodeIfPresent(scoringStatus, forKey: .scoringStatus)
        try c.encodeIfPresent(unscorableReason, forKey: .unscorableReason)
        // Absent when unscorable (SCH-5, CON-14).
        try c.encodeIfPresent(socialPreferenceScore, forKey: .socialPreferenceScore)
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
    /// RFL-3: offered only in proxy blob mode and for a research-consented session. Read from the
    /// typed asd-sync `AsdIngestAck`.
    public let researchFramesUploadURL: String?

    public init(
        sessionId: String?,
        status: String?,
        duplicate: Bool?,
        framesUploadURL: String?,
        reportUploadURL: String?,
        researchFramesUploadURL: String? = nil
    ) {
        self.sessionId = sessionId
        self.status = status
        self.duplicate = duplicate
        self.framesUploadURL = framesUploadURL
        self.reportUploadURL = reportUploadURL
        self.researchFramesUploadURL = researchFramesUploadURL
    }

    enum CodingKeys: String, CodingKey {
        case sessionId, status, duplicate
        case framesUploadURL = "frames_upload_url"
        case reportUploadURL = "report_upload_url"
        case researchFramesUploadURL = "research_frames_upload_url"
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
    /// true (the caller owns that budget per flush) and retries. A 409 is an idempotent
    /// replay and counts as accepted, unless its problem slug is `participant-withdrawn`
    /// or `idempotency-key-reused` (SYNC-7): then it throws `.http(409)` and the queue
    /// quarantines the item.
    public func postSession(_ payload: SessionIngestPayload, mayReRegister: inout Bool) async throws -> SessionIngestResponse {
        guard var credential = registration.currentCredential() else { throw SessionSyncError.notRegistered }
        var (status, outcome, _) = try await send(makeSessionRequest(payload, token: credential.deviceToken))

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
            (status, outcome, _) = try await send(makeSessionRequest(payload, token: credential.deviceToken))
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
                reportUploadURL: ack.reportUploadUrl,
                researchFramesUploadURL: ack.researchFramesUploadUrl
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

    /// PUTs one blob from its (encrypted) queue file, streamed by the transport (a
    /// background session in the app, SYNC-5), with `Content-Digest` over exactly the bytes
    /// sent (SYNC-6, trap 5). The bearer is attached only for the API host.
    public func uploadFile(_ file: URL, to url: URL, contentType: String) async throws {
        let digest = try Self.contentDigest(of: file)
        var headers = [
            SyncHeader.contentType: contentType,
            SyncHeader.userAgent: clientInfo.userAgent,
            SyncHeader.contentDigest: digest
        ]
        if isAPIHost(url), let credential = registration.currentCredential() {
            headers[SyncHeader.authorization] = "Bearer \(credential.deviceToken)"
        }
        let (data, response) = try await transport.uploadFile(file, to: url, headers: headers)
        let (status, outcome) = classify(data, response)
        switch outcome {
        case .accepted:
            return
        case .retryAfter(let seconds):
            throw SessionSyncError.http(status: status, retryAfter: seconds)
        case .duplicate, .reauthenticate, .fatal:
            throw SessionSyncError.http(status: status, retryAfter: nil)
        }
    }

    /// `Content-Digest: sha-256=:<base64>:` (RFC 9530) of a file, hashed in 64 KiB chunks
    /// so a large frames file is never read into memory at once.
    public static func contentDigest(of file: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: file)
        defer { try? handle.close() }
        var hasher = SHA256()
        while let chunk = try handle.read(upToCount: 64 * 1024), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        return "sha-256=:\(Data(hasher.finalize()).base64EncodedString()):"
    }

    /// Sends a request and lets asd-sync classify the answer.
    private func send(_ request: URLRequest) async throws -> (status: Int, outcome: AsdSyncOutcome, body: Data) {
        let (data, response) = try await transport.send(request)
        let (status, outcome) = classify(data, response)
        return (status, outcome, data)
    }

    /// asd-sync's classification, reading the 409 problem slug (SYNC-7).
    private func classify(_ data: Data, _ response: HTTPURLResponse) -> (status: Int, outcome: AsdSyncOutcome) {
        let headers = response.allHeaderFields.compactMap { name, value -> AsdHttpHeader? in
            guard let name = name as? String else { return nil }
            return AsdHttpHeader(name: name, value: "\(value)")
        }
        let status = response.statusCode
        // HTTP statuses are three digits; anything else is not an answer asd-sync can accept.
        let wire = UInt16(exactly: status) ?? 0
        return (status, syncClassifyResponseWithBody(status: wire, headers: headers, body: data))
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
