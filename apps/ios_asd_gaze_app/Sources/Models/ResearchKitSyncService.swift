import Combine
import CryptoKit
import Foundation
import Network
import os

/// State of research study synchronization with the clinical backend.
public enum ResearchSyncState: Sendable, Equatable {
    case idle
    case syncing
    case synced(timestamp: Date, receiptId: String)
    case failed(reason: String)

    public var isSyncing: Bool {
        if case .syncing = self { return true }
        return false
    }

    public var description: String {
        switch self {
        case .idle: return "Ready"
        case .syncing: return "Transmitting study data..."
        case .synced(let ts, let receipt):
            let f = DateFormatter()
            f.timeStyle = .short
            return "Synced at \(f.string(from: ts)) (Receipt #\(receipt.prefix(8)))"
        case .failed(let err): return "Sync error: \(err)"
        }
    }
}

/// Transient-failure schedule for the research upload queue.
/// Delays are 1s, 2s, 4s, 8s, then 60s. The next failure after that moves the payload to the dead-letter folder.
public enum ResearchSyncRetryPolicy {
    public static let delays: [TimeInterval] = [1, 2, 4, 8, 60]
    /// Failures tolerated before a payload is stuck. One past the last delay.
    public static let maxAttempts = delays.count + 1

    public static func delay(afterAttempt attempt: Int) -> TimeInterval {
        let index = min(max(attempt - 1, 0), delays.count - 1)
        return delays[index]
    }
}

private struct ResearchSyncFileMeta: Codable {
    var attemptCount: Int
    var nextAttemptAt: TimeInterval
}

/// Offline-first background synchronization client for a configured research
/// study's ingest endpoint (CNS-9).
///
/// The service exists only for a build whose Info.plist carries a valid
/// `ASDResearchStudy` dictionary: `shared` is `nil` otherwise, and the init
/// requires a `ResearchStudyConfiguration`, which cannot be created without an
/// https, non-IP-literal ingest URL. Every request re-checks the scheme before
/// it is sent, so nothing relies on ATS (trap 10).
@MainActor
public final class ResearchKitSyncService: ObservableObject {
    /// `nil` in the default build: there is no research study at launch (H7).
    public static let shared: ResearchKitSyncService? = ResearchStudyConfiguration.current.map {
        ResearchKitSyncService(configuration: $0)
    }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ResearchKitSyncService")

    @Published public private(set) var syncState: ResearchSyncState = .idle
    @Published public private(set) var pendingQueueCount: Int = 0
    @Published public private(set) var deadLetterCount: Int = 0
    @Published public private(set) var lastSyncedAt: Date?
    @Published public private(set) var isOnline: Bool = true

    public let configuration: ResearchStudyConfiguration
    private let store: ResearchKitStudyStore
    private var serverURL: URL { configuration.ingestURL }
    private let fileManager = FileManager.default
    private let queueDirectory: URL
    private let session: URLSession
    private let pathMonitor: NWPathMonitor?
    private let monitorQueue = DispatchQueue(label: "org.curv.researchkit.network", qos: .utility)
    private let now: @Sendable () -> Date

    private static let deadLetterFolder = "DeadLetter"
    private static let quarantineFolder = "Quarantine"
    static let queueFolder = "ResearchStudyQueue"

    /// Queue payloads may be flushed on reconnect while the device is locked,
    /// hence "until first user authentication" rather than `.complete` (CNS-4).
    private static let fileProtection: FileProtectionType = .completeUntilFirstUserAuthentication
    /// Options for every queue and retry-metadata write. Internal for tests:
    /// the iOS Simulator does not record per-file protection, so tests assert
    /// this contract there and the real attribute only on a device.
    static let writeOptions: Data.WritingOptions = [.atomic, .completeFileProtectionUntilFirstUserAuthentication]

    /// The running flush, cancelled by `purgeAllQueued()` (CNS-5).
    private var syncTask: Task<Void, Never>?
    /// Bumped on every purge so a flush that outlives its cancellation never
    /// writes retry metadata or moves files back into a purged queue.
    private var queueGeneration = 0

    /// Application Support (not Documents) so the queue is not user-visible and
    /// can be excluded from backup (CNS-4).
    public static func defaultQueueDirectory(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return base.appendingPathComponent(queueFolder, isDirectory: true)
    }

    /// Deletes every queued, dead-lettered and quarantined upload. Works without
    /// a service too: a queue left on disk by an earlier build that had a study
    /// configured is still removed when this build has none (CNS-5, CNS-6).
    public static func purgeQueuedUploads(
        service: ResearchKitSyncService? = ResearchKitSyncService.shared,
        queueDirectory: URL = ResearchKitSyncService.defaultQueueDirectory(),
        fileManager: FileManager = .default
    ) {
        if let service {
            service.purgeAllQueued()
            return
        }
        guard fileManager.fileExists(atPath: queueDirectory.path) else { return }
        let contents = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        for item in contents {
            do {
                try fileManager.removeItem(at: item)
            } catch {
                logger.error("Failed to purge \(item.lastPathComponent, privacy: .public): \(error.localizedDescription, privacy: .public)")
            }
        }
        logger.info("Purged research upload queue left by an earlier build")
    }

