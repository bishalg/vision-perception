import AsdCore
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
            f.locale = Locale.current
            return "Synced at \(f.string(from: ts)) (Receipt #\(receipt.prefix(8)))"
        case .failed(let err): return "Sync error: \(err)"
        }
    }
}

/// Transient-failure schedule for the research upload queue (SYNC-3). The numbers live
/// in Rust (`asd-sync::retry`, through `AsdCore`); this type only adapts them to Swift
/// types. The failure after the last delay moves the payload to the dead-letter folder.
public enum ResearchSyncRetryPolicy {
    /// Failures tolerated before a payload is dead-lettered (`syncMaxAttempts()`).
    public static var maxAttempts: Int { Int(syncMaxAttempts()) }

    /// The schedule after the 1st, 2nd, ... failure (`syncBackoffSchedule`).
    public static var delays: [TimeInterval] {
        (1..<maxAttempts).map { TimeInterval(syncBackoffSchedule(attempt: UInt32($0))) }
    }

    /// Wait after failed attempt `attempt` (1-based): the schedule, or the server's
    /// `Retry-After` when longer (`syncNextRetryDelay`).
    public static func delay(afterAttempt attempt: Int, retryAfter: TimeInterval? = nil) -> TimeInterval {
        syncNextRetryDelay(attempt: UInt32(clamping: max(attempt, 0)), retryAfterSecs: retryAfter)
    }

    /// Automatic dead-letter recycles (on reconnect and at launch) before an item stays
    /// dead-lettered until someone taps retry (SYNC-4).
    public static let maxDeadLetterCycles = 3
}

/// Retry state beside each queue file (`<name>.syncmeta`), kept with the file when it is
/// dead-lettered so the recycle count survives (SYNC-4).
private struct ResearchSyncFileMeta: Codable {
    var attemptCount: Int
    var nextAttemptAt: TimeInterval
    /// Times the item came back from the dead-letter folder automatically; absent in
    /// metadata written before SYNC-4.
    var deadLetterCycles: Int?
}

/// What the clinic dashboard shows about the upload queue (SYNC-8). Counts only: no
/// participant tokens or session ids.
public struct SyncStatusSummary: Equatable, Sendable {
    /// Queued and not yet accepted by the server.
    public let waiting: Int
    /// Accepted by the server, encrypted files still uploading.
    public let uploading: Int
    /// Dead-lettered or quarantined: someone has to look (one-tap retry for dead letters).
    public let needsAttention: Int
    public let lastSyncedAt: Date?

    public init(waiting: Int, uploading: Int, needsAttention: Int, lastSyncedAt: Date?) {
        self.waiting = waiting
        self.uploading = uploading
        self.needsAttention = needsAttention
        self.lastSyncedAt = lastSyncedAt
    }
}

/// Why a flush was asked for (SYNC-2); logged, and `.manual` bypasses nothing.
public enum SyncFlushTrigger: String, Sendable {
    case enqueue, launch, foreground, reconnect, timer, registration, backgroundUpload
}

/// The decoded stage of a queue file, without the payload (for the status counts).
private struct QueuedSessionStageProbe: Decodable {
    let stage: QueuedSessionUpload.Stage
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
        /// RFL-2: the plain research frame log (the backend seals it, RFL-3). Proxy blob mode
        /// only; the file is deleted as soon as it is uploaded or can never be.
        case researchFrames = "research_frames"

