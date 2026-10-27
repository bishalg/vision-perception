import Foundation
import os

// MARK: - Session History Store (data collection)

/// On-device history of completed gaze screenings, persisted as Codable JSON in a protected
/// file under Application Support (`.completeUntilFirstUserAuthentication`).
/// Feeds the dashboard trend badges, weekly summaries, and the doctor report.
/// De-identified: records carry the study participant token, never raw video.
/// Simulated sessions are never stored (SIM-5), including ones persisted by
/// earlier builds, which are dropped on load.
@MainActor
public final class GazeSessionHistoryStore: ObservableObject {
    public static let shared = GazeSessionHistoryStore()

    /// Cap so the history file stays small on-device.
    public static let maxStoredRecords = 100

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "GazeSessionHistoryStore")

    @Published public private(set) var records: [GazeSessionRecord] = []

    private let storageKey = "org.curv.asd.gaze_session_history"
    private let defaults: UserDefaults
    public let fileURL: URL

    public init(defaults: UserDefaults = .standard, fileURL: URL? = nil) {
        self.defaults = defaults

        let targetURL: URL
        if let explicitURL = fileURL {
            targetURL = explicitURL
        } else if defaults === UserDefaults.standard {
            targetURL = Self.defaultStorageURL()
        } else {
            // For isolated test suites, key by store identifier in the test defaults
            let storeIdKey = "org.curv.asd.test_store_id"
            let storeId = defaults.string(forKey: storeIdKey) ?? UUID().uuidString
            defaults.set(storeId, forKey: storeIdKey)
            targetURL = Self.defaultStorageURL(identifier: storeId)
        }
        self.fileURL = targetURL

        loadAndMigrate()
    }

    private static func defaultStorageURL(identifier: String? = nil) -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        let dir = base.appendingPathComponent("org.curv.asd", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let filename = identifier != nil ? "session_history_\(identifier!).json" : "session_history.json"
        return dir.appendingPathComponent(filename)
    }

    private func loadAndMigrate() {
        // 1. Try reading from protected file storage first
        if FileManager.default.fileExists(atPath: fileURL.path),
           let data = try? Data(contentsOf: fileURL),
           let decoded = try? JSONDecoder().decode([GazeSessionRecord].self, from: data) {
            self.records = decoded.filter { $0.captureSource != .simulated }
            return
        }

        // 2. Migration from legacy UserDefaults storage
        if let data = defaults.data(forKey: storageKey),
           let decoded = try? JSONDecoder().decode([GazeSessionRecord].self, from: data) {
            let valid = decoded.filter { $0.captureSource != .simulated }
            self.records = valid
            persist()
            Self.logger.info("Migrated \(valid.count) sessions from UserDefaults to protected file store")
        }
    }

    /// Records a completed session. Replaces any prior entry with the same
    /// session id (re-exports and retries must not double-count).
    public func record(_ record: GazeSessionRecord) {
        guard record.captureSource != .simulated else {
            Self.logger.info("Ignoring simulated session \(record.sessionId, privacy: .public) for history")
            return
        }
        records.removeAll { $0.sessionId == record.sessionId }
        records.append(record)
        records.sort { $0.endedAt < $1.endedAt }
        if records.count > Self.maxStoredRecords {
            records.removeFirst(records.count - Self.maxStoredRecords)
        }
        persist()
    }

    public func fetchAll() -> [GazeSessionRecord] { records }

    public var lastRecord: GazeSessionRecord? { records.last }

    /// Sessions finished in the trailing 7 days (dashboard weekly badges).
    public func sessionsThisWeek(now: Date = Date()) -> [GazeSessionRecord] {
        let cutoff = now.addingTimeInterval(-7 * 24 * 3600)
        return records.filter { $0.endedAt >= cutoff }
    }

    public func clear() {
        records = []
        defaults.removeObject(forKey: storageKey)
        try? FileManager.default.removeItem(at: fileURL)
    }

    private func persist() {
        guard let data = try? JSONEncoder().encode(records) else { return }
        do {
            let parentDir = fileURL.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
            try data.write(to: fileURL, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
        } catch {
            Self.logger.error("Failed to persist gaze session history: \(error.localizedDescription, privacy: .public)")
        }
    }
}