    public init(
        configuration: ResearchStudyConfiguration,
        store: ResearchKitStudyStore = .shared,
        queueDirectory: URL? = nil,
        session: URLSession = .shared,
        startMonitoring: Bool = true,
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.configuration = configuration
        self.store = store
        self.session = session
        self.now = now

        self.queueDirectory = queueDirectory ?? Self.defaultQueueDirectory()
        if startMonitoring {
            self.pathMonitor = NWPathMonitor()
        } else {
            self.pathMonitor = nil
        }
        prepareProtectedDirectory(self.queueDirectory)
        updatePendingCount()

        if let monitor = self.pathMonitor {
            monitor.pathUpdateHandler = { [weak self] path in
                Task { @MainActor in
                    self?.handlePathUpdate(status: path.status)
                }
            }
            monitor.start(queue: monitorQueue)
        }
    }

    deinit {
        pathMonitor?.cancel()
    }

    /// Internal handler when network connectivity transitions
    public func handlePathUpdate(status: NWPath.Status) {
        let wasOnline = isOnline
        isOnline = (status == .satisfied)
        Self.logger.info("Network path status changed to \(self.isOnline ? "online" : "offline", privacy: .public)")

        // If we transitioned from offline to online and have pending items, trigger background flush
        if !wasOnline && isOnline && pendingQueueCount > 0 && !syncState.isSyncing {
            Self.logger.info("Auto-triggering flush of \(self.pendingQueueCount) queued research study bundles on network reconnect")
            syncNow(forceBundleCompile: false)
        }
    }

    // MARK: - Queue Management

    /// Enqueue a newly compiled study bundle for background dispatch.
    public func enqueueStudyPayload(bundle: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: bundle, options: [.prettyPrinted, .sortedKeys]) else {
            Self.logger.error("Failed to serialize study payload for queuing")
            return
        }

