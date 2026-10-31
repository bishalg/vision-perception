@testable import AsdGazeCollector
import CurvAsdGazeKit
import Foundation
import XCTest

/// WP-A.4 sync client v2: `/v1/sessions` uploads through the existing offline queue
/// (backoff, dead letter, quarantine, purge), answered by `MockURLProtocol` so the real
/// `URLSessionSyncTransport` path is exercised and nothing touches the network.
final class SessionSyncServiceTests: XCTestCase {
    static let apiBase = URL(string: "https://api.example.org")!
    static let sessionsPath = "/v1/sessions"
    static let registerPath = "/v1/devices/register"
    static let client = SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")

    static let studyConfig = try! ResearchStudyConfiguration(
        ingestURL: URL(string: "https://ingest.example.org/api/v1/research/ingest")!,
        studyIdentifier: "EXAMPLE-STUDY-1",
        irbProtocolId: "ETHICS-REF-42",
        institution: "Example University",
        principalInvestigator: "Example Investigator",
        consentVersion: "2027.1",
        apiBaseURL: apiBase
    )

    private var suiteName: String!
    private var defaults: UserDefaults!
    private var researchStore: ResearchKitStudyStore!
    private var queueDir: URL!
    private var blobRoot: URL!
    private var mockSession: URLSession!
    private var credentials: InMemoryDeviceCredentialStore!
    private var log: RequestLog!
    private var clock: SessionTestClock!

    override func setUp() {
        super.setUp()
        suiteName = "SessionSyncServiceTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
        researchStore = ResearchKitStudyStore(userDefaults: defaults)
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory())
        queueDir = tmp.appendingPathComponent("SessionSyncQueue_\(UUID().uuidString)", isDirectory: true)
        blobRoot = tmp.appendingPathComponent("SessionSyncBlobs_\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: queueDir, withIntermediateDirectories: true)
        try? FileManager.default.createDirectory(at: blobRoot, withIntermediateDirectories: true)
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        mockSession = URLSession(configuration: config)
        credentials = InMemoryDeviceCredentialStore()
        log = RequestLog()
        clock = SessionTestClock()
    }

