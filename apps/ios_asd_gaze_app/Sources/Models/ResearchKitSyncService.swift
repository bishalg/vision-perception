import Combine
import CryptoKit
import CurvAsdGazeKit
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

/// Where this device stands with the `/v1` API (WP-A.4).
public enum DeviceRegistrationStatus: Sendable, Equatable {
    /// The configuration has no `apiBaseURL`: sessions are not uploaded.
    case unavailable
    /// No device token yet; enter the clinic's enrolment code.
    case unregistered
    case registered(orgId: String)
    /// The token was refused and re-registration failed; a new code is needed.
    case needsEnrolment
}

/// One queued `/v1/sessions` upload (WP-A.4), stored as `session_<id>.json` in the
/// same protected queue as study bundles so backoff, dead letter, quarantine and
/// purge (CNS-4/5/6) apply unchanged. Re-enqueueing a session id replaces its file.
struct QueuedSessionUpload: Codable, Equatable {
    enum Stage: String, Codable {
        /// `POST /v1/sessions` not yet accepted.
        case post
        /// Session accepted; blob PUTs still pending.
        case uploadBlobs
    }

    enum BlobKind: String, Codable {
        case frames
        case report

        var contentType: String {
            switch self {
            case .frames: return "application/octet-stream"
            case .report: return "application/pdf"
            }
        }
    }

    struct Blob: Codable, Equatable {
        let kind: BlobKind
        /// Relative to the blob root (the app container moves between installs) or absolute.
        let path: String
        let isRelative: Bool
        var uploadURL: String?
        var done: Bool
    }

    static let filePrefix = "session_"
    var format = 1
    let sessionId: String
    let payload: SessionIngestPayload
    var stage: Stage
    var blobs: [Blob]

    var hasPendingUploads: Bool {
        blobs.contains { !$0.done && $0.uploadURL != nil }
    }

    static func isSessionFile(_ url: URL) -> Bool {
        url.lastPathComponent.hasPrefix(filePrefix)
    }

    /// Readable, collision-free file name: sanitised id plus a hash of the raw id.
    static func fileName(for sessionId: String) -> String {
        let allowed = Set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_")
        let safe = String(sessionId.prefix(64).map { allowed.contains($0) ? $0 : "_" })
        let digest = SHA256.hash(data: Data(sessionId.utf8)).prefix(4).map { String(format: "%02x", $0) }.joined()
        return "\(filePrefix)\(safe)_\(digest).json"
    }

    mutating func applyUploadURLs(frames: String?, report: String?) {
        for index in blobs.indices {
            switch blobs[index].kind {
            case .frames: blobs[index].uploadURL = frames
            case .report: blobs[index].uploadURL = report
            }
            if blobs[index].uploadURL == nil { blobs[index].done = true }
        }
    }

    /// An upload URL expired or was refused: ask the server for fresh ones.
    mutating func resetForFreshUploadURLs() {
        stage = .post
        for index in blobs.indices where blobs[index].uploadURL != nil && !blobs[index].done {
            blobs[index].uploadURL = nil
        }
    }
}

private enum QueueItemOutcome {
    case delivered(receipt: String)
    case quarantine(code: Int)
    case transient(retryAfter: TimeInterval?)
    /// Not attempted or refused for auth: stays queued, no retry budget spent.
    case held
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
    /// Device-plane registration for `/v1/sessions` (WP-A.4).
    @Published public private(set) var deviceRegistration: DeviceRegistrationStatus = .unavailable

