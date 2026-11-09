@testable import AsdGazeCollector
import AsdCore
import CryptoKit
import Foundation
import Network
import XCTest

/// SYNC-2..5, SYNC-7..9: the offline-first queue against `FaultInjectingTransport` on a
/// virtual clock. Scenario (a) is the 40-session flapping network; (b)-(e) are the cut
/// blob, the relaunch, `409 participant-withdrawn` and `429 Retry-After`.
final class SyncFieldScenarioTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!
    private var researchStore: ResearchKitStudyStore!
    private var queueDir: URL!
    private var blobRoot: URL!
    private var credentials: InMemoryDeviceCredentialStore!
    private var clock: VirtualClock!
    private var server: StubIngestServer!

    override func setUp() {
        super.setUp()
        suiteName = "SyncFieldScenarioTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
        researchStore = ResearchKitStudyStore(userDefaults: defaults)
        researchStore.saveConsent(
            signature: ORKConsentSignature(guardianFullName: "Field Parent", signatureToken: "SIG-FIELD"),
            participantId: "STUDY-FIELD"
        )
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory())
        queueDir = tmp.appendingPathComponent("SyncFieldQueue_\(UUID().uuidString)", isDirectory: true)
        blobRoot = tmp.appendingPathComponent("SyncFieldBlobs_\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: queueDir, withIntermediateDirectories: true)
        try? FileManager.default.createDirectory(at: blobRoot, withIntermediateDirectories: true)
        credentials = InMemoryDeviceCredentialStore()
        try? credentials.save(DeviceCredential(
            deviceToken: "tok-field",
            orgId: "org-clinic",
            deviceId: "dev-field",
            configBundleVersion: nil,
            apiHost: "api.example.org",
            enrolCode: "CODE-FIELD",
            registeredAt: Date()
        ))
        clock = VirtualClock()
        server = StubIngestServer()
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: queueDir)
        try? FileManager.default.removeItem(at: blobRoot)
        defaults.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    // MARK: - Helpers

    private func makeTransport() -> FaultInjectingTransport {
        FaultInjectingTransport(server: server, clock: clock)
    }

    /// A service on the virtual clock whose own retry timer never fires: the scenarios
    /// call `fireRetryTimerIfDue()` themselves as virtual time passes.
    @MainActor
    private func makeService(_ transport: FaultInjectingTransport, realTime: Bool = false) -> ResearchKitSyncService {
        let clock = self.clock!
        let service: ResearchKitSyncService
        if realTime {
            service = ResearchKitSyncService(
                configuration: SessionSyncServiceTests.studyConfig,
                store: researchStore,
                queueDirectory: queueDir,
                startMonitoring: false,
                credentialStore: credentials,
                clientInfo: SessionSyncServiceTests.client,
                blobRoot: blobRoot,
                transport: transport,
                deliveryAuthorization: ClinicDeliveryAuthorization.optedInForTests
            )
        } else {
            service = ResearchKitSyncService(
                configuration: SessionSyncServiceTests.studyConfig,
                store: researchStore,
                queueDirectory: queueDir,
                startMonitoring: false,
                now: { clock.now },
                credentialStore: credentials,
                clientInfo: SessionSyncServiceTests.client,
                blobRoot: blobRoot,
                transport: transport,
                retrySleep: { _ in try await Task.sleep(for: .seconds(86_400)) },
                deliveryAuthorization: ClinicDeliveryAuthorization.optedInForTests
            )
        }
        transport.onPathChange = { [weak service] online in
            service?.handlePathUpdate(status: online ? .satisfied : .unsatisfied)
        }
        return service
    }

    /// Queues session `sessionId` with a frames file of `bytes` bytes.
    @MainActor
    @discardableResult
    private func enqueue(_ service: ResearchKitSyncService, _ sessionId: String, bytes: Int = 4096) -> Bool {
        var generator = SystemRandomNumberGenerator()
        let data = Data((0..<bytes).map { _ in UInt8.random(in: 0...255, using: &generator) })
        let file = blobRoot.appendingPathComponent("\(sessionId).jsonl.enc")
        try? data.write(to: file)
        server.expectBlob(data, for: sessionId)
        return service.enqueueSession(
            GazeSessionRecord.fixture(sessionId: sessionId, participantId: "child-0123456789abcdef0123"),
            framesFile: file
        )
    }

    /// Steps virtual time: reports the network pattern, fires the retry timer when due,
    /// lets every flush finish, until `done()` or `deadline`.
    @MainActor
    private func drive(
        _ service: ResearchKitSyncService,
        _ transport: FaultInjectingTransport,
        until deadline: TimeInterval,
        step: TimeInterval = 1,
        done: () -> Bool
    ) async {
        while clock.seconds < deadline {
            transport.reportPath()
            service.fireRetryTimerIfDue()
            await service.waitUntilIdle()
            if done() { return }
            clock.advance(by: step)
        }
    }

    private func folderJSONCount(_ folder: String) -> Int {
        let dir = queueDir.appendingPathComponent(folder, isDirectory: true)
        return ((try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? [])
            .filter { $0.pathExtension == "json" }.count
    }

    private func queueMeta(in folder: String? = nil) -> [String: Any]? {
        let dir = folder.map { queueDir.appendingPathComponent($0, isDirectory: true) } ?? queueDir!
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
        guard let url = files.first(where: { $0.pathExtension == "syncmeta" }),
              let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    // MARK: - SYNC-9 (a): 40 sessions, flapping network, 30 % drops

    @MainActor
    func testFortySessionsSurviveTwoMinutesOfFlappingNetwork() async {
        let transport = makeTransport()
        let service = makeService(transport)
        let start = clock.seconds
        transport.flapOnline(.init(start: start, period: 2, until: start + 120, startsOnline: false))
        transport.randomFaults(.init(dropRate: 0.3, latencyMs: 1000...3000))
        transport.reportPath()
        XCTAssertEqual(service.isOnline, false)

        let ids = (1...40).map { String(format: "FIELD-%04d", $0) }
        for id in ids { XCTAssertTrue(enqueue(service, id)) }
        XCTAssertEqual(service.pendingQueueCount, 40)
        XCTAssertTrue(transport.requests.isEmpty, "nothing is sent while offline")

        // Two minutes of flapping, then the network stays up until the queue drains.
        await drive(service, transport, until: start + 3600) {
            clock.seconds >= start + 120 && service.pendingQueueCount == 0
        }

        XCTAssertEqual(Set(server.acceptedSessionIds), Set(ids), "exactly 40 sessions accepted")
        XCTAssertEqual(server.acceptedSessionIds.count, 40)
        XCTAssertEqual(server.completeBlobs, 40)
        XCTAssertEqual(server.digestMismatches, 0)
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.deadLetterCount, 0)
        XCTAssertEqual(service.quarantineCount, 0)
        XCTAssertEqual(folderJSONCount("DeadLetter"), 0)
        XCTAssertEqual(folderJSONCount("Quarantine"), 0)
        let posts = transport.requests(method: "POST", pathSuffix: "/v1/sessions").count
        print("SYNC-9(a): \(posts) POSTs (\(server.duplicatePosts) answered duplicate), "
              + "\(transport.requests.count) requests, drained at +\(Int(clock.seconds - start)) s virtual")
    }

    // MARK: - SYNC-9 (b): a blob cut mid-body completes without a second POST

    @MainActor
    func testCutBlobCompletesOnTheNextFlushWithoutASecondPost() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.script("PUT", "/upload/frames", .cutBody(afterBytes: 100))

        enqueue(service, "CUT-0001")
        await service.waitUntilIdle()
        XCTAssertEqual(server.acceptedSessionIds, ["CUT-0001"])
        XCTAssertEqual(server.incompleteBlobs, 1)
        XCTAssertEqual(service.pendingQueueCount, 1)
        XCTAssertEqual(service.statusSummary.uploading, 1, "accepted, blob still uploading")
        XCTAssertEqual(service.statusSummary.waiting, 0)

        clock.advance(by: ResearchSyncRetryPolicy.delay(afterAttempt: 1))
        service.fireRetryTimerIfDue()
        await service.waitUntilIdle()

        XCTAssertEqual(server.completeBlobs, 1)
        XCTAssertEqual(transport.requests(method: "POST", pathSuffix: "/v1/sessions").count, 1)
        XCTAssertEqual(transport.requests(method: "PUT", pathSuffix: "/upload/frames").count, 2)
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    // MARK: - SYNC-9 (c): a relaunch finishes what the old instance started

    @MainActor
    func testNewInstanceOverTheSameQueueFinishesTheUploads() async {
        let first = makeTransport()
        var service: ResearchKitSyncService? = makeService(first)
        first.reportPath()
        first.script("PUT", "/upload/frames", .status(503), .status(503), .status(503))
        for id in ["RELAUNCH-1", "RELAUNCH-2", "RELAUNCH-3"] { enqueue(service!, id) }
        await service!.waitUntilIdle()
        XCTAssertEqual(server.acceptedSessionIds.count, 3)
        XCTAssertEqual(server.completeBlobs, 0)
        XCTAssertEqual(service!.pendingQueueCount, 3)
        service = nil

        // "Relaunch": a new process, a new transport, the same queue directory.
        clock.advance(by: 5)
        let second = makeTransport()
        let relaunched = makeService(second)
        XCTAssertEqual(relaunched.statusSummary.uploading, 3, "stage read back from the queue")
        relaunched.flushAtLaunch()
        XCTAssertTrue(second.requests.isEmpty, "network state unknown: the first report decides")
        second.reportPath()
        await relaunched.waitUntilIdle()

        XCTAssertTrue(second.requests(method: "POST", pathSuffix: "/v1/sessions").isEmpty, "no second POST")
        XCTAssertEqual(second.requests(method: "PUT", pathSuffix: "/upload/frames").count, 3)
        XCTAssertEqual(server.completeBlobs, 3)
        XCTAssertEqual(relaunched.pendingQueueCount, 0)
        XCTAssertNotNil(relaunched.statusSummary.lastSyncedAt)
    }

    // MARK: - SYNC-9 (d) / SYNC-7: 409 slugs

    @MainActor
    func testParticipantWithdrawnIsQuarantinedNotDeleted() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.script("POST", "/v1/sessions", .problem(409, slug: "participant-withdrawn"))

        enqueue(service, "WITHDRAWN-1")
        await service.waitUntilIdle()

        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.quarantineCount, 1)
        XCTAssertEqual(folderJSONCount("Quarantine"), 1, "kept for inspection, never deleted")
        XCTAssertEqual(service.deadLetterCount, 0)
        XCTAssertEqual(service.statusSummary.needsAttention, 1)
        XCTAssertTrue(server.acceptedSessionIds.isEmpty)
    }

    @MainActor
    func testConflictReplayIsDeliveredButKeyReuseIsQuarantined() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.script(
            "POST", "/v1/sessions",
            .status(409, retryAfter: nil, body: jsonData(["duplicate": true, "sessionId": "REPLAY-1"])),
            .status(409),
            .problem(409, slug: "idempotency-key-reused")
        )

        enqueue(service, "REPLAY-1")
        await service.waitUntilIdle()
        enqueue(service, "OLDSERVER-1")
        await service.waitUntilIdle()
        XCTAssertEqual(service.pendingQueueCount, 0, "duplicate:true and an empty 409 count as delivered")
        XCTAssertEqual(service.quarantineCount, 0)

        enqueue(service, "REUSED-1")
        await service.waitUntilIdle()
        XCTAssertEqual(service.quarantineCount, 1)
    }

    func testFFIClassifierReadsThe409Slug() {
        let withdrawn = jsonData(["type": "https://schemas.curv.org/asd/problems/participant-withdrawn"])
        XCTAssertEqual(syncClassifyResponseWithBody(status: 409, headers: [], body: withdrawn), .fatal(status: 409))
        XCTAssertEqual(syncClassifyResponseWithBody(status: 409, headers: [], body: jsonData(["duplicate": true])), .duplicate)
        XCTAssertEqual(syncClassifyResponseWithBody(status: 409, headers: [], body: Data()), .duplicate)
        XCTAssertEqual(syncClassifyResponse(status: 409, headers: [], body: withdrawn), .duplicate, "old function unchanged")
    }

    // MARK: - SYNC-9 (e) / SYNC-3: Retry-After

    @MainActor
    func testRetryAfterThirtySecondsIsHonoured() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.script("POST", "/v1/sessions", .status(429, retryAfter: 30))
        let t0 = clock.seconds

        enqueue(service, "LIMITED-1")
        await service.waitUntilIdle()
        XCTAssertEqual(transport.requests(method: "POST", pathSuffix: "/v1/sessions"), [t0])
        XCTAssertEqual(service.nextRetryAt?.timeIntervalSince1970 ?? 0, t0 + 30, accuracy: 0.001)

        for offset in [1.0, 10, 29, 29.9] {
            clock.advance(by: t0 + offset - clock.seconds)
            service.fireRetryTimerIfDue()
            service.requestFlush(.foreground)
            await service.waitUntilIdle()
            XCTAssertEqual(transport.requests(method: "POST", pathSuffix: "/v1/sessions").count, 1, "no attempt at +\(offset) s")
        }

        clock.advance(by: t0 + 30 - clock.seconds)
        service.fireRetryTimerIfDue()
        await service.waitUntilIdle()
        let attempts = transport.requests(method: "POST", pathSuffix: "/v1/sessions")
        XCTAssertEqual(attempts.count, 2)
        XCTAssertGreaterThanOrEqual((attempts.last ?? 0) - t0, 30)
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    // MARK: - SYNC-3: backoff parity with the Rust schedule

    @MainActor
    func testServiceDelaysEqualTheFFISchedule() async throws {
        let maxAttempts = Int(syncMaxAttempts())
        XCTAssertEqual(ResearchSyncRetryPolicy.maxAttempts, maxAttempts)
        XCTAssertEqual(
            ResearchSyncRetryPolicy.delays,
            (1..<maxAttempts).map { TimeInterval(syncBackoffSchedule(attempt: UInt32($0))) }
        )

        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.randomFaults(.init(dropRate: 0, latencyMs: 0...0))
        for _ in 0..<maxAttempts { transport.script("POST", "/v1/sessions", .status(500)) }

        enqueue(service, "BACKOFF-1")
        await service.waitUntilIdle()
        for attempt in 1..<maxAttempts {
            let meta = try XCTUnwrap(queueMeta(), "attempt \(attempt)")
            XCTAssertEqual(meta["attemptCount"] as? Int, attempt)
            let delay = (meta["nextAttemptAt"] as? Double ?? 0) - clock.seconds
            XCTAssertEqual(delay, syncNextRetryDelay(attempt: UInt32(attempt), retryAfterSecs: nil), accuracy: 0.001)
            XCTAssertEqual(delay, TimeInterval(syncBackoffSchedule(attempt: UInt32(attempt))), accuracy: 0.001)
            clock.advance(by: delay)
            service.fireRetryTimerIfDue()
            await service.waitUntilIdle()
        }
        XCTAssertEqual(transport.requests(method: "POST", pathSuffix: "/v1/sessions").count, maxAttempts)
        XCTAssertEqual(service.deadLetterCount, 1, "dead-lettered after syncMaxAttempts() failures")
    }

    // MARK: - SYNC-4: dead-letter cycles

    @MainActor
    func testDeadLettersAreRecycledOnReconnectAtMostThreeTimes() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.randomFaults(.init(dropRate: 1, latencyMs: 0...0))
        enqueue(service, "STUCK-1")

        func exhaust() async {
            for _ in 0..<(ResearchSyncRetryPolicy.maxAttempts * 2) where service.deadLetterCount == 0 {
                clock.advance(by: 61)
                service.fireRetryTimerIfDue()
                await service.waitUntilIdle()
            }
        }

        await exhaust()
        XCTAssertEqual(service.deadLetterCount, 1)
        for cycle in 1...ResearchSyncRetryPolicy.maxDeadLetterCycles {
            service.handlePathUpdate(status: .unsatisfied)
            service.handlePathUpdate(status: .satisfied)
            await service.waitUntilIdle()
            XCTAssertEqual(queueMeta()?["deadLetterCycles"] as? Int ?? queueMeta(in: "DeadLetter")?["deadLetterCycles"] as? Int, cycle)
            await exhaust()
            XCTAssertEqual(service.deadLetterCount, 1, "cycle \(cycle)")
        }

        // Fourth reconnect (and a launch): it stays dead-lettered and needs attention.
        service.handlePathUpdate(status: .unsatisfied)
        service.handlePathUpdate(status: .satisfied)
        service.flushAtLaunch()
        await service.waitUntilIdle()
        XCTAssertEqual(service.deadLetterCount, 1)
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.statusSummary.needsAttention, 1)

        // The one-tap retry still works.
        transport.randomFaults(nil)
        service.retryDeadLetterItems()
        service.requestFlush(.foreground)
        await service.waitUntilIdle()
        XCTAssertEqual(service.deadLetterCount, 0)
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(server.completeBlobs, 1)
    }

    // MARK: - SYNC-2: triggers

    @MainActor
    func testEnqueueFlushesImmediatelyWhenOnline() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()

        enqueue(service, "ENQ-1")
        await service.waitUntilIdle()
        XCTAssertEqual(server.acceptedSessionIds, ["ENQ-1"])
        XCTAssertEqual(service.pendingQueueCount, 0)

        // Several enqueues during one flush: all of them go in the follow-up flush.
        enqueue(service, "ENQ-2")
        enqueue(service, "ENQ-3")
        enqueue(service, "ENQ-4")
        await service.waitUntilIdle()
        XCTAssertEqual(Set(server.acceptedSessionIds), ["ENQ-1", "ENQ-2", "ENQ-3", "ENQ-4"])
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    @MainActor
    func testUnknownNetworkStateWaitsForTheFirstReport() async {
        let transport = makeTransport()
        let service = makeService(transport)
        XCTAssertNil(service.isOnline, "unknown until the monitor reports")
        enqueue(service, "UNKNOWN-1")
        await service.waitUntilIdle()
        XCTAssertTrue(transport.requests.isEmpty)

        transport.reportPath()
        await service.waitUntilIdle()
        XCTAssertEqual(server.acceptedSessionIds, ["UNKNOWN-1"])
    }

    @MainActor
    func testLaunchWithTheNetworkUpFlushesWithinFiveSeconds() async {
        let transport = makeTransport()
        let earlier = makeService(transport)
        enqueue(earlier, "LAUNCH-1")
        enqueue(earlier, "LAUNCH-2")
        XCTAssertTrue(transport.requests.isEmpty)

        let launchTransport = makeTransport()
        let service = makeService(launchTransport, realTime: true)
        let launchedAt = Date()
        service.flushAtLaunch()
        launchTransport.reportPath()
        while service.pendingQueueCount > 0 && Date().timeIntervalSince(launchedAt) < 5 {
            await service.waitUntilIdle()
            try? await Task.sleep(for: .milliseconds(20))
        }
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertLessThan(Date().timeIntervalSince(launchedAt), 5)
        XCTAssertEqual(Set(server.acceptedSessionIds), ["LAUNCH-1", "LAUNCH-2"])
    }

    @MainActor
    func testRetryTimerFlushesWhenTheBackoffPasses() async {
        let transport = makeTransport()
        let service = makeService(transport, realTime: true)
        transport.reportPath()
        transport.script("POST", "/v1/sessions", .status(503))

        enqueue(service, "TIMER-1")
        await service.waitUntilIdle()
        XCTAssertEqual(service.pendingQueueCount, 1)
        XCTAssertNotNil(service.nextRetryAt)

        let started = Date()
        while service.pendingQueueCount > 0 && Date().timeIntervalSince(started) < 5 {
            try? await Task.sleep(for: .milliseconds(50))
            await service.waitUntilIdle()
        }
        XCTAssertEqual(service.pendingQueueCount, 0, "the timer retried without any other trigger")
        XCTAssertEqual(transport.requests(method: "POST", pathSuffix: "/v1/sessions").count, 2)
    }

    // MARK: - SYNC-5 / SYNC-6

    @MainActor
    func testBlobPutCarriesTheRFC9530DigestOfTheFile() async throws {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        enqueue(service, "DIGEST-1", bytes: 70_000)
        await service.waitUntilIdle()
        XCTAssertEqual(server.completeBlobs, 1)
        XCTAssertEqual(server.digestMismatches, 0)

        let file = blobRoot.appendingPathComponent("DIGEST-1.jsonl.enc")
        let expected = "sha-256=:\(Data(SHA256.hash(data: try Data(contentsOf: file))).base64EncodedString()):"
        XCTAssertEqual(try SessionSyncClient.contentDigest(of: file), expected)
    }

    @MainActor
    func testBackgroundUploadFinishedAfterRelaunchMarksTheBlobDone() async {
        let transport = makeTransport()
        let service = makeService(transport)
        transport.reportPath()
        transport.script("PUT", "/upload/frames", .status(503))
        enqueue(service, "ORPHAN-1")
        await service.waitUntilIdle()
        XCTAssertEqual(service.statusSummary.uploading, 1)

        // The system finished the upload while the app was not running.
        let path = blobRoot.appendingPathComponent("ORPHAN-1.jsonl.enc").resolvingSymlinksInPath().standardizedFileURL.path
        clock.advance(by: 5)
        service.backgroundUploadFinished(filePath: path, status: 200)
        await service.waitUntilIdle()

        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(transport.requests(method: "PUT", pathSuffix: "/upload/frames").count, 1, "not uploaded again")
        XCTAssertEqual(transport.requests(method: "POST", pathSuffix: "/v1/sessions").count, 1, "not posted again")
    }

    func testBackgroundSessionIdentifierIsFixed() {
        XCTAssertEqual(BackgroundBlobUploader.identifier, "org.curv.asd.sync.blobs")
        let first = BackgroundBlobUploader.makeConfiguration()
        let second = BackgroundBlobUploader.makeConfiguration()
        XCTAssertEqual(first.identifier, BackgroundBlobUploader.identifier)
        XCTAssertEqual(first.identifier, second.identifier, "stable across instances and launches")
        XCTAssertFalse(first.isDiscretionary)
        XCTAssertTrue(first.sessionSendsLaunchEvents)
    }

    // MARK: - SYNC-8

    @MainActor
    func testClinicStatusLineShowsCountsOnly() {
        let now = Date()
        let line = ClinicSyncStatusLine.text(
            for: SyncStatusSummary(waiting: 2, uploading: 1, needsAttention: 0, lastSyncedAt: now),
            now: now,
            lang: .english
        )
        XCTAssertTrue(line.hasPrefix("2 waiting · 1 uploading · last synced "), line)
        XCTAssertFalse(line.contains("child-"))

        let attention = ClinicSyncStatusLine.text(
            for: SyncStatusSummary(waiting: 0, uploading: 0, needsAttention: 3, lastSyncedAt: nil),
            lang: .english
        )
        XCTAssertEqual(attention, "0 waiting · 0 uploading · 3 need attention · not synced yet")
        for lang in AppLocale.allCases {
            XCTAssertFalse(ClinicSyncStatusLine.text(
                for: SyncStatusSummary(waiting: 1, uploading: 0, needsAttention: 1, lastSyncedAt: nil), lang: lang
            ).contains("%"), "\(lang) has a format placeholder left")
        }
    }
}