        let fileName = "study_payload_\(UUID().uuidString).json"
        let fileURL = queueDirectory.appendingPathComponent(fileName)
        do {
            prepareProtectedDirectory(queueDirectory)
            try data.write(to: fileURL, options: Self.writeOptions)
            Self.logger.info("Enqueued research study payload to \(fileName, privacy: .public)")
            updatePendingCount()
        } catch {
            Self.logger.error("Failed to write research study payload: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Update the count of pending transmissions in the sandbox queue.
    public func updatePendingCount() {
        let contents = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        pendingQueueCount = contents.filter { $0.pathExtension == "json" }.count
        let stuck = (try? fileManager.contentsOfDirectory(at: deadLetterDirectory, includingPropertiesForKeys: nil)) ?? []
        deadLetterCount = stuck.filter { $0.pathExtension == "json" }.count
    }

    /// Deletes every queued, dead-lettered and quarantined payload and cancels
    /// any in-flight upload. Called when research consent is revoked (CNS-5)
    /// and by "Delete all observation data" (CNS-6).
    public func purgeAllQueued() {
        queueGeneration += 1
        syncTask?.cancel()
        syncTask = nil

        let contents = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        for item in contents {
            do {
                try fileManager.removeItem(at: item)
            } catch {
                Self.logger.error("Failed to purge \(item.lastPathComponent, privacy: .public): \(error.localizedDescription, privacy: .public)")
            }
        }
        syncState = .idle
        updatePendingCount()
        Self.logger.info("Purged research upload queue, dead-letter and quarantine folders")
    }

    /// Move stuck payloads back onto the live queue so the next transmit can try them again.
    public func retryDeadLetterItems() {
        let stuck = (try? fileManager.contentsOfDirectory(at: deadLetterDirectory, includingPropertiesForKeys: nil)) ?? []
        for file in stuck where file.pathExtension == "json" {
            let dest = queueDirectory.appendingPathComponent(file.lastPathComponent)
            try? fileManager.removeItem(at: dest)
            try? fileManager.removeItem(at: metaURL(for: dest))
            try? fileManager.moveItem(at: file, to: dest)
        }
        updatePendingCount()
    }

    // MARK: - Synchronization Dispatcher

    /// Flush all queued payloads or compile and transmit the latest study bundle.
    public func syncNow(forceBundleCompile: Bool = true, completion: (@Sendable (Bool) -> Void)? = nil) {
        guard !syncState.isSyncing else {
            completion?(false)
            return
        }

        let enrollment = store.fetchEnrollment()
        guard enrollment.isConsented else {
            syncState = .failed(reason: "Parental research consent is not active.")
            completion?(false)
            return
        }

        if forceBundleCompile {
            let observations = MoodDiaryStore.shared.recentEntries.map { DiaryObservationRecord(entry: $0) }
            let latestBundle = store.compileStudyBundle(diaryObservations: observations)
            enqueueStudyPayload(bundle: latestBundle)
        }

        updatePendingCount()
        guard pendingQueueCount > 0 else {
            syncState = .idle
            completion?(true)
            return
        }

        syncState = .syncing
        Self.logger.info("Starting research upload to \(self.serverURL.host ?? "?", privacy: .public)")

        let generation = queueGeneration
        syncTask = Task {
            defer { if generation == queueGeneration { syncTask = nil } }
            let files = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
            let jsonFiles = files.filter { $0.pathExtension == "json" }
            let startedAt = now()
            var successCount = 0
            var transientFailures = 0
            var deferredCount = 0
            var deadLettered = 0
            var lastReceipt = "RCPT-\(UUID().uuidString.prefix(6))"

            for file in jsonFiles {
                guard generation == queueGeneration, !Task.isCancelled else { break }
                let meta = loadMeta(for: file)
                if meta.attemptCount > 0 && meta.nextAttemptAt > startedAt.timeIntervalSince1970 {
                    deferredCount += 1
                    continue
                }
                guard let data = try? Data(contentsOf: file) else { continue }
                do {
                    let receipt = try await postPayload(data: data)
                    guard generation == queueGeneration else { break }
                    lastReceipt = receipt
                    try? fileManager.removeItem(at: file)
                    try? fileManager.removeItem(at: metaURL(for: file))
                    successCount += 1
                } catch let err as NSError {
                    // Purged while this upload was in flight: the file is gone,
                    // so record nothing about it.
                    guard generation == queueGeneration, !Task.isCancelled else { break }
                    Self.logger.error("Transmission failed for \(file.lastPathComponent, privacy: .public): \(err.localizedDescription, privacy: .public)")
                    // 4xx status codes indicate permanent payload/client errors.
                    // Quarantine so this file does not block future sync runs.
                    if (400...499).contains(err.code) {
                        moveAside(file, folderName: Self.quarantineFolder)
                        Self.logger.warning("Quarantined invalid payload \(file.lastPathComponent, privacy: .public) (HTTP \(err.code))")
                    } else {
                        let nextCount = meta.attemptCount + 1
                        if nextCount >= ResearchSyncRetryPolicy.maxAttempts {
                            moveAside(file, folderName: Self.deadLetterFolder)
                            deadLettered += 1
                            Self.logger.warning("Dead-lettered \(file.lastPathComponent, privacy: .public) after \(nextCount) attempts")
                        } else {
                            let delay = ResearchSyncRetryPolicy.delay(afterAttempt: nextCount)
                            let updated = ResearchSyncFileMeta(
                                attemptCount: nextCount,
                                nextAttemptAt: now().timeIntervalSince1970 + delay
                            )
                            saveMeta(updated, for: file)
                            transientFailures += 1
                            Self.logger.info("Scheduled retry \(nextCount) for \(file.lastPathComponent, privacy: .public) in \(delay, privacy: .public)s")
                        }
                    }
                }
            }

            guard generation == queueGeneration else {
                Self.logger.info("Sync cancelled by queue purge")
                completion?(false)
                return
            }

            updatePendingCount()

            if successCount > 0 {
                let syncedAt = now()
                lastSyncedAt = syncedAt
                syncState = .synced(timestamp: syncedAt, receiptId: lastReceipt)
                Self.logger.info("Successfully synced \(successCount) ResearchKit bundle(s). Receipt: \(lastReceipt, privacy: .public)")
                completion?(true)
            } else if deadLettered > 0 && pendingQueueCount == 0 {
                syncState = .failed(reason: "Stuck items need a manual retry. Data safely queued.")
                completion?(false)
            } else {
                Self.logger.info("Sync held \(transientFailures) retryable failures and \(deferredCount) waiting payloads")
                syncState = .failed(reason: "Unable to reach research server. Data safely queued.")
                completion?(false)
            }
        }
    }

    private var deadLetterDirectory: URL {
        queueDirectory.appendingPathComponent(Self.deadLetterFolder, isDirectory: true)
    }

    private func metaURL(for file: URL) -> URL {
        file.deletingPathExtension().appendingPathExtension("syncmeta")
    }

    private func loadMeta(for file: URL) -> ResearchSyncFileMeta {
        guard let data = try? Data(contentsOf: metaURL(for: file)),
              let meta = try? JSONDecoder().decode(ResearchSyncFileMeta.self, from: data) else {
            return ResearchSyncFileMeta(attemptCount: 0, nextAttemptAt: 0)
        }
        return meta
    }

    private func saveMeta(_ meta: ResearchSyncFileMeta, for file: URL) {
        guard let data = try? JSONEncoder().encode(meta) else { return }
        try? data.write(to: metaURL(for: file), options: Self.writeOptions)
    }

    private func moveAside(_ file: URL, folderName: String) {
        let dir = queueDirectory.appendingPathComponent(folderName, isDirectory: true)
        prepareProtectedDirectory(dir)
        let dest = dir.appendingPathComponent(file.lastPathComponent)
        try? fileManager.removeItem(at: dest)
        try? fileManager.moveItem(at: file, to: dest)
        try? fileManager.removeItem(at: metaURL(for: file))
    }

    /// Creates `dir` with queue file protection and marks it excluded from
    /// iCloud / Finder backups (CNS-4). Idempotent.
    private func prepareProtectedDirectory(_ dir: URL) {
        do {
            try fileManager.createDirectory(
                at: dir,
                withIntermediateDirectories: true,
                attributes: [.protectionKey: Self.fileProtection]
            )
            var url = dir
            var values = URLResourceValues()
            values.isExcludedFromBackup = true
            try url.setResourceValues(values)
        } catch {
            Self.logger.error("Failed to prepare protected queue directory: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: - Direct Doctor Share Bundle

    /// Generates a complete doctor export bundle file URL in the temporary directory.
    public func generateDoctorShareFile() -> URL? {
        let observations = MoodDiaryStore.shared.recentEntries.map { DiaryObservationRecord(entry: $0) }
        let jsonString = store.exportResearchKitJSON(diaryObservations: observations)
        guard let data = jsonString.data(using: .utf8) else { return nil }

        let enrollment = store.fetchEnrollment()
        let fileName = "ResearchKit_Study_\(enrollment.participantId)_\(Int(Date().timeIntervalSince1970)).json"
        let tempURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(fileName)

        do {
            try data.write(to: tempURL, options: .atomic)
            return tempURL
        } catch {
            Self.logger.error("Failed to generate doctor share file: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    /// Generates a standardized clinical PDF Doctor Report using PDFKit.
    public func generateDoctorReportPDF(
        gazeRecord: GazeSessionRecord? = nil,
        mchatResult: MCHATScoreResult? = nil,
        careKitCompletionRate: Double? = nil,
        careKitMedicationsLogged: Int? = nil,
        careKitDiaryEntriesLogged: Int? = nil
    ) -> URL? {
        let enrollment = store.fetchEnrollment()
        let guardian = enrollment.consentSignature?.guardianFullName ?? "Parent / Legal Guardian"

        let input = DoctorReportInput(
            participantId: enrollment.participantId,
            childAgeMonths: enrollment.childAgeMonths,
            guardianName: guardian,
            assessmentDate: Date(),
            gazeRecord: gazeRecord,
            mchatResult: mchatResult,
            careKitCompletionRate: careKitCompletionRate,
            careKitMedicationsLogged: careKitMedicationsLogged,
            careKitDiaryEntriesLogged: careKitDiaryEntriesLogged
        )

        let pdfData = DoctorReportGenerator.generatePDFData(input: input)
        guard !pdfData.isEmpty else { return nil }

        let fileName = "Pediatric_Doctor_Report_\(enrollment.participantId)_\(Int(Date().timeIntervalSince1970)).pdf"
        let tempURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(fileName)

        do {
            try pdfData.write(to: tempURL, options: .atomic)
            return tempURL
        } catch {
            Self.logger.error("Failed to write doctor report PDF: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    // MARK: - HTTP Network Request

    private func postPayload(data: Data) async throws -> String {
        // Fail closed even if a future change bypasses configuration validation.
        do {
            try ResearchStudyConfiguration.validateIngestURL(serverURL)
        } catch {
            throw NSError(domain: "ResearchSync", code: -2, userInfo: [NSLocalizedDescriptionKey: "Ingest URL rejected: \(error)"])
        }
        var request = URLRequest(url: serverURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("ResearchKit-Sync-iOS/1.4", forHTTPHeaderField: "User-Agent")
        request.httpBody = data
        request.timeoutInterval = 15.0

        let (respData, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw NSError(domain: "ResearchSync", code: -1, userInfo: [NSLocalizedDescriptionKey: "Invalid server response"])
        }

        guard (200...299).contains(http.statusCode) else {
            let msg = String(data: respData, encoding: .utf8) ?? "HTTP \(http.statusCode)"
            throw NSError(domain: "ResearchSync", code: http.statusCode, userInfo: [NSLocalizedDescriptionKey: msg])
        }

        if let json = try? JSONSerialization.jsonObject(with: respData) as? [String: Any],
           let receipt = json["receipt_id"] as? String {
            return receipt
        }

        return "RCPT-\(UUID().uuidString.prefix(6))"
    }
}
