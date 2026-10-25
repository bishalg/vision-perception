@testable import AsdGazeCollector
import CurvAsdGazeKit
import Foundation
import XCTest

final class MockURLProtocol: URLProtocol, @unchecked Sendable {
    static var requestHandler: (@Sendable (URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = MockURLProtocol.requestHandler else {
            client?.urlProtocol(self, didFailWithError: NSError(domain: "MockURLProtocol", code: -1))
            return
        }

        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

final class ResearchKitSyncServiceTests: XCTestCase {
    /// Injected study configuration (CNS-9). The host is reserved (RFC 2606) and
    /// every request is answered by `MockURLProtocol`, so no test touches the network.
    static let studyConfig = try! ResearchStudyConfiguration(
        ingestURL: URL(string: "https://ingest.example.org/api/v1/research/ingest")!,
        studyIdentifier: "EXAMPLE-STUDY-1",
        irbProtocolId: "ETHICS-REF-42",
        institution: "Example University",
        principalInvestigator: "Example Investigator",
        consentVersion: "2027.1"
    )

    private var suiteName: String!
    private var testUserDefaults: UserDefaults!
    private var researchStore: ResearchKitStudyStore!
    private var tempQueueDir: URL!
    private var mockSession: URLSession!

    override func setUp() {
        super.setUp()
        suiteName = "ResearchKitSyncServiceTests_\(UUID().uuidString)"
        testUserDefaults = UserDefaults(suiteName: suiteName)!
        researchStore = ResearchKitStudyStore(userDefaults: testUserDefaults)

        tempQueueDir = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("TestSyncQueue_\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: tempQueueDir, withIntermediateDirectories: true)

        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        mockSession = URLSession(configuration: config)
    }

    override func tearDown() {
        MockURLProtocol.requestHandler = nil
        try? FileManager.default.removeItem(at: tempQueueDir)
        testUserDefaults.removePersistentDomain(forName: suiteName)
        testUserDefaults = nil
        researchStore = nil
        mockSession = nil
        super.tearDown()
    }

    @MainActor
    func testEnqueuePayloadIncrementsPendingCount() {
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        XCTAssertEqual(service.pendingQueueCount, 0)

        let payload1: [String: Any] = ["testKey": "val1", "version": 1]
        service.enqueueStudyPayload(bundle: payload1)
        XCTAssertEqual(service.pendingQueueCount, 1)

        let payload2: [String: Any] = ["testKey": "val2", "version": 2]
        service.enqueueStudyPayload(bundle: payload2)
        XCTAssertEqual(service.pendingQueueCount, 2)
    }

    @MainActor
    func testGenerateDoctorShareFileCreatesValidJson() {
        let sig = ORKConsentSignature(guardianFullName: "Doctor Share Parent", signatureToken: "SIG-DOC-001")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-DOC-TEST")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        let fileURL = service.generateDoctorShareFile()
        XCTAssertNotNil(fileURL)
        guard let url = fileURL else { return }

        XCTAssertTrue(FileManager.default.fileExists(atPath: url.path))
        let data = (try? Data(contentsOf: url)) ?? Data()
        XCTAssertFalse(data.isEmpty)

        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        XCTAssertNotNil(json)
        XCTAssertEqual(json?["bundleType"] as? String, "ResearchKitStudyPackage")

        try? FileManager.default.removeItem(at: url)
    }

    @MainActor
    func testGenerateDoctorReportPDFCreatesValidPdf() {
        let sig = ORKConsentSignature(guardianFullName: "PDF Parent", signatureToken: "SIG-PDF-001")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-PDF-TEST")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        let fileURL = service.generateDoctorReportPDF(
            careKitCompletionRate: 95.0,
            careKitMedicationsLogged: 10,
            careKitDiaryEntriesLogged: 5
        )
        XCTAssertNotNil(fileURL)
        guard let url = fileURL else { return }

        XCTAssertTrue(FileManager.default.fileExists(atPath: url.path))
        let data = (try? Data(contentsOf: url)) ?? Data()
        XCTAssertFalse(data.isEmpty)
        // PDF header magic bytes %PDF
        XCTAssertTrue(data.starts(with: [0x25, 0x50, 0x44, 0x46]))

        try? FileManager.default.removeItem(at: url)
    }

    @MainActor
    func testSyncNowRequiresParentalConsent() {
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        // Store is unconsented by default
        let exp = expectation(description: "sync completion")
        service.syncNow(forceBundleCompile: false) { success in
            XCTAssertFalse(success)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 3.0)

        if case .failed(let reason) = service.syncState {
            XCTAssertTrue(reason.contains("consent is not active"))
        } else {
            XCTFail("Expected syncState to be failed due to missing consent")
        }
    }

    @MainActor
    func testSyncNowWithSuccessfulTransmissionDrainsQueue() {
        let sig = ORKConsentSignature(guardianFullName: "Consented Parent", signatureToken: "SIG-SYNC-OK")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-SYNC-OK")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        // Enqueue a test payload
        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "valid": true])
        XCTAssertEqual(service.pendingQueueCount, 1)

        MockURLProtocol.requestHandler = { request in
            guard let url = request.url else {
                throw URLError(.badURL)
            }
            let response = HTTPURLResponse(
                url: url,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let responseData = """
            {"status": "ingested", "receipt_id": "RCPT-TEST-778899"}
            """.data(using: .utf8)!
            return (response, responseData)
        }

        let exp = expectation(description: "sync succeeds")
        service.syncNow(forceBundleCompile: false) { success in
            XCTAssertTrue(success)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)

        XCTAssertEqual(service.pendingQueueCount, 0, "Queue must be drained upon successful transmission")
        if case .synced(_, let receipt) = service.syncState {
            XCTAssertEqual(receipt, "RCPT-TEST-778899")
        } else {
            XCTFail("Expected syncState to be synced")
        }
    }

    @MainActor
    func testSyncNowQuarantines400BadRequest() {
        let sig = ORKConsentSignature(guardianFullName: "Consented Parent", signatureToken: "SIG-SYNC-400")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-SYNC-400")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        service.enqueueStudyPayload(bundle: ["malformedPayload": true])
        XCTAssertEqual(service.pendingQueueCount, 1)

        MockURLProtocol.requestHandler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 400,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let responseData = """
            {"error": "Invalid schema"}
            """.data(using: .utf8)!
            return (response, responseData)
        }

        let exp = expectation(description: "sync 400 bad request")
        service.syncNow(forceBundleCompile: false) { success in
            XCTAssertFalse(success)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)

        // Queue must have 0 pending active files (poison pill quarantined)
        XCTAssertEqual(service.pendingQueueCount, 0)
        let quarantineDir = tempQueueDir.appendingPathComponent("Quarantine", isDirectory: true)
        let quarantinedFiles = (try? FileManager.default.contentsOfDirectory(at: quarantineDir, includingPropertiesForKeys: nil)) ?? []
        XCTAssertEqual(quarantinedFiles.count, 1, "Failed file must be safely moved to Quarantine folder")
    }

    @MainActor
    func testSyncNowNetworkFailureLeavesQueueIntact() {
        let sig = ORKConsentSignature(guardianFullName: "Consented Parent", signatureToken: "SIG-SYNC-NET")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-SYNC-NET")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession
        )

        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "retryMe": true])
        XCTAssertEqual(service.pendingQueueCount, 1)

        MockURLProtocol.requestHandler = { _ in
            throw URLError(.cannotConnectToHost)
        }

        let exp = expectation(description: "network error")
        service.syncNow(forceBundleCompile: false) { success in
            XCTAssertFalse(success)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)

        // File must remain in queue for future retry
        XCTAssertEqual(service.pendingQueueCount, 1)
        if case .failed(let reason) = service.syncState {
            XCTAssertTrue(reason.contains("Data safely queued"))
        } else {
            XCTFail("Expected syncState to be failed")
        }
    }

    @MainActor
    func testNetworkReconnectTriggersAutoSyncWhenPendingItemsExist() {
        let sig = ORKConsentSignature(guardianFullName: "Reconnect Parent", signatureToken: "SIG-RECONNECT")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-RECONNECT")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )

        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "autoSync": true])
        XCTAssertEqual(service.pendingQueueCount, 1)

        MockURLProtocol.requestHandler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let data = """
            {"status": "ingested", "receipt_id": "RCPT-AUTO-RECONNECT"}
            """.data(using: .utf8)!
            return (response, data)
        }

        // 1. Transition to offline
        service.handlePathUpdate(status: .unsatisfied)
        XCTAssertFalse(service.isOnline)
        XCTAssertEqual(service.pendingQueueCount, 1)

        // 2. Transition to online -> triggers auto-sync
        service.handlePathUpdate(status: .satisfied)
        XCTAssertTrue(service.isOnline)

        // Wait for background Task to finish and queue to drain
        var drained = false
        for _ in 0..<40 {
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            if service.pendingQueueCount == 0 {
                drained = true
                break
            }
        }
        XCTAssertTrue(drained, "Pending items must be flushed automatically when transitioning back online")
        if case .synced(_, let receipt) = service.syncState {
            XCTAssertEqual(receipt, "RCPT-AUTO-RECONNECT")
        } else {
            XCTFail("Expected syncState to be synced on auto-flush")
        }
    }

    @MainActor
    func testExponentialBackoffTiming() {
        XCTAssertEqual(ResearchSyncRetryPolicy.delays, [1, 2, 4, 8, 60])
        XCTAssertEqual(ResearchSyncRetryPolicy.delay(afterAttempt: 1), 1)
        XCTAssertEqual(ResearchSyncRetryPolicy.delay(afterAttempt: 2), 2)
        XCTAssertEqual(ResearchSyncRetryPolicy.delay(afterAttempt: 3), 4)
        XCTAssertEqual(ResearchSyncRetryPolicy.delay(afterAttempt: 4), 8)
        XCTAssertEqual(ResearchSyncRetryPolicy.delay(afterAttempt: 5), 60)
        XCTAssertEqual(ResearchSyncRetryPolicy.delay(afterAttempt: 99), 60)

        let clock = SyncTestClock()
        let sig = ORKConsentSignature(guardianFullName: "Backoff Parent", signatureToken: "SIG-BACKOFF")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-BACKOFF")
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false,
            now: { clock.instant }
        )
        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "backoff": true])

        let hits = HitCounter()
        MockURLProtocol.requestHandler = { _ in
            hits.count += 1
            throw URLError(.cannotConnectToHost)
        }

        waitForSync(service, forceBundleCompile: false)
        XCTAssertEqual(hits.count, 1)
        var meta = readSyncMeta()
        XCTAssertEqual(meta?.attemptCount, 1)
        XCTAssertEqual(meta?.nextAttemptAt ?? 0, clock.instant.timeIntervalSince1970 + 1, accuracy: 0.001)

        let hitsBeforeEarlyRetry = hits.count
        waitForSync(service, forceBundleCompile: false)
        XCTAssertEqual(hits.count, hitsBeforeEarlyRetry, "A payload must wait out its backoff before the next attempt")
        XCTAssertEqual(service.pendingQueueCount, 1)

        let expectedDelays: [TimeInterval] = [2, 4, 8, 60]
        for delay in expectedDelays {
            clock.advance(120)
            waitForSync(service, forceBundleCompile: false)
            meta = readSyncMeta()
            XCTAssertEqual(meta?.nextAttemptAt ?? 0, clock.instant.timeIntervalSince1970 + delay, accuracy: 0.001)
        }
        XCTAssertEqual(service.deadLetterCount, 0)
    }

    @MainActor
    func testDeadLetterAfterMaxRetries() {
        let clock = SyncTestClock()
        let sig = ORKConsentSignature(guardianFullName: "Dead Letter Parent", signatureToken: "SIG-DEAD")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-DEAD")
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false,
            now: { clock.instant }
        )
        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "exhaust": true])

        MockURLProtocol.requestHandler = { _ in
            throw URLError(.timedOut)
        }

        for _ in 0..<ResearchSyncRetryPolicy.maxAttempts {
            clock.advance(120)
            waitForSync(service, forceBundleCompile: false)
        }

        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.deadLetterCount, 1)
        if case .failed(let reason) = service.syncState {
            XCTAssertTrue(reason.contains("manual retry"))
        } else {
            XCTFail("Expected a stuck-item failure after the retry budget")
        }
    }

    @MainActor
    func testManualRetryFromDeadLetter() {
        let clock = SyncTestClock()
        let sig = ORKConsentSignature(guardianFullName: "Retry Parent", signatureToken: "SIG-RETRY")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-RETRY")
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false,
            now: { clock.instant }
        )
        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "retryStuck": true])

        MockURLProtocol.requestHandler = { _ in
            throw URLError(.networkConnectionLost)
        }
        for _ in 0..<ResearchSyncRetryPolicy.maxAttempts {
            clock.advance(120)
            waitForSync(service, forceBundleCompile: false)
        }
        XCTAssertEqual(service.deadLetterCount, 1)

        service.retryDeadLetterItems()
        XCTAssertEqual(service.pendingQueueCount, 1)
        XCTAssertEqual(service.deadLetterCount, 0)

        MockURLProtocol.requestHandler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let data = """
            {"status": "ingested", "receipt_id": "RCPT-RETRY-STUCK"}
            """.data(using: .utf8)!
            return (response, data)
        }

        waitForSync(service, forceBundleCompile: false)
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.deadLetterCount, 0)
        XCTAssertNotNil(service.lastSyncedAt)
        if case .synced(_, let receipt) = service.syncState {
            XCTAssertEqual(receipt, "RCPT-RETRY-STUCK")
        } else {
            XCTFail("Expected the retried payload to sync")
        }
    }

    // MARK: - CNS-4 queue location and protection

    @MainActor
    func testDefaultQueueDirectoryIsApplicationSupportNotDocuments() {
        let dir = ResearchKitSyncService.defaultQueueDirectory()
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        XCTAssertTrue(dir.standardizedFileURL.path.hasPrefix(appSupport.standardizedFileURL.path))
        XCTAssertFalse(dir.standardizedFileURL.path.hasPrefix(documents.standardizedFileURL.path))
        XCTAssertEqual(dir.lastPathComponent, "ResearchStudyQueue")
    }

    @MainActor
    func testQueueDirectoryIsExcludedFromBackup() throws {
        _ = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        let values = try tempQueueDir.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)
    }

    @MainActor
    func testEnqueuedPayloadIsWrittenWithFileProtection() throws {
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        service.enqueueStudyPayload(bundle: ["study": "CURV-ASD", "protected": true])

        let file = try XCTUnwrap(jsonFiles(in: tempQueueDir).first)
        XCTAssertTrue(ResearchKitSyncService.writeOptions.contains(.completeFileProtectionUntilFirstUserAuthentication))
        XCTAssertTrue(ResearchKitSyncService.writeOptions.contains(.atomic))
        #if !targetEnvironment(simulator)
        // The simulator neither records nor reports per-file protection classes.
        let attrs = try FileManager.default.attributesOfItem(atPath: file.path)
        XCTAssertEqual(
            attrs[.protectionKey] as? FileProtectionType,
            .completeUntilFirstUserAuthentication
        )
        #else
        XCTAssertTrue(FileManager.default.fileExists(atPath: file.path))
        #endif
    }

    @MainActor
    func testDeadLetterAndQuarantineFoldersAreExcludedFromBackup() throws {
        let sig = ORKConsentSignature(guardianFullName: "Folder Parent", signatureToken: "SIG-FOLDERS")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-FOLDERS")
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        service.enqueueStudyPayload(bundle: ["bad": true])
        MockURLProtocol.requestHandler = { request in
            (HTTPURLResponse(url: request.url!, statusCode: 422, httpVersion: nil, headerFields: nil)!, Data())
        }
        waitForSync(service, forceBundleCompile: false)

        let quarantine = tempQueueDir.appendingPathComponent("Quarantine", isDirectory: true)
        XCTAssertEqual(jsonFiles(in: quarantine).count, 1)
        let values = try quarantine.resourceValues(forKeys: [.isExcludedFromBackupKey])
        XCTAssertEqual(values.isExcludedFromBackup, true)
    }

    // MARK: - CNS-5 purge on revoke

    @MainActor
    func testPurgeAllQueuedEmptiesQueueDeadLetterAndQuarantine() {
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        for index in 0..<3 {
            service.enqueueStudyPayload(bundle: ["payload": index])
        }
        seedFile(in: "DeadLetter", name: "stuck.json")
        seedFile(in: "Quarantine", name: "poison.json")
        try? Data("{}".utf8).write(to: tempQueueDir.appendingPathComponent("orphan.syncmeta"))
        service.updatePendingCount()
        XCTAssertEqual(service.pendingQueueCount, 3)
        XCTAssertEqual(service.deadLetterCount, 1)

        service.purgeAllQueued()

        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(service.deadLetterCount, 0)
        XCTAssertEqual(allFiles(under: tempQueueDir), [], "Queue, dead-letter and quarantine must be empty")
        XCTAssertEqual(service.syncState, .idle)
    }

    @MainActor
    func testRevokeConsentPurgesQueueAndReconnectUploadsNothing() {
        let box = SyncServiceBox()
        let store = ResearchKitStudyStore(
            userDefaults: testUserDefaults,
            queuePurger: ResearchKitStudyStore.mainActorPurger { box.service?.purgeAllQueued() }
        )
        let sig = ORKConsentSignature(guardianFullName: "Revoke Parent", signatureToken: "SIG-REVOKE")
        store.saveConsent(signature: sig, participantId: "STUDY-REVOKE")

        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: store,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        box.service = service

        service.handlePathUpdate(status: .unsatisfied)
        for index in 0..<3 {
            service.enqueueStudyPayload(bundle: ["offline": index])
        }
        seedFile(in: "DeadLetter", name: "stuck.json")
        seedFile(in: "Quarantine", name: "poison.json")
        XCTAssertEqual(service.pendingQueueCount, 3)

        store.revokeConsent()

        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(allFiles(under: tempQueueDir), [])

        let hits = HitCounter()
        MockURLProtocol.requestHandler = { request in
            hits.count += 1
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data())
        }
        service.handlePathUpdate(status: .satisfied)
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertEqual(hits.count, 0, "Nothing may upload after consent is revoked")
    }

    @MainActor
    func testPurgeCancelsInFlightUploadAndLeavesNoFilesBehind() {
        let sig = ORKConsentSignature(guardianFullName: "Inflight Parent", signatureToken: "SIG-INFLIGHT")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-INFLIGHT")
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        service.enqueueStudyPayload(bundle: ["inflight": true])

        let started = DispatchSemaphore(value: 0)
        let release = DispatchSemaphore(value: 0)
        MockURLProtocol.requestHandler = { _ in
            started.signal()
            _ = release.wait(timeout: .now() + 3)
            throw URLError(.cannotConnectToHost)
        }

        let done = expectation(description: "in-flight sync finished")
        let result = ResultBox()
        service.syncNow(forceBundleCompile: false) { success in
            result.value = success
            done.fulfill()
        }

        var didStart = false
        for _ in 0..<60 {
            if started.wait(timeout: .now()) == .success {
                didStart = true
                break
            }
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        }
        XCTAssertTrue(didStart, "Upload never started")

        service.purgeAllQueued()
        release.signal()
        wait(for: [done], timeout: 5.0)

        XCTAssertEqual(result.value, false)
        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(allFiles(under: tempQueueDir), [], "A cancelled upload must not write retry metadata or move files")
        XCTAssertEqual(service.syncState, .idle)
    }

    // MARK: - SIM-5 study store refuses simulated sessions

    func testRecordActiveStepResultRefusesSimulatedSummary() throws {
        let sig = ORKConsentSignature(guardianFullName: "Sim Parent", signatureToken: "SIG-SIM")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-SIM")

        let simulated = try Self.summary(captureSource: "simulated")
        XCTAssertNil(researchStore.recordActiveStepResult(summary: simulated))
        XCTAssertTrue(researchStore.fetchCompletedTaskResults().isEmpty)

        let live = try Self.summary(captureSource: "truedepth")
        XCTAssertNotNil(researchStore.recordActiveStepResult(summary: live))
        XCTAssertEqual(researchStore.fetchCompletedTaskResults().count, 1)
    }

    // MARK: - CNS-9 configuration-driven, https only

    /// The default build carries no `ASDResearchStudy` key, so there is no
    /// shared service and nothing in the app can upload.
    @MainActor
    func testSharedServiceIsNilWithoutStudyConfiguration() {
        XCTAssertNil(ResearchStudyConfiguration.current)
        XCTAssertNil(ResearchKitSyncService.shared)
    }

    @MainActor
    func testServiceExposesItsConfiguration() {
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        XCTAssertEqual(service.configuration, Self.studyConfig)
        XCTAssertEqual(service.configuration.ingestURL.scheme, "https")
    }

    @MainActor
    func testUploadsGoOnlyToTheConfiguredHttpsEndpoint() {
        let sig = ORKConsentSignature(guardianFullName: "Endpoint Parent", signatureToken: "SIG-ENDPOINT")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-ENDPOINT")
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        service.enqueueStudyPayload(bundle: ["endpoint": true])

        let seen = URLRecorder()
        MockURLProtocol.requestHandler = { request in
            seen.urls.append(request.url)
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data())
        }
        waitForSync(service, forceBundleCompile: false)

        XCTAssertEqual(seen.urls, [Self.studyConfig.ingestURL])
        XCTAssertEqual(seen.urls.first??.scheme, "https")
        XCTAssertNotEqual(seen.urls.first??.host, "127.0.0.1")
        XCTAssertEqual(service.pendingQueueCount, 0)
    }

    /// Delete-all and revoke must still clear a queue left on disk by an earlier
    /// build that had a study configured, even when this build has none (CNS-6).
    @MainActor
    func testPurgeQueuedUploadsWithoutServiceEmptiesQueueDirectory() {
        for name in ["a.json", "a.syncmeta"] {
            try? Data("{}".utf8).write(to: tempQueueDir.appendingPathComponent(name))
        }
        seedFile(in: "DeadLetter", name: "stuck.json")
        seedFile(in: "Quarantine", name: "poison.json")
        XCTAssertEqual(allFiles(under: tempQueueDir).count, 4)

        ResearchKitSyncService.purgeQueuedUploads(service: nil, queueDirectory: tempQueueDir)

        XCTAssertEqual(allFiles(under: tempQueueDir), [])
    }

    @MainActor
    func testPurgeQueuedUploadsDelegatesToService() {
        let service = ResearchKitSyncService(
            configuration: Self.studyConfig,
            store: researchStore,
            queueDirectory: tempQueueDir,
            session: mockSession,
            startMonitoring: false
        )
        service.enqueueStudyPayload(bundle: ["purge": true])
        XCTAssertEqual(service.pendingQueueCount, 1)

        ResearchKitSyncService.purgeQueuedUploads(service: service, queueDirectory: tempQueueDir)

        XCTAssertEqual(service.pendingQueueCount, 0)
        XCTAssertEqual(allFiles(under: tempQueueDir), [])
    }

    @MainActor
    func testPurgeQueuedUploadsToleratesMissingDirectory() {
        let missing = tempQueueDir.appendingPathComponent("does-not-exist", isDirectory: true)
        ResearchKitSyncService.purgeQueuedUploads(service: nil, queueDirectory: missing)
        XCTAssertFalse(FileManager.default.fileExists(atPath: missing.path))
    }

    // MARK: - Helpers

    /// Decoded from JSON so the fixture does not depend on the summary's initialiser shape.
    private static func summary(captureSource: String) throws -> GazeSessionReportSummary {
        let json = """
        {
          "sessionId": "SIM5TEST",
          "durationSec": 30,
          "totalFrames": 1800,
          "validFrames": 1500,
          "validRatio": 0.83,
          "socialPreferenceScore": 0.6,
          "socialDwellSec": 12,
          "geometricDwellSec": 8,
          "eyeDwellRatio": 0.3,
          "mouthDwellRatio": 0.1,
          "firstEyeLatencyMs": 500,
          "socialSmileIntensityMean": 0.4,
          "smileReactivityIndex": 0.3,
          "flatAffectScore": 0.1,
          "surpriseScoreMean": 0.2,
          "qualityVerdict": "optimal",
          "captureSource": "\(captureSource)",
          "interruptionCount": 0,
          "interruptedSec": 0
        }
        """
        return try JSONDecoder().decode(GazeSessionReportSummary.self, from: Data(json.utf8))
    }

    private func seedFile(in folder: String, name: String) {
        let dir = tempQueueDir.appendingPathComponent(folder, isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? Data("{}".utf8).write(to: dir.appendingPathComponent(name))
    }

    private func jsonFiles(in dir: URL) -> [URL] {
        ((try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? [])
            .filter { $0.pathExtension == "json" }
    }

    /// Every regular file below `dir`, as paths relative to it.
    private func allFiles(under dir: URL) -> [String] {
        guard let enumerator = FileManager.default.enumerator(
            at: dir,
            includingPropertiesForKeys: [.isRegularFileKey]
        ) else { return [] }
        var files: [String] = []
        for case let url as URL in enumerator {
            let isFile = (try? url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) ?? false
            if isFile { files.append(url.lastPathComponent) }
        }
        return files.sorted()
    }

    @MainActor
    private func waitForSync(_ service: ResearchKitSyncService, forceBundleCompile: Bool) {
        let exp = expectation(description: "sync")
        service.syncNow(forceBundleCompile: forceBundleCompile) { _ in
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
    }

    private func readSyncMeta() -> ResearchSyncFileMetaSnapshot? {
        let files = (try? FileManager.default.contentsOfDirectory(at: tempQueueDir, includingPropertiesForKeys: nil)) ?? []
        guard let metaURL = files.first(where: { $0.pathExtension == "syncmeta" }),
              let data = try? Data(contentsOf: metaURL) else {
            return nil
        }
        return try? JSONDecoder().decode(ResearchSyncFileMetaSnapshot.self, from: data)
    }
}

private struct ResearchSyncFileMetaSnapshot: Codable {
    var attemptCount: Int
    var nextAttemptAt: TimeInterval
}

private final class SyncTestClock: @unchecked Sendable {
    var instant = Date(timeIntervalSince1970: 1_700_000_000)
    func advance(_ seconds: TimeInterval) {
        instant = instant.addingTimeInterval(seconds)
    }
}

private final class HitCounter: @unchecked Sendable {
    var count = 0
}

private final class URLRecorder: @unchecked Sendable {
    var urls: [URL?] = []
}

private final class ResultBox: @unchecked Sendable {
    var value: Bool?
}

@MainActor
private final class SyncServiceBox: @unchecked Sendable {
    weak var service: ResearchKitSyncService?
}