    override func tearDown() {
        MockURLProtocol.requestHandler = nil
        try? FileManager.default.removeItem(at: queueDir)
        try? FileManager.default.removeItem(at: blobRoot)
        defaults.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    // MARK: - Helpers

    @MainActor
    private func makeService(configuration: ResearchStudyConfiguration = SessionSyncServiceTests.studyConfig) -> ResearchKitSyncService {
        let clock = self.clock!
        return ResearchKitSyncService(
            configuration: configuration,
            store: researchStore,
            queueDirectory: queueDir,
            session: mockSession,
            startMonitoring: false,
            now: { clock.instant },
            credentialStore: credentials,
            clientInfo: Self.client,
            blobRoot: blobRoot
        )
    }

    private func consent() {
        researchStore.saveConsent(
            signature: ORKConsentSignature(guardianFullName: "Sync Parent", signatureToken: "SIG-SYNC-V2"),
            participantId: "STUDY-SYNC-V2"
        )
    }

    private func registerToken(_ token: String = "tok-1", enrolCode: String? = "CODE-1") {
        try? credentials.save(DeviceCredential(
            deviceToken: token,
            orgId: "org-home",
            deviceId: "dev-1",
            configBundleVersion: nil,
            apiHost: "api.example.org",
            enrolCode: enrolCode,
            registeredAt: Date()
        ))
    }

    private func record(_ sessionId: String = "SESS-0001") -> GazeSessionRecord {
        GazeSessionRecord.fixture(sessionId: sessionId, participantId: "child-\(sessionId.prefix(8))")
    }

    /// Routes by path; anything else is a 404 so a stray request is visible.
    private func route(_ routes: [String: @Sendable (URLRequest) throws -> (HTTPURLResponse, Data)]) {
        let log = self.log!
        MockURLProtocol.requestHandler = { request in
            _ = log.record(request)
            if let handler = routes[request.url!.path] { return try handler(request) }
            return (httpResponse(request, 404), Data())
        }
    }

    private static func ingested(
        frames: String? = nil,
        report: String? = nil,
        duplicate: Bool = false
    ) -> @Sendable (URLRequest) throws -> (HTTPURLResponse, Data) {
        { request in
            var body: [String: Any] = ["sessionId": "SESS-0001", "status": "ingested", "duplicate": duplicate]
            body["frames_upload_url"] = frames ?? NSNull()
            body["report_upload_url"] = report ?? NSNull()
            return (httpResponse(request, 200, headers: ["Content-Type": "application/json"]), jsonData(body))
        }
    }

    private static let ok: @Sendable (URLRequest) throws -> (HTTPURLResponse, Data) = { request in
        (httpResponse(request, 200), Data())
    }

    @MainActor
    private func sync(_ service: ResearchKitSyncService) -> Bool {
        let exp = expectation(description: "sync")
        let box = SessionResultBox()
        service.syncNow(forceBundleCompile: false) { success in
            box.value = success
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
        return box.value ?? false
    }

    private func writeBlob(_ name: String, bytes: String = "ENCRYPTED-FRAMES") -> URL {
        let url = blobRoot.appendingPathComponent(name)
        try? Data(bytes.utf8).write(to: url)
        return url
    }

    private func queuedFiles(_ folder: String? = nil) -> [URL] {
        let dir = folder.map { queueDir.appendingPathComponent($0, isDirectory: true) } ?? queueDir!
        return ((try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? [])
            .filter { $0.pathExtension == "json" }
    }

    // MARK: - Configuration gate

    @MainActor
    func testSessionSyncIsOffWithoutAPIBaseURL() throws {
        consent()
        let legacyOnly = try ResearchStudyConfiguration(
            ingestURL: URL(string: "https://ingest.example.org/api/v1/research/ingest")!,
            studyIdentifier: "EXAMPLE-STUDY-1",
            irbProtocolId: "ETHICS-REF-42",
            institution: "Example University",
            principalInvestigator: "Example Investigator",
            consentVersion: "2027.1"
        )
        let service = makeService(configuration: legacyOnly)
        XCTAssertFalse(service.isSessionSyncEnabled)
        XCTAssertEqual(service.deviceRegistration, .unavailable)
        XCTAssertFalse(service.enqueueSession(record()))
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    // MARK: - Enqueue

    @MainActor
    func testEnqueueSessionRequiresConsent() {
        let service = makeService()
        XCTAssertFalse(service.enqueueSession(record()), "no research consent, nothing queued")
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testEnqueueSessionRefusesSimulatedRecords() {
        consent()
        let service = makeService()
        XCTAssertFalse(service.enqueueSession(.fixture(sessionId: "SIM-1", captureSource: .simulated)))
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testEnqueueSessionDedupesBySessionId() {
        consent()
        let service = makeService()
        XCTAssertTrue(service.enqueueSession(record("SESS-DUP")))
        XCTAssertTrue(service.enqueueSession(record("SESS-DUP")))
        XCTAssertTrue(service.enqueueSession(record("SESS-OTHER")))
        XCTAssertEqual(service.pendingQueueCount, 2)
    }

    @MainActor
    func testQueuedSessionIsWrittenWithFileProtection() throws {
        consent()
        let service = makeService()
        XCTAssertTrue(service.enqueueSession(record()))
        let file = try XCTUnwrap(queuedFiles().first)
        XCTAssertTrue(file.lastPathComponent.hasPrefix("session_"))
        XCTAssertTrue(ResearchKitSyncService.writeOptions.contains(.completeFileProtectionUntilFirstUserAuthentication))
        #if !targetEnvironment(simulator)
        let attrs = try FileManager.default.attributesOfItem(atPath: file.path)
        XCTAssertEqual(attrs[.protectionKey] as? FileProtectionType, .completeUntilFirstUserAuthentication)
        #endif
    }

    // MARK: - Request contract

    @MainActor
    func testSessionUploadSendsContractHeadersAndPayload() throws {
        consent()
        registerToken("tok-headers")
        let service = makeService()
        service.enqueueSession(record("SESS-HDR-1"), attentionFlag: .green)
        route([Self.sessionsPath: Self.ingested()])

        XCTAssertTrue(sync(service))

        let post = try XCTUnwrap(log.entries(path: Self.sessionsPath).first)
        XCTAssertEqual(post.method, "POST")
        XCTAssertEqual(post.url.absoluteString, "https://api.example.org/v1/sessions")
        XCTAssertEqual(post.headers["Authorization"], "Bearer tok-headers")
        XCTAssertEqual(post.headers["X-Idempotency-Key"], "SESS-HDR-1")
        XCTAssertEqual(post.headers["X-Schema-Version"], "1.6")
        XCTAssertEqual(post.headers["User-Agent"], "EyeGaze-iOS/1.0.0 (3; iOS 18.0)")
        XCTAssertEqual(post.headers["Content-Type"], "application/json")

        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: post.body) as? [String: Any])
        XCTAssertEqual(body["sessionId"] as? String, "SESS-HDR-1")
        XCTAssertEqual(body["schemaVersion"] as? String, "1.6")
        XCTAssertEqual(body["attentionFlag"] as? String, "green")
        XCTAssertNotNil(body["rawPayload"])

        XCTAssertEqual(service.pendingQueueCount, 0)
        if case .synced(_, let receipt) = service.syncState {
            XCTAssertEqual(receipt, "SESS-HDR-1")
        } else {
            XCTFail("expected synced, got \(service.syncState)")
        }
    }

    @MainActor
    func testIdempotencyKeyIsStableAcrossRetries() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record("SESS-RETRY-KEY"))
        route([Self.sessionsPath: { request in (httpResponse(request, 503), Data()) }])
        XCTAssertFalse(sync(service))
        clock.advance(120)
        route([Self.sessionsPath: Self.ingested()])
        XCTAssertTrue(sync(service))

        let keys = log.entries(path: Self.sessionsPath).map { $0.headers["X-Idempotency-Key"] }
        XCTAssertEqual(keys, ["SESS-RETRY-KEY", "SESS-RETRY-KEY"])
        let bodies = log.entries(path: Self.sessionsPath).map(\.body)
        XCTAssertEqual(bodies[0], bodies[1], "asd-sync's canonical body: a retry sends identical bytes")
    }

    /// asd-sync refuses a payload whose schema version it does not support; the session is
    /// quarantined without a request (the server would answer 400 unsupported-schema-version).
    @MainActor
    func testUnsupportedSchemaVersionIsQuarantinedWithoutARequest() {
        consent()
        registerToken()
        let service = makeService()
        let old = GazeSessionRecord.fixture(sessionId: "SESS-OLD-SCHEMA", participantId: "child-old", schemaVersion: "v1.5")
        XCTAssertTrue(service.enqueueSession(old))
        route([Self.sessionsPath: Self.ingested()])

        XCTAssertFalse(sync(service))
        XCTAssertTrue(log.entries(path: Self.sessionsPath).isEmpty)
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(queuedFiles("Quarantine").count, 1)
    }

    /// `Retry-After` is read by asd-sync: delta-seconds only, capped at an hour.
    @MainActor
    func testRetryAfterIsCappedAtOneHour() throws {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record())
        route([Self.sessionsPath: { request in (httpResponse(request, 503, headers: ["Retry-After": "86400"]), Data()) }])

        XCTAssertFalse(sync(service))
        let meta = try XCTUnwrap(readMeta())
        XCTAssertEqual(meta.nextAttemptAt, clock.instant.timeIntervalSince1970 + 3600, accuracy: 0.001)
    }

    @MainActor
    func testUnregisteredDeviceHoldsTheQueueWithoutCallingSessions() {
        consent()
        let service = makeService()
        service.enqueueSession(record())
        route([Self.sessionsPath: Self.ingested()])

        XCTAssertFalse(sync(service))
        XCTAssertTrue(log.entries(path: Self.sessionsPath).isEmpty)
        XCTAssertEqual(service.pendingQueueCount, 1, "data stays queued until the device is enrolled")
        XCTAssertTrue(queuedFiles("Quarantine").isEmpty)
        XCTAssertEqual(service.deviceRegistration, .unregistered)
        if case .failed(let reason) = service.syncState {
            XCTAssertTrue(reason.contains("enrol"), reason)
        } else {
            XCTFail("expected failed")
        }
    }

    // MARK: - 401 / 409

    @MainActor
    func testUnauthorizedReRegistersOnceAndRetries() throws {
        consent()
        registerToken("tok-stale", enrolCode: "CODE-401")
        let service = makeService()
        service.enqueueSession(record("SESS-401"))
        let attempts = SessionCounter()
        route([
            Self.sessionsPath: { request in
                attempts.increment()
                if request.value(forHTTPHeaderField: "Authorization") == "Bearer tok-stale" {
                    return (httpResponse(request, 401), Data())
                }
                return (httpResponse(request, 200), jsonData(["sessionId": "SESS-401", "status": "ingested"]))
            },
            Self.registerPath: { request in
                (httpResponse(request, 200), jsonData(["status": "rotated", "device_token": "tok-fresh", "org_id": "org-home", "device_id": "dev-1"]))
            }
        ])

        XCTAssertTrue(sync(service))
        XCTAssertEqual(attempts.value, 2)
        XCTAssertEqual(log.entries(path: Self.registerPath).count, 1)
        let registerBody = try XCTUnwrap(JSONSerialization.jsonObject(with: log.entries(path: Self.registerPath)[0].body) as? [String: Any])
        XCTAssertEqual(registerBody["enrol_code"] as? String, "CODE-401")
        XCTAssertEqual(registerBody["device_id"] as? String, "dev-1", "CON-11: re-registration rotates this device's token")
        XCTAssertEqual(log.entries(path: Self.sessionsPath).last?.headers["Authorization"], "Bearer tok-fresh")
        XCTAssertEqual(credentials.load()?.deviceToken, "tok-fresh")
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.deviceRegistration, .registered(orgId: "org-home"))
    }