    public let configuration: ResearchStudyConfiguration
    private let store: ResearchKitStudyStore
    private var serverURL: URL { configuration.ingestURL }
    private let fileManager = FileManager.default
    private let queueDirectory: URL
    private let session: URLSession
    private let pathMonitor: NWPathMonitor?
    private let monitorQueue = DispatchQueue(label: "org.curv.researchkit.network", qos: .utility)
    private let now: @Sendable () -> Date
    /// `nil` when the configuration has no `apiBaseURL` (WP-A.4).
    private let sessionClient: SessionSyncClient?
    /// Base for queued blob paths (the app container path changes between installs).
    private let blobRoot: URL

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
        now: @escaping @Sendable () -> Date = { Date() },
        credentialStore: DeviceCredentialStore = KeychainDeviceCredentialStore(),
        clientInfo: SyncClientInfo? = nil,
        blobRoot: URL? = nil,
        transport: SyncTransport? = nil
    ) {
        self.configuration = configuration
        self.store = store
        self.session = session
        self.now = now
        self.blobRoot = (blobRoot ?? URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)).resolvingSymlinksInPath()
        if let apiBaseURL = configuration.apiBaseURL {
            let info = clientInfo ?? .current
            let transport = transport ?? URLSessionSyncTransport(session: session)
            let registration = DeviceRegistrationService(
                apiBaseURL: apiBaseURL,
                transport: transport,
                store: credentialStore,
                clientInfo: info,
                now: now
            )
            self.sessionClient = SessionSyncClient(
                apiBaseURL: apiBaseURL,
                transport: transport,
                registration: registration,
                clientInfo: info
            )
        } else {
            self.sessionClient = nil
        }

        self.queueDirectory = queueDirectory ?? Self.defaultQueueDirectory()
        if startMonitoring {
            self.pathMonitor = NWPathMonitor()
        } else {
            self.pathMonitor = nil
        }
        prepareProtectedDirectory(self.queueDirectory)
        updatePendingCount()
        refreshRegistrationStatus(authFailed: false)

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

    /// Whether this build uploads sessions to `/v1/sessions` (an `apiBaseURL` is configured).
    public var isSessionSyncEnabled: Bool { sessionClient != nil }

    /// Queues a finished session for `POST /v1/sessions` and, when the server
    /// returns pre-signed URLs, PUTs of the encrypted frames file and report.
    ///
    /// Refused (returns `false`) when session sync is off, research consent is not
    /// active, or the record is simulated or of legacy provenance (SIM-4/5). The
    /// queue file is keyed by session id, so re-enqueueing replaces it (dedupe).
    @discardableResult
    public func enqueueSession(
        _ record: GazeSessionRecord,
        framesFile: URL? = nil,
        reportFile: URL? = nil,
        attentionFlag: AttentionFlag? = nil,
        battery: String? = nil
    ) -> Bool {
        guard isSessionSyncEnabled else { return false }
        guard store.fetchEnrollment().isConsented else {
            Self.logger.info("Session \(record.sessionId, privacy: .public) not queued: research consent is not active")
            return false
        }
        let payload: SessionIngestPayload
        do {
            payload = try SessionIngestPayload(record: record, attentionFlag: attentionFlag, battery: battery)
        } catch {
            Self.logger.info("Session \(record.sessionId, privacy: .public) not queued: \(String(describing: error), privacy: .public)")
            return false
        }
        var blobs: [QueuedSessionUpload.Blob] = []
        if let framesFile { blobs.append(queuedBlob(.frames, framesFile)) }
        if let reportFile { blobs.append(queuedBlob(.report, reportFile)) }
        let item = QueuedSessionUpload(sessionId: record.sessionId, payload: payload, stage: .post, blobs: blobs)
        let fileURL = queueDirectory.appendingPathComponent(QueuedSessionUpload.fileName(for: record.sessionId))
        do {
            prepareProtectedDirectory(queueDirectory)
            try SessionPayloadCoding.encoder().encode(item).write(to: fileURL, options: Self.writeOptions)
            // A fresh enqueue starts a fresh retry budget.
            try? fileManager.removeItem(at: metaURL(for: fileURL))
            updatePendingCount()
            return true
        } catch {
            Self.logger.error("Failed to queue session: \(error.localizedDescription, privacy: .public)")
            return false
        }
    }

    /// Registers this device with the clinic's one-time enrolment code
    /// (`POST /v1/devices/register`) and stores the token in the Keychain.
    public func registerDevice(enrolCode: String) async throws {
        guard let sessionClient else { throw DeviceRegistrationError.notRegistered }
        do {
            try await sessionClient.registration.register(.enrolCode(enrolCode))
            refreshRegistrationStatus(authFailed: false)
        } catch {
            refreshRegistrationStatus(authFailed: false)
            throw error
        }
        if pendingQueueCount > 0 && !syncState.isSyncing {
            syncNow(forceBundleCompile: false)
        }
    }

    private func refreshRegistrationStatus(authFailed: Bool) {
        guard let sessionClient else {
            deviceRegistration = .unavailable
            return
        }
        if let credential = sessionClient.registration.currentCredential() {
            deviceRegistration = .registered(orgId: credential.orgId)
        } else if authFailed || deviceRegistration == .needsEnrolment {
            deviceRegistration = .needsEnrolment
        } else {
            deviceRegistration = .unregistered
        }
    }

    private func queuedBlob(_ kind: QueuedSessionUpload.BlobKind, _ url: URL) -> QueuedSessionUpload.Blob {
        let path = url.resolvingSymlinksInPath().path
        let root = blobRoot.path.hasSuffix("/") ? blobRoot.path : blobRoot.path + "/"
        if path.hasPrefix(root) {
            return .init(kind: kind, path: String(path.dropFirst(root.count)), isRelative: true, uploadURL: nil, done: false)
        }
        return .init(kind: kind, path: path, isRelative: false, uploadURL: nil, done: false)
    }

    private func blobFileURL(_ blob: QueuedSessionUpload.Blob) -> URL {
        blob.isRelative ? blobRoot.appendingPathComponent(blob.path) : URL(fileURLWithPath: blob.path)
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
            var authHeld = false
            // One re-registration per flush, not per item (WP-A.4).
            var mayReRegister = true

            for file in jsonFiles {
                guard generation == queueGeneration, !Task.isCancelled else { break }
                let meta = loadMeta(for: file)
                if meta.attemptCount > 0 && meta.nextAttemptAt > startedAt.timeIntervalSince1970 {
                    deferredCount += 1
                    continue
                }
                guard let data = try? Data(contentsOf: file) else { continue }

                let outcome: QueueItemOutcome
                if QueuedSessionUpload.isSessionFile(file) {
                    if authHeld {
                        deferredCount += 1
                        continue
                    }
                    outcome = await processSession(file: file, data: data, generation: generation, mayReRegister: &mayReRegister)
                } else {
                    outcome = await processStudyBundle(data: data)
                }
                // Purged while this upload was in flight: the file is gone,
                // so record nothing about it.
                guard generation == queueGeneration, !Task.isCancelled else { break }

                switch outcome {
                case .delivered(let receipt):
                    lastReceipt = receipt
                    try? fileManager.removeItem(at: file)
                    try? fileManager.removeItem(at: metaURL(for: file))
                    successCount += 1
                case .quarantine(let code):
                    // 4xx status codes indicate permanent payload/client errors.
                    // Quarantine so this file does not block future sync runs.
                    moveAside(file, folderName: Self.quarantineFolder)
                    Self.logger.warning("Quarantined invalid payload \(file.lastPathComponent, privacy: .public) (HTTP \(code))")
                case .held:
                    authHeld = true
                    Self.logger.info("Held \(file.lastPathComponent, privacy: .public): device enrolment needed")
                case .transient(let retryAfter):
                    Self.logger.error("Transmission failed for \(file.lastPathComponent, privacy: .public)")
                    let nextCount = meta.attemptCount + 1
                    if nextCount >= ResearchSyncRetryPolicy.maxAttempts {
                        moveAside(file, folderName: Self.deadLetterFolder)
                        deadLettered += 1
                        Self.logger.warning("Dead-lettered \(file.lastPathComponent, privacy: .public) after \(nextCount) attempts")
                    } else {
                        let delay = max(ResearchSyncRetryPolicy.delay(afterAttempt: nextCount), retryAfter ?? 0)
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
            } else if authHeld {
                syncState = .failed(reason: "Device enrolment needed. Data safely queued.")
                completion?(false)
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

    // MARK: - Queue item processing

    /// Legacy study bundle to `ingestURL` (unchanged behaviour).
    private func processStudyBundle(data: Data) async -> QueueItemOutcome {
        do {
            return .delivered(receipt: try await postPayload(data: data))
        } catch let err as NSError {
            return (400...499).contains(err.code) ? .quarantine(code: err.code) : .transient(retryAfter: nil)
        }
    }

    /// `/v1/sessions` item: POST (once accepted, never again unless an upload URL
    /// expires), then the pre-signed blob PUTs. Progress is written back to the
    /// queue file so a retry resumes where it stopped.
    private func processSession(
        file: URL,
        data: Data,
        generation: Int,
        mayReRegister: inout Bool
    ) async -> QueueItemOutcome {
        guard var item = try? SessionPayloadCoding.decoder().decode(QueuedSessionUpload.self, from: data) else {
            return .quarantine(code: -3)
        }
        guard let client = sessionClient else { return .held }

        do {
            if item.stage == .post {
                let response = try await client.postSession(item.payload, mayReRegister: &mayReRegister)
                refreshRegistrationStatus(authFailed: false)
                guard generation == queueGeneration else { return .held }
                item.applyUploadURLs(frames: response.framesUploadURL, report: response.reportUploadURL)
                guard item.hasPendingUploads else { return .delivered(receipt: item.sessionId) }
                item.stage = .uploadBlobs
                persist(item, to: file, generation: generation)
            }

            for index in item.blobs.indices where !item.blobs[index].done {
                guard let raw = item.blobs[index].uploadURL else {
                    item.blobs[index].done = true
                    continue
                }
                let target = try client.resolveUploadURL(raw)
                let source = blobFileURL(item.blobs[index])
                guard fileManager.fileExists(atPath: source.path) else {
                    // Deleted locally (for example by delete-all): nothing to send.
                    Self.logger.info("Skipping missing \(item.blobs[index].kind.rawValue, privacy: .public) blob for \(item.sessionId, privacy: .public)")
                    item.blobs[index].done = true
                    persist(item, to: file, generation: generation)
                    continue
                }
                // Throws while the device is locked (complete protection): transient.
                let bytes = try Data(contentsOf: source)
                do {
                    try await client.upload(bytes, to: target, contentType: item.blobs[index].kind.contentType)
                } catch SessionSyncError.http(let status, _) where [401, 403, 404, 410].contains(status) {
                    // Expired or refused signature: re-post for fresh URLs next time.
                    guard generation == queueGeneration else { return .held }
                    item.resetForFreshUploadURLs()
                    persist(item, to: file, generation: generation)
                    return .transient(retryAfter: nil)
                }
                guard generation == queueGeneration else { return .held }
                item.blobs[index].done = true
                persist(item, to: file, generation: generation)
            }
            return .delivered(receipt: item.sessionId)
        } catch let error as SessionSyncError {
            switch error {
            case .notRegistered:
                refreshRegistrationStatus(authFailed: false)
                return .held
            case .unauthorized:
                refreshRegistrationStatus(authFailed: true)
                return .held
            case .insecureURL:
                return .quarantine(code: -4)
            case .invalidPayload:
                // asd-sync refused the payload itself; retrying cannot help.
                return .quarantine(code: -5)
            case .http(let status, let retryAfter):
                if status == 408 || status == 429 || status >= 500 {
                    return .transient(retryAfter: retryAfter)
                }
                return .quarantine(code: status)
            }
        } catch {
            return .transient(retryAfter: nil)
        }
    }

    /// Writes upload progress back to the queue file, unless a purge happened.
    private func persist(_ item: QueuedSessionUpload, to file: URL, generation: Int) {
        guard generation == queueGeneration,
              fileManager.fileExists(atPath: file.path),
              let data = try? SessionPayloadCoding.encoder().encode(item) else { return }
        try? data.write(to: file, options: Self.writeOptions)
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
