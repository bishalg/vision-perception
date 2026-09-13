@testable import AsdGazeCollector
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
    private var testUserDefaults: UserDefaults!
    private var researchStore: ResearchKitStudyStore!
    private var tempQueueDir: URL!
    private var mockSession: URLSession!

    override func setUp() {
        super.setUp()
        testUserDefaults = UserDefaults(suiteName: "ResearchKitSyncServiceTests_\(UUID().uuidString)")!
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
        testUserDefaults.removePersistentDomain(forName: testUserDefaults.description)
        testUserDefaults = nil
        researchStore = nil
        mockSession = nil
        super.tearDown()
    }

    @MainActor
    func testEnqueuePayloadIncrementsPendingCount() {
        let service = ResearchKitSyncService(
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
    func testSyncNowRequiresParentalConsent() {
        let service = ResearchKitSyncService(
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
}