        var contentType: String {
            switch self {
            case .frames: return "application/octet-stream"
            case .report: return "application/pdf"
            case .researchFrames: return "application/x-ndjson"
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
        let digest = SHA256.hash(data: Data(sessionId.utf8)).prefix(4).map { String(format: "%02x", locale: FormatLocale.posix, $0) }.joined()
        return "\(filePrefix)\(safe)_\(digest).json"
    }

    mutating func applyUploadURLs(frames: String?, report: String?, researchFrames: String? = nil) {
        for index in blobs.indices {
            switch blobs[index].kind {
            case .frames: blobs[index].uploadURL = frames
            case .report: blobs[index].uploadURL = report
            case .researchFrames: blobs[index].uploadURL = researchFrames
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
    /// The device has no network path: stop the flush, no retry budget spent.
    case offline
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
    /// Blob uploads go through the process's one background session (SYNC-5); uploads
    /// that finish after a relaunch mark their blob done here.
    public static let shared: ResearchKitSyncService? = ResearchStudyConfiguration.current.map { configuration in
        let uploader = BackgroundBlobUploader.shared
        let service = ResearchKitSyncService(
            configuration: configuration,
            transport: URLSessionSyncTransport(session: .shared, backgroundUploader: uploader)
        )
        // `shared` lives for the whole process, so the handler may hold it.
        uploader.setOrphanHandler { path, status in
            Task { @MainActor in service.backgroundUploadFinished(filePath: path, status: status) }
        }
        return service
    }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ResearchKitSyncService")

    @Published public private(set) var syncState: ResearchSyncState = .idle
    @Published public private(set) var pendingQueueCount: Int = 0
    @Published public private(set) var deadLetterCount: Int = 0
    @Published public private(set) var lastSyncedAt: Date?
    /// Quarantined payloads (refused by the server; kept for inspection, manual only).
    @Published public private(set) var quarantineCount: Int = 0
    /// Queued sessions whose POST was accepted and whose blobs are still uploading.
    @Published public private(set) var uploadingCount: Int = 0
    /// `nil` until the first network path report (trap 13): starting `true` skipped the
    /// launch flush, starting `false` would block it until the monitor reports. The first
    /// `.satisfied` report counts as a reconnect and flushes.
    @Published public private(set) var isOnline: Bool?
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
    /// `GET /v1/config/bundle` (WP-A.5); `nil` without an `apiBaseURL`.
    private let configClient: ConfigBundleClient?
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
    /// A trigger arrived while a flush was running: run once more when it ends (SYNC-2).
    private var flushRequestedDuringSync = false
    /// Wakes when the earliest `nextAttemptAt` passes while online (SYNC-2).
    private var retryTimer: Task<Void, Never>?
    private let retrySleep: @Sendable (TimeInterval) async throws -> Void
    /// Queue file names in stage `.uploadBlobs` (SYNC-8 "uploading").
    private var uploadingFileNames: Set<String> = []

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
        transport: SyncTransport? = nil,
        retrySleep: @escaping @Sendable (TimeInterval) async throws -> Void = { try await Task.sleep(for: .seconds($0)) }
    ) {
        self.configuration = configuration
        self.store = store
        self.session = session
        self.now = now
        self.retrySleep = retrySleep
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
            self.configClient = ConfigBundleClient(
                apiBaseURL: apiBaseURL,
                transport: transport,
                registration: registration,
                clientInfo: info,
                now: now
            )
        } else {
            self.sessionClient = nil
            self.configClient = nil
        }

        self.queueDirectory = queueDirectory ?? Self.defaultQueueDirectory()
        if startMonitoring {
            self.pathMonitor = NWPathMonitor()
        } else {
            self.pathMonitor = nil
        }
        prepareProtectedDirectory(self.queueDirectory)
        uploadingFileNames = Set(probeUploadingFiles())
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
        retryTimer?.cancel()
    }

    /// Network path report (`NWPathMonitor`, or a test double). The first `.satisfied`
    /// report and every offline -> online change count as a reconnect: dead letters are
    /// recycled (SYNC-4) and the queue flushes (SYNC-2).
    public func handlePathUpdate(status: NWPath.Status) {
        let wasOnline = isOnline
        let online = status == .satisfied
        isOnline = online
        guard wasOnline != online else { return }
        Self.logger.info("Network path is \(online ? "online" : "offline", privacy: .public)")
        guard online else {
            retryTimer?.cancel()
            retryTimer = nil
            return
        }
        recycleDeadLetters()
        requestFlush(.reconnect)
    }

    // MARK: - Flush triggers (SYNC-2)

    /// Launch (after `installAtLaunch`): recycles dead letters (SYNC-4) and flushes when
    /// the network is known to be up; while the state is unknown, the first path report
    /// flushes instead.
    public func flushAtLaunch() {
        recycleDeadLetters()
        requestFlush(.launch)
    }

    /// Schedules a flush of the queue when it can do anything: online, research consent
    /// active, something queued. A trigger during a running flush runs one more flush
    /// after it. Manual `syncNow` is unaffected by these conditions.
    public func requestFlush(_ trigger: SyncFlushTrigger) {
        guard isOnline == true else { return }
        guard store.fetchEnrollment().isConsented else { return }
        updatePendingCount()
        guard pendingQueueCount > 0 else { return }
        if syncState.isSyncing {
            flushRequestedDuringSync = true
            return
        }
        Self.logger.info("Flushing \(self.pendingQueueCount) queued upload(s) (\(trigger.rawValue, privacy: .public))")
        syncNow(forceBundleCompile: false)
    }

    /// The retry timer's body: flushes when the earliest deferred item is due, otherwise
    /// re-arms. Internal so the field scenarios can drive it on a virtual clock.
    func fireRetryTimerIfDue() {
        guard let due = earliestRetryAt() else { return }
        if due <= now().timeIntervalSince1970 {
            requestFlush(.timer)
        } else {
            scheduleRetryTimer()
        }
    }

    /// When the timer will flush next (`nil`: nothing deferred or not online).
    var nextRetryAt: Date? {
        guard isOnline == true, let due = earliestRetryAt() else { return nil }
        return Date(timeIntervalSince1970: due)
    }

    /// Waits until no flush is running (chained flushes included). For tests.
    func waitUntilIdle() async {
        while let task = syncTask {
            await task.value
        }
    }

    private func scheduleRetryTimer() {
        retryTimer?.cancel()
        retryTimer = nil
        guard isOnline == true, let due = earliestRetryAt() else { return }
        let delay = max(due - now().timeIntervalSince1970, 0)
        let sleep = retrySleep
        retryTimer = Task { [weak self] in
            do {
                try await sleep(delay)
            } catch {
                return
            }
            self?.fireRetryTimerIfDue()
        }
    }

    /// Earliest `nextAttemptAt` of a queued item that has failed before.
    private func earliestRetryAt() -> TimeInterval? {
        let files = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        return files
            .filter { $0.pathExtension == "json" }
            .map { loadMeta(for: $0) }
            .filter { $0.attemptCount > 0 }
            .map(\.nextAttemptAt)
            .min()
    }

    /// The clinic dashboard's line (SYNC-8).
    public var statusSummary: SyncStatusSummary {
        let uploading = min(uploadingCount, pendingQueueCount)
        return SyncStatusSummary(
            waiting: pendingQueueCount - uploading,
            uploading: uploading,
            needsAttention: deadLetterCount + quarantineCount,
            lastSyncedAt: lastSyncedAt
        )
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

    /// Fetches the device's config bundle (WP-A.5). An accepted bundle is persisted and scores
    /// from the next launch; a refused one changes nothing.
    @discardableResult
    public func refreshConfigBundle() async -> ConfigBundleClient.Outcome {
        guard let configClient else { return .disabled }
        return await configClient.refresh()
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
        guard isSessionSyncEnabled, store.fetchEnrollment().isConsented else {
            // Not queued: the plain research log has no use, so it is not left behind.
            _ = researchLogFiles(for: record, framesFile: framesFile, keep: false)
            if isSessionSyncEnabled {
                Self.logger.info("Session \(record.sessionId, privacy: .public) not queued: research consent is not active")
            }
            return false
        }
        // RFL-2: the research log sits next to the vault file; a drop marker says the log went
        // over its cap (acceptance 16). Only a record that carries norm-collection consent
        // may send either, so a stray file without consent is deleted, never uploaded.
        let research = researchLogFiles(for: record, framesFile: framesFile)
        var record = record
        if research.dropped { record.normCollection?.researchLogDropped = true }
        let payload: SessionIngestPayload
        do {
            payload = try SessionIngestPayload(record: record, attentionFlag: attentionFlag, battery: battery)
        } catch {
            Self.logger.info("Session \(record.sessionId, privacy: .public) not queued: \(String(describing: error), privacy: .public)")
            if let log = research.log { try? fileManager.removeItem(at: log) }
            return false
        }
        var blobs: [QueuedSessionUpload.Blob] = []
        if let framesFile { blobs.append(queuedBlob(.frames, framesFile)) }
        if let reportFile { blobs.append(queuedBlob(.report, reportFile)) }
        if let log = research.log { blobs.append(queuedBlob(.researchFrames, log)) }
        let item = QueuedSessionUpload(sessionId: record.sessionId, payload: payload, stage: .post, blobs: blobs)
        let fileURL = queueDirectory.appendingPathComponent(QueuedSessionUpload.fileName(for: record.sessionId))
        do {
            prepareProtectedDirectory(queueDirectory)
            try SessionPayloadCoding.encoder().encode(item).write(to: fileURL, options: Self.writeOptions)
            // A fresh enqueue starts a fresh retry budget.
            try? fileManager.removeItem(at: metaURL(for: fileURL))
            uploadingFileNames.remove(fileURL.lastPathComponent)
            updatePendingCount()
            requestFlush(.enqueue)
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
            Self.logger.info("Flushing after registration")
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

    /// The research log and drop marker `GazeProcessingActor` left beside the vault file for
    /// this session. Without norm-collection consent on the record both are removed.
    private func researchLogFiles(for record: GazeSessionRecord, framesFile: URL?, keep: Bool = true) -> (log: URL?, dropped: Bool) {
        guard let directory = framesFile?.deletingLastPathComponent() else { return (nil, false) }
        let log = ResearchLogWriter.fileURL(sessionId: record.sessionId, in: directory)
        let marker = ResearchLogWriter.droppedMarkerURL(sessionId: record.sessionId, in: directory)
        let hasLog = fileManager.fileExists(atPath: log.path)
        let dropped = fileManager.fileExists(atPath: marker.path)
        try? fileManager.removeItem(at: marker)
        guard keep, record.normCollection?.consented == true else {
            if hasLog { try? fileManager.removeItem(at: log) }
            return (nil, false)
        }
        return (hasLog ? log : nil, dropped)
    }

    private func queuedBlob(_ kind: QueuedSessionUpload.BlobKind, _ url: URL) -> QueuedSessionUpload.Blob {
        let path = url.resolvingSymlinksInPath().path
        let root = blobRoot.path.hasSuffix("/") ? blobRoot.path : blobRoot.path + "/"
        if path.hasPrefix(root) {
            return .init(kind: kind, path: String(path.dropFirst(root.count)), isRelative: true, uploadURL: nil, done: false)
        }
        return .init(kind: kind, path: path, isRelative: false, uploadURL: nil, done: false)
    }

    /// RFL-2: the research log is deleted as soon as its blob is done (uploaded, or never
    /// offered by the server); it is never kept once the queue has no use for it.
    private func removeResearchFilesOfFinishedBlobs(_ item: QueuedSessionUpload) {
        for blob in item.blobs where blob.kind == .researchFrames && blob.done {
            try? fileManager.removeItem(at: blobFileURL(blob))
        }
    }

    /// A quarantined session will never upload its research log: delete it.
    private func removeResearchFiles(ofQueueFile file: URL) {
        guard let data = try? Data(contentsOf: file),
              let item = try? SessionPayloadCoding.decoder().decode(QueuedSessionUpload.self, from: data) else { return }
        for blob in item.blobs where blob.kind == .researchFrames {
            try? fileManager.removeItem(at: blobFileURL(blob))
        }
    }

    private func blobFileURL(_ blob: QueuedSessionUpload.Blob) -> URL {
        blob.isRelative ? blobRoot.appendingPathComponent(blob.path) : URL(fileURLWithPath: blob.path)
    }

    /// Update the count of pending transmissions in the sandbox queue.
    public func updatePendingCount() {
        let contents = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        let queued = contents.filter { $0.pathExtension == "json" }
        pendingQueueCount = queued.count
        uploadingFileNames.formIntersection(queued.map(\.lastPathComponent))
        uploadingCount = uploadingFileNames.count
        let stuck = (try? fileManager.contentsOfDirectory(at: deadLetterDirectory, includingPropertiesForKeys: nil)) ?? []
        deadLetterCount = stuck.filter { $0.pathExtension == "json" }.count
        let quarantined = (try? fileManager.contentsOfDirectory(
            at: queueDirectory.appendingPathComponent(Self.quarantineFolder, isDirectory: true),
            includingPropertiesForKeys: nil
        )) ?? []
        quarantineCount = quarantined.filter { $0.pathExtension == "json" }.count
    }

    /// Queue files in stage `.uploadBlobs`, read once at launch.
    private func probeUploadingFiles() -> [String] {
        let contents = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        return contents
            .filter { $0.pathExtension == "json" && QueuedSessionUpload.isSessionFile($0) }
            .filter { file in
                guard let data = try? Data(contentsOf: file),
                      let probe = try? JSONDecoder().decode(QueuedSessionStageProbe.self, from: data) else { return false }
                return probe.stage == .uploadBlobs
            }
            .map(\.lastPathComponent)
    }

    /// Deletes every queued, dead-lettered and quarantined payload and cancels
    /// any in-flight upload. Called when research consent is revoked (CNS-5)
    /// and by "Delete all observation data" (CNS-6).
    public func purgeAllQueued() {
        queueGeneration += 1
        syncTask?.cancel()
        syncTask = nil
        retryTimer?.cancel()
        retryTimer = nil
        flushRequestedDuringSync = false
        uploadingFileNames.removeAll()

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

    /// Move stuck payloads back onto the live queue so the next transmit can try them
    /// again (the one-tap retry). The recycle count is kept.
    public func retryDeadLetterItems() {
        let stuck = (try? fileManager.contentsOfDirectory(at: deadLetterDirectory, includingPropertiesForKeys: nil)) ?? []
        for file in stuck where file.pathExtension == "json" {
            restoreFromDeadLetter(file, cycles: loadMeta(for: file).deadLetterCycles ?? 0)
        }
        updatePendingCount()
    }

    /// SYNC-4: on reconnect and at launch, every dead-lettered item that has been
    /// recycled fewer than `maxDeadLetterCycles` times goes back to the queue once with a
    /// fresh attempt budget; the others stay until someone taps retry.
    public func recycleDeadLetters() {
        let stuck = (try? fileManager.contentsOfDirectory(at: deadLetterDirectory, includingPropertiesForKeys: nil)) ?? []
        var recycled = 0
        for file in stuck where file.pathExtension == "json" {
            let cycles = loadMeta(for: file).deadLetterCycles ?? 0
            guard cycles < ResearchSyncRetryPolicy.maxDeadLetterCycles else { continue }
            restoreFromDeadLetter(file, cycles: cycles + 1)
            recycled += 1
        }
        if recycled > 0 {
            Self.logger.info("Recycled \(recycled) dead-lettered upload(s)")
        }
        updatePendingCount()
    }

    private func restoreFromDeadLetter(_ file: URL, cycles: Int) {
        let dest = queueDirectory.appendingPathComponent(file.lastPathComponent)
        try? fileManager.removeItem(at: dest)
        try? fileManager.removeItem(at: metaURL(for: dest))
        do {
            try fileManager.moveItem(at: file, to: dest)
        } catch {
            Self.logger.error("Failed to restore \(file.lastPathComponent, privacy: .public): \(error.localizedDescription, privacy: .public)")
            return
        }
        try? fileManager.removeItem(at: metaURL(for: file))
        saveMeta(ResearchSyncFileMeta(attemptCount: 0, nextAttemptAt: 0, deadLetterCycles: cycles), for: dest)
    }

    /// A background blob upload finished in a process that was not waiting for it (the
    /// app was relaunched, SYNC-5): mark that blob done so the next flush neither re-sends
    /// it nor the POST.
    func backgroundUploadFinished(filePath: String, status: Int) {
        guard (200...299).contains(status) else { return }
        let files = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
        for file in files where file.pathExtension == "json" && QueuedSessionUpload.isSessionFile(file) {
            guard let data = try? Data(contentsOf: file),
                  var item = try? SessionPayloadCoding.decoder().decode(QueuedSessionUpload.self, from: data),
                  let index = item.blobs.firstIndex(where: {
                      !$0.done && blobFileURL($0).standardizedFileURL.path == filePath
                  }) else { continue }
            item.blobs[index].done = true
            persist(item, to: file, generation: queueGeneration)
            removeResearchFilesOfFinishedBlobs(item)
            Self.logger.info("Marked a background-uploaded blob done for \(item.sessionId, privacy: .public)")
            break
        }
        requestFlush(.backgroundUpload)
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
        flushRequestedDuringSync = false
        retryTimer?.cancel()
        retryTimer = nil
        syncTask = Task {
            defer {
                if generation == queueGeneration {
                    syncTask = nil
                    flushEnded()
                }
            }
            let files = (try? fileManager.contentsOfDirectory(at: queueDirectory, includingPropertiesForKeys: nil)) ?? []
            // Name order: deterministic, and the oldest study bundles are not starved.
            let jsonFiles = files.filter { $0.pathExtension == "json" }.sorted { $0.lastPathComponent < $1.lastPathComponent }
            let startedAt = now()
            var successCount = 0
            var transientFailures = 0
            var deferredCount = 0
            var deadLettered = 0
            var lastReceipt = "RCPT-\(UUID().uuidString.prefix(6))"
            var authHeld = false
            var wentOffline = false
            // One re-registration per flush, not per item (WP-A.4).
            var mayReRegister = true

            for file in jsonFiles {
                guard generation == queueGeneration, !Task.isCancelled else { break }
                // Offline mid-flush: stop instead of spending every item's retry budget.
                guard isOnline != false else {
                    wentOffline = true
                    break
                }
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
                    uploadingFileNames.remove(file.lastPathComponent)
                    successCount += 1
                case .quarantine(let code):
                    // 4xx status codes indicate permanent payload/client errors.
                    // Quarantine so this file does not block future sync runs.
                    removeResearchFiles(ofQueueFile: file)
                    moveAside(file, folderName: Self.quarantineFolder)
                    Self.logger.warning("Quarantined invalid payload \(file.lastPathComponent, privacy: .public) (HTTP \(code))")
                case .held:
                    authHeld = true
                    Self.logger.info("Held \(file.lastPathComponent, privacy: .public): device enrolment needed")
                case .offline:
                    wentOffline = true
                    Self.logger.info("No network path; flush stopped at \(file.lastPathComponent, privacy: .public)")
                case .transient(let retryAfter):
                    Self.logger.error("Transmission failed for \(file.lastPathComponent, privacy: .public)")
                    let nextCount = meta.attemptCount + 1
                    if nextCount >= ResearchSyncRetryPolicy.maxAttempts {
                        moveAside(file, folderName: Self.deadLetterFolder, keepingCycles: meta.deadLetterCycles)
                        deadLettered += 1
                        Self.logger.warning("Dead-lettered \(file.lastPathComponent, privacy: .public) after \(nextCount) attempts")
                    } else {
                        // SYNC-3: the schedule and the Retry-After rule come from asd-sync.
                        let delay = ResearchSyncRetryPolicy.delay(afterAttempt: nextCount, retryAfter: retryAfter)
                        let updated = ResearchSyncFileMeta(
                            attemptCount: nextCount,
                            nextAttemptAt: now().timeIntervalSince1970 + delay,
                            deadLetterCycles: meta.deadLetterCycles
                        )
                        saveMeta(updated, for: file)
                        transientFailures += 1
                        Self.logger.info("Scheduled retry \(nextCount) for \(file.lastPathComponent, privacy: .public) in \(delay, privacy: .public)s")
                    }
                }
                if wentOffline { break }
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
            } else if wentOffline {
                syncState = .failed(reason: "Offline. Data safely queued.")
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

    /// End of a flush: run the one that was asked for meanwhile, else arm the timer.
    private func flushEnded() {
        if flushRequestedDuringSync {
            flushRequestedDuringSync = false
            requestFlush(.timer)
            if syncState.isSyncing { return }
        }
        scheduleRetryTimer()
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
            return ResearchSyncFileMeta(attemptCount: 0, nextAttemptAt: 0, deadLetterCycles: nil)
        }
        return meta
    }

    private func saveMeta(_ meta: ResearchSyncFileMeta, for file: URL) {
        guard let data = try? JSONEncoder().encode(meta) else { return }
        try? data.write(to: metaURL(for: file), options: Self.writeOptions)
    }

    /// Moves a queue file into `folderName`. A dead-lettered file keeps its recycle count
    /// in metadata beside it (SYNC-4); a quarantined one keeps none.
    private func moveAside(_ file: URL, folderName: String, keepingCycles cycles: Int?? = .none) {
        let dir = queueDirectory.appendingPathComponent(folderName, isDirectory: true)
        prepareProtectedDirectory(dir)
        let dest = dir.appendingPathComponent(file.lastPathComponent)
        try? fileManager.removeItem(at: dest)
        try? fileManager.removeItem(at: metaURL(for: dest))
        try? fileManager.moveItem(at: file, to: dest)
        try? fileManager.removeItem(at: metaURL(for: file))
        uploadingFileNames.remove(file.lastPathComponent)
        if case .some(let kept) = cycles {
            saveMeta(ResearchSyncFileMeta(attemptCount: 0, nextAttemptAt: 0, deadLetterCycles: kept ?? 0), for: dest)
        }
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
                item.applyUploadURLs(
                    frames: response.framesUploadURL,
                    report: response.reportUploadURL,
                    researchFrames: response.researchFramesUploadURL
                )
                // A research log the server will not take (presigned mode, no consent on file)
                // is not kept: it is plain content.
                removeResearchFilesOfFinishedBlobs(item)
                guard item.hasPendingUploads else { return .delivered(receipt: item.sessionId) }
                item.stage = .uploadBlobs
                persist(item, to: file, generation: generation)
                uploadingFileNames.insert(file.lastPathComponent)
                uploadingCount = uploadingFileNames.count
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
                // Streamed from the encrypted file (background session in the app, SYNC-5);
                // opening it throws while the device is locked (complete protection): transient.
                do {
                    try await client.uploadFile(source, to: target, contentType: item.blobs[index].kind.contentType)
                } catch SessionSyncError.http(let status, _) where [401, 403, 404, 410].contains(status) {
                    // Expired or refused signature: re-post for fresh URLs next time.
                    guard generation == queueGeneration else { return .held }
                    item.resetForFreshUploadURLs()
                    persist(item, to: file, generation: generation)
                    uploadingFileNames.remove(file.lastPathComponent)
                    return .transient(retryAfter: nil)
                }
                guard generation == queueGeneration else { return .held }
                item.blobs[index].done = true
                persist(item, to: file, generation: generation)
                removeResearchFilesOfFinishedBlobs(item)
            }
            removeResearchFilesOfFinishedBlobs(item)
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
        } catch let error as URLError where Self.offlineCodes.contains(error.code) {
            return .offline
        } catch {
            return .transient(retryAfter: nil)
        }
    }

    /// Transport errors that mean "no network path", not "this request failed".
    private static let offlineCodes: Set<URLError.Code> = [.notConnectedToInternet, .dataNotAllowed, .internationalRoamingOff]

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
