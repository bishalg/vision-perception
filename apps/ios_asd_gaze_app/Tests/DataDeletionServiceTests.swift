@testable import AsdGazeCollector
import CurvAsdGazeKit
import Foundation
import XCTest

/// CNS-6: "Delete all observation data" empties every on-device store; care plans
/// are deleted only on the separate request.
@MainActor
final class DataDeletionServiceTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!
    private var root: URL!
    private var vaultDir: URL!
    private var blendShapeDir: URL!
    private var queueDir: URL!
    private var researchStore: ResearchKitStudyStore!
    private var historyStore: GazeSessionHistoryStore!
    private var syncService: ResearchKitSyncService!
    private var calibrationResets = 0
    private var carePlanDeletes = 0

    private static let historyKey = "org.curv.asd.gaze_session_history"

    override func setUp() {
        super.setUp()
        suiteName = "DataDeletionServiceTests_\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
        root = FileManager.default.temporaryDirectory
            .appendingPathComponent("DataDeletion_\(UUID().uuidString)", isDirectory: true)
        vaultDir = root.appendingPathComponent("vault", isDirectory: true)
        blendShapeDir = root.appendingPathComponent("BlendShapeML", isDirectory: true)
        queueDir = root.appendingPathComponent("ResearchStudyQueue", isDirectory: true)
        for dir in [vaultDir!, blendShapeDir!, queueDir!] {
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        calibrationResets = 0
        carePlanDeletes = 0

        let box = DeletionSyncBox()
        researchStore = ResearchKitStudyStore(
            userDefaults: defaults,
            queuePurger: ResearchKitStudyStore.mainActorPurger { box.service?.purgeAllQueued() }
        )
        syncService = ResearchKitSyncService(
            configuration: ResearchKitSyncServiceTests.studyConfig,
            store: researchStore,
            queueDirectory: queueDir,
            startMonitoring: false
        )
        box.service = syncService

        defaults.set(Data(Self.legacyHistoryJSON.utf8), forKey: Self.historyKey)
        historyStore = GazeSessionHistoryStore(defaults: defaults)
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: root)
        defaults.removePersistentDomain(forName: suiteName)
        syncService = nil
        historyStore = nil
        researchStore = nil
        defaults = nil
        super.tearDown()
    }

    func testDeleteAllEmptiesEveryObservationStore() throws {
        seedEverything()

        let service = makeService()
        let report = service.deleteAll(includeCarePlans: false)

        XCTAssertEqual(report.failures, [])
        XCTAssertEqual(contents(of: vaultDir), [], "Vault directory must be empty")
        XCTAssertFalse(FileManager.default.fileExists(atPath: blendShapeDir.path), "BlendShapeML must be removed")
        XCTAssertEqual(regularFiles(under: queueDir), [], "No sync queue files may remain")
        XCTAssertEqual(syncService.pendingQueueCount, 0)
        XCTAssertTrue(historyStore.records.isEmpty)
        XCTAssertNil(defaults.data(forKey: Self.historyKey))
        XCTAssertEqual(calibrationResets, 1)

        let enrollment = researchStore.fetchEnrollment()
        XCTAssertFalse(enrollment.isConsented)
        XCTAssertNil(enrollment.consentSignature)
        XCTAssertNotEqual(enrollment.participantId, "STUDY-DELETE", "Enrollment must be cleared")
        XCTAssertTrue(researchStore.fetchCompletedSurveys().isEmpty, "M-CHAT results must be deleted")
        XCTAssertTrue(researchStore.fetchCompletedTaskResults().isEmpty)
    }

    func testDeleteAllLeavesCarePlansUnlessAskedSeparately() {
        let service = makeService()

        service.deleteAll(includeCarePlans: false)
        XCTAssertEqual(carePlanDeletes, 0, "Care plans must never be deleted silently")

        service.deleteAll(includeCarePlans: true)
        XCTAssertEqual(carePlanDeletes, 1)

        service.deleteCarePlans()
        XCTAssertEqual(carePlanDeletes, 2)
    }

    func testDeleteAllToleratesMissingDirectories() throws {
        try FileManager.default.removeItem(at: blendShapeDir)
        try FileManager.default.removeItem(at: vaultDir)

        let report = makeService().deleteAll(includeCarePlans: false)

        XCTAssertEqual(report.failures, [], "Absent stores are already deleted, not failures")
    }

    func testDefaultBlendShapeDirectoryIsApplicationSupport() {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let dir = DataDeletionService.defaultBlendShapeDirectory()
        XCTAssertEqual(dir.lastPathComponent, "BlendShapeML")
        XCTAssertEqual(
            dir.deletingLastPathComponent().standardizedFileURL.path,
            appSupport.standardizedFileURL.path
        )
    }

    // MARK: - Helpers

    private func makeService() -> DataDeletionService {
        DataDeletionService(
            vaultDirectory: vaultDir,
            blendShapeDirectory: blendShapeDir,
            historyStore: historyStore,
            researchStore: researchStore,
            purgeQueues: { [syncService] in syncService?.purgeAllQueued() },
            resetCalibration: { [weak self] in self?.calibrationResets += 1 },
            deleteCarePlans: { [weak self] in self?.carePlanDeletes += 1 }
        )
    }

    private func seedEverything() {
        try? Data("cipher".utf8).write(to: vaultDir.appendingPathComponent("session-A.jsonl.enc"))
        try? Data("cipher".utf8).write(to: vaultDir.appendingPathComponent("session-B.sim.jsonl.enc"))
        try? Data("a,b\n".utf8).write(to: blendShapeDir.appendingPathComponent("blendshapes-1.csv"))

        let sig = ORKConsentSignature(guardianFullName: "Delete Parent", signatureToken: "SIG-DELETE")
        researchStore.saveConsent(signature: sig, participantId: "STUDY-DELETE")
        researchStore.recordSurveyResult(answers: [1: true, 2: false])
        XCTAssertFalse(researchStore.fetchCompletedSurveys().isEmpty)

        syncService.enqueueStudyPayload(bundle: ["queued": 1])
        syncService.enqueueStudyPayload(bundle: ["queued": 2])
        let deadLetter = queueDir.appendingPathComponent("DeadLetter", isDirectory: true)
        try? FileManager.default.createDirectory(at: deadLetter, withIntermediateDirectories: true)
        try? Data("{}".utf8).write(to: deadLetter.appendingPathComponent("stuck.json"))
        XCTAssertEqual(regularFiles(under: queueDir).count, 3)
        XCTAssertNotNil(defaults.data(forKey: Self.historyKey))
    }

    private func contents(of dir: URL) -> [String] {
        ((try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []).sorted()
    }

    private func regularFiles(under dir: URL) -> [String] {
        guard let enumerator = FileManager.default.enumerator(
            at: dir,
            includingPropertiesForKeys: [.isRegularFileKey]
        ) else { return [] }
        var files: [String] = []
        for case let url as URL in enumerator {
            if (try? url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true {
                files.append(url.lastPathComponent)
            }
        }
        return files.sorted()
    }

    /// One v1.5 history record, as stored before provenance existed.
    private static let legacyHistoryJSON = """
    [{
      "id": "REC-1",
      "sessionId": "SESS-1",
      "participantId": "child-SESS0001",
      "startedAt": 700000000,
      "endedAt": 700000030,
      "durationSec": 30,
      "socialPreferenceScore": 0.4,
      "validFrameRatio": 0.9,
      "eyeDwellRatio": 0.3,
      "mouthDwellRatio": 0.1,
      "firstEyeLatencyMs": 500,
      "calibratedDistanceCm": 35,
      "qualityVerdict": "optimal",
      "socialSmileIntensity": 0.4,
      "smileReactivityIndex": 0.3,
      "flatAffectScore": 0.1,
      "surpriseScoreMean": 0.2,
      "schemaVersion": "v1.5",
      "zeroRawVideoEnforced": true
    }]
    """
}

@MainActor
private final class DeletionSyncBox: @unchecked Sendable {
    weak var service: ResearchKitSyncService?
}