    @MainActor
    func testUnauthorizedAfterReRegisterHoldsQueueAndReRegistersOnlyOnce() {
        consent()
        registerToken("tok-stale", enrolCode: "CODE-X")
        let service = makeService()
        service.enqueueSession(record("SESS-A"))
        service.enqueueSession(record("SESS-B"))
        route([
            Self.sessionsPath: { request in (httpResponse(request, 401), Data()) },
            Self.registerPath: { request in
                (httpResponse(request, 200), jsonData(["status": "rotated", "device_token": "tok-also-bad", "org_id": "org-home", "device_id": "dev-1"]))
            }
        ])

        XCTAssertFalse(sync(service))
        XCTAssertEqual(log.entries(path: Self.registerPath).count, 1, "re-register once per run, not per item")
        XCTAssertEqual(log.entries(path: Self.sessionsPath).count, 2, "stale token, then fresh token; then stop")
        XCTAssertEqual(service.pendingQueueCount, 2, "auth failure never quarantines data")
        XCTAssertTrue(queuedFiles("Quarantine").isEmpty)
        XCTAssertEqual(service.deadLetterCount, 0)
        XCTAssertEqual(service.deviceRegistration, .needsEnrolment)
        XCTAssertNil(credentials.load())
    }

    @MainActor
    func testRejectedReRegistrationLeavesDeviceNeedingEnrolment() {
        consent()
        registerToken("tok-revoked", enrolCode: "ONE-TIME")
        let service = makeService()
        service.enqueueSession(record())
        route([
            Self.sessionsPath: { request in (httpResponse(request, 401), Data()) },
            Self.registerPath: { request in (httpResponse(request, 410), Data()) }
        ])

        XCTAssertFalse(sync(service))
        XCTAssertEqual(service.pendingQueueCount, 1)
        XCTAssertEqual(service.deviceRegistration, .needsEnrolment)
        XCTAssertNil(credentials.load())
    }

    @MainActor
    func testConflictIsAnIdempotentReplayAndCountsAsSuccess() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record("SESS-409"))
        route([Self.sessionsPath: { request in
            (httpResponse(request, 409, headers: ["Content-Type": "application/problem+json"]),
             jsonData(["type": "about:blank", "title": "duplicate", "status": 409]))
        }])

        XCTAssertTrue(sync(service))
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertTrue(queuedFiles("Quarantine").isEmpty)
    }

    // MARK: - Backoff, quarantine, dead letter

    @MainActor
    func testServerErrorBacksOffAndHonoursRetryAfter() throws {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record())
        route([Self.sessionsPath: { request in (httpResponse(request, 429, headers: ["Retry-After": "120"]), Data()) }])

        XCTAssertFalse(sync(service))
        XCTAssertEqual(service.pendingQueueCount, 1)
        let meta = try XCTUnwrap(readMeta())
        XCTAssertEqual(meta.attemptCount, 1)
        XCTAssertEqual(meta.nextAttemptAt, clock.instant.timeIntervalSince1970 + 120, accuracy: 0.001)

        clock.advance(60)
        XCTAssertFalse(sync(service))
        XCTAssertEqual(log.entries(path: Self.sessionsPath).count, 1, "waits out Retry-After")
    }

    @MainActor
    func testServerErrorUsesTheExistingBackoffSchedule() throws {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record())
        route([Self.sessionsPath: { request in (httpResponse(request, 500), Data()) }])

        XCTAssertFalse(sync(service))
        let meta = try XCTUnwrap(readMeta())
        XCTAssertEqual(meta.nextAttemptAt, clock.instant.timeIntervalSince1970 + 1, accuracy: 0.001)
    }

    @MainActor
    func testSessionDeadLettersAfterMaxRetries() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record())
        route([Self.sessionsPath: { _ in throw URLError(.timedOut) }])

        for _ in 0..<ResearchSyncRetryPolicy.maxAttempts {
            clock.advance(120)
            _ = sync(service)
        }
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.deadLetterCount, 1)
    }

    @MainActor
    func testUnprocessablePayloadIsQuarantined() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record())
        route([Self.sessionsPath: { request in (httpResponse(request, 422), jsonData(["detail": "bad"])) }])

        XCTAssertFalse(sync(service))
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(queuedFiles("Quarantine").count, 1)
    }

    @MainActor
    func testPurgeRemovesQueuedSessions() {
        consent()
        let service = makeService()
        service.enqueueSession(record("SESS-P1"))
        service.enqueueSession(record("SESS-P2"))
        XCTAssertEqual(service.pendingQueueCount, 2)
        service.purgeAllQueued()
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertTrue(((try? FileManager.default.contentsOfDirectory(atPath: queueDir.path)) ?? []).isEmpty)
    }

    @MainActor
    func testLegacyStudyBundleStillGoesToTheIngestURL() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueStudyPayload(bundle: ["legacy": true])
        route(["/api/v1/research/ingest": { request in
            (httpResponse(request, 200), jsonData(["receipt_id": "RCPT-LEGACY"]))
        }])

        XCTAssertTrue(sync(service))
        let entry = log.entries.first
        XCTAssertEqual(entry?.url.absoluteString, "https://ingest.example.org/api/v1/research/ingest")
        XCTAssertNil(entry?.headers["Authorization"], "the legacy ingest path is unchanged")
    }

    // MARK: - Pre-signed uploads

    @MainActor
    func testPreSignedFramesUploadToForeignHostCarriesNoBearer() throws {
        consent()
        registerToken("tok-secret")
        let service = makeService()
        let frames = writeBlob("SESS-0001.jsonl.enc", bytes: "CIPHERTEXT")
        service.enqueueSession(record(), framesFile: frames)
        let presigned = "https://blobs.example.net/sessions/org/child/SESS-0001/frames.jsonl.enc?X-Amz-Signature=abc"
        route([
            Self.sessionsPath: Self.ingested(frames: presigned),
            "/sessions/org/child/SESS-0001/frames.jsonl.enc": Self.ok
        ])

        XCTAssertTrue(sync(service))
        let put = try XCTUnwrap(log.entries.first { $0.method == "PUT" })
        XCTAssertEqual(put.url.absoluteString, presigned)
        XCTAssertEqual(put.body, Data("CIPHERTEXT".utf8))
        XCTAssertEqual(put.headers["Content-Type"], "application/octet-stream")
        XCTAssertNil(put.headers["Authorization"], "the device token never goes to a blob host")
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testRelativeUploadURLResolvesAgainstAPIBaseAndCarriesBearer() throws {
        consent()
        registerToken("tok-same-origin")
        let service = makeService()
        let frames = writeBlob("frames.jsonl.enc")
        let report = writeBlob("report.pdf", bytes: "%PDF-1.7")
        service.enqueueSession(record(), framesFile: frames, reportFile: report)
        route([
            Self.sessionsPath: Self.ingested(
                frames: "/v1/sessions/SESS-0001/upload/frames",
                report: "/v1/sessions/SESS-0001/upload/report"
            ),
            "/v1/sessions/SESS-0001/upload/frames": Self.ok,
            "/v1/sessions/SESS-0001/upload/report": Self.ok
        ])

        XCTAssertTrue(sync(service))
        let puts = log.entries.filter { $0.method == "PUT" }
        XCTAssertEqual(puts.map(\.url.absoluteString).sorted(), [
            "https://api.example.org/v1/sessions/SESS-0001/upload/frames",
            "https://api.example.org/v1/sessions/SESS-0001/upload/report"
        ])
        XCTAssertTrue(puts.allSatisfy { $0.headers["Authorization"] == "Bearer tok-same-origin" })
        XCTAssertEqual(puts.first { $0.url.path.hasSuffix("report") }?.headers["Content-Type"], "application/pdf")
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testNoBlobUploadWhenServerReturnsNoURL() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record(), framesFile: writeBlob("frames.jsonl.enc"))
        route([Self.sessionsPath: Self.ingested(duplicate: true)])

        XCTAssertTrue(sync(service))
        XCTAssertTrue(log.entries.filter { $0.method == "PUT" }.isEmpty)
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testInsecureUploadURLIsNeverUsed() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record(), framesFile: writeBlob("frames.jsonl.enc"))
        route([Self.sessionsPath: Self.ingested(frames: "http://blobs.example.net/frames")])

        _ = sync(service)
        XCTAssertTrue(log.entries.filter { $0.method == "PUT" }.isEmpty, "http upload URLs are refused")
        XCTAssertEqual(queuedFiles("Quarantine").count, 1)
    }

    @MainActor
    func testFailedBlobUploadRetriesOnlyTheUpload() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record(), framesFile: writeBlob("frames.jsonl.enc"))
        let presigned = "https://blobs.example.net/frames?sig=1"
        route([
            Self.sessionsPath: Self.ingested(frames: presigned),
            "/frames": { request in (httpResponse(request, 503), Data()) }
        ])

        XCTAssertFalse(sync(service))
        XCTAssertEqual(service.pendingQueueCount, 1)

        clock.advance(120)
        route([
            Self.sessionsPath: Self.ingested(frames: presigned),
            "/frames": Self.ok
        ])
        XCTAssertTrue(sync(service))
        XCTAssertEqual(log.entries(path: Self.sessionsPath).count, 1, "the session is not posted again")
        XCTAssertEqual(log.entries(path: "/frames").count, 2)
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testExpiredUploadURLRequestsTheSessionAgain() {
        consent()
        registerToken()
        let service = makeService()
        service.enqueueSession(record(), framesFile: writeBlob("frames.jsonl.enc"))
        route([
            Self.sessionsPath: Self.ingested(frames: "https://blobs.example.net/frames?sig=old"),
            "/frames": { request in (httpResponse(request, 403), Data()) }
        ])

        XCTAssertFalse(sync(service))
        XCTAssertEqual(service.pendingQueueCount, 1)
        XCTAssertTrue(queuedFiles("Quarantine").isEmpty, "an expired URL is not a bad payload")

        clock.advance(120)
        route([
            Self.sessionsPath: Self.ingested(frames: "https://blobs.example.net/frames?sig=new"),
            "/frames": Self.ok
        ])
        XCTAssertTrue(sync(service))
        XCTAssertEqual(log.entries(path: Self.sessionsPath).count, 2, "re-posted for a fresh URL")
        XCTAssertEqual(log.entries(path: "/frames").last?.url.query, "sig=new")
    }

    @MainActor
    func testMissingBlobFileIsSkipped() {
        consent()
        registerToken()
        let service = makeService()
        let frames = writeBlob("gone.jsonl.enc")
        service.enqueueSession(record(), framesFile: frames)
        try? FileManager.default.removeItem(at: frames)
        route([Self.sessionsPath: Self.ingested(frames: "https://blobs.example.net/frames")])

        XCTAssertTrue(sync(service))
        XCTAssertTrue(log.entries.filter { $0.method == "PUT" }.isEmpty)
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    // MARK: - Registration through the service

    @MainActor
    func testRegisterDevicePublishesRegisteredState() async throws {
        let service = makeService()
        XCTAssertEqual(service.deviceRegistration, .unregistered)
        route([Self.registerPath: { request in
            (httpResponse(request, 200), jsonData(["status": "registered", "device_token": "tok-new", "org_id": "org-clinic", "device_id": "dev-new"]))
        }])

        try await service.registerDevice(enrolCode: "CLINIC-CODE")
        XCTAssertEqual(service.deviceRegistration, .registered(orgId: "org-clinic"))
        XCTAssertEqual(credentials.load()?.deviceToken, "tok-new")
    }

    @MainActor
    func testRegisteredStateIsRestoredFromTheCredentialStore() {
        registerToken()
        XCTAssertEqual(makeService().deviceRegistration, .registered(orgId: "org-home"))
    }

    private func readMeta() -> (attemptCount: Int, nextAttemptAt: TimeInterval)? {
        let files = (try? FileManager.default.contentsOfDirectory(at: queueDir, includingPropertiesForKeys: nil)) ?? []
        guard let url = files.first(where: { $0.pathExtension == "syncmeta" }),
              let data = try? Data(contentsOf: url),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let count = object["attemptCount"] as? Int,
              let next = object["nextAttemptAt"] as? Double else { return nil }
        return (count, next)
    }
}

private final class SessionTestClock: @unchecked Sendable {
    var instant = Date(timeIntervalSince1970: 1_793_000_000)
    func advance(_ seconds: TimeInterval) { instant = instant.addingTimeInterval(seconds) }
}

private final class SessionResultBox: @unchecked Sendable {
    var value: Bool?
}

private final class SessionCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var _value = 0
    var value: Int { lock.lock(); defer { lock.unlock() }; return _value }
    func increment() { lock.lock(); _value += 1; lock.unlock() }
}
