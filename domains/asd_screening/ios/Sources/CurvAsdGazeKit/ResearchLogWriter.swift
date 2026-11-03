import AsdCore
import Foundation

/// What a live session needs to write a research frame log (RFL-2). The app builds one only
/// when `NormCollectionGate.isActive` (research consent, opt-in, live TrueDepth); the kit
/// never decides consent. One-shot: the actor consumes it at the next `startSession`.
public struct ResearchLogConfiguration: Sendable, Equatable {
    /// Directory the log is written to (the session vault directory: removed with the vault).
    public let directory: URL
    /// Pseudonymous participant token for the header (never a name, CNS-7).
    public let participantToken: String?
    /// Config bundle the session ran with.
    public let bundleVersion: String?
    /// Scoring profile version of that bundle.
    public let profileVersion: String?
    public let appVersion: String?
    /// Size cap in bytes (RFL-2: 16 MiB); a smaller value is a test hook.
    public let capBytes: Int

    public init(
        directory: URL,
        participantToken: String?,
        bundleVersion: String?,
        profileVersion: String? = nil,
        appVersion: String? = nil,
        capBytes: Int = ResearchLogWriter.defaultCapBytes
    ) {
        self.directory = directory
        self.participantToken = participantToken
        self.bundleVersion = bundleVersion
        self.profileVersion = profileVersion
        self.appVersion = appVersion
        self.capBytes = capBytes
    }
}

/// File handling for `research.jsonl` (RFL-2). It writes only lines the Rust core produced
/// (`gaze_research_log_sample_line`, `gaze_research_log_header_line`) and never formats one.
///
/// Sample lines are appended to a body file while the session runs; the header, which needs
/// the clips and their AOIs, goes first when `finish` assembles `research_<sessionId>.jsonl`.
/// Plain content (the backend seals it, RFL-3) under `completeFileProtection`, excluded from
/// backup. A log over the cap is dropped, and `research_<sessionId>.dropped` records that so
/// the session payload can say `research_log_dropped` (acceptance 16).
///
/// Confined to its owner (the processing actor); not `Sendable`.
public final class ResearchLogWriter {
    public static let defaultCapBytes = 16 * 1024 * 1024
    private static let flushThreshold = 32 * 1024
    private static let bodyFileName = "research_active.body"

    public enum Outcome: Equatable {
        /// The assembled log.
        case written(URL)
        /// Over the cap (or the disk refused a write): no log, a drop marker instead.
        case dropped
        /// No sample line was written.
        case empty
    }

    private let directory: URL
    private let capBytes: Int
    private let fileManager: FileManager
    private var handle: FileHandle?
    private var buffer = Data()
    private var bodyBytes = 0
    private var lineCount = 0
    private var isDropped = false

    private var bodyURL: URL { directory.appendingPathComponent(Self.bodyFileName) }

    /// Creates the (protected, backup-excluded) body file. A body left by a crashed session is
    /// replaced. Returns `nil` when the file cannot be created: no log, the session goes on.
    public init?(directory: URL, capBytes: Int = ResearchLogWriter.defaultCapBytes, fileManager: FileManager = .default) {
        self.directory = directory
        self.capBytes = capBytes
        self.fileManager = fileManager
        do {
            try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
            Self.excludeFromBackup(directory)
            try? fileManager.removeItem(at: bodyURL)
            try Data().write(to: bodyURL, options: [.atomic, .completeFileProtection])
            Self.excludeFromBackup(bodyURL)
            handle = try FileHandle(forWritingTo: bodyURL)
        } catch {
            AsdCoreBridge.logger.error("research log not started: \(error.localizedDescription, privacy: .public)")
            try? fileManager.removeItem(at: bodyURL)
            return nil
        }
    }

    deinit {
        try? handle?.close()
    }

    public static func fileName(sessionId: String) -> String {
        "research_\(sanitised(sessionId)).jsonl"
    }

    public static func droppedMarkerName(sessionId: String) -> String {
        "research_\(sanitised(sessionId)).dropped"
    }

    public static func fileURL(sessionId: String, in directory: URL) -> URL {
        directory.appendingPathComponent(fileName(sessionId: sessionId))
    }

    public static func droppedMarkerURL(sessionId: String, in directory: URL) -> URL {
        directory.appendingPathComponent(droppedMarkerName(sessionId: sessionId))
    }

    /// File-name safe text: ASCII letters, digits, `-` and `_`.
    private static func sanitised(_ text: String) -> String {
        String(text.prefix(64).unicodeScalars.map { scalar in
            let ok = scalar.value < 128 && (CharacterSet.alphanumerics.contains(scalar) || scalar == "-" || scalar == "_")
            return ok ? Character(scalar) : "_"
        })
    }

    /// Whether the log still accepts lines (not dropped, not finished).
    public var isActive: Bool { handle != nil && !isDropped }

    public var sampleLineCount: Int { lineCount }

    /// Appends one line produced by the Rust core (no trailing newline). Over the cap the
    /// whole log is dropped and later calls do nothing.
    public func append(line: String) {
        guard isActive else { return }
        let bytes = line.utf8.count + 1
        guard bodyBytes + bytes <= capBytes else {
            drop()
            return
        }
        buffer.append(contentsOf: line.utf8)
        buffer.append(0x0A)
        bodyBytes += bytes
        lineCount += 1
        if buffer.count >= Self.flushThreshold { flush() }
    }

    /// Closes the body and assembles the log: `headerLine` (from Rust), then the sample lines.
    /// Over the cap with the header counted, the log is dropped instead.
    public func finish(headerLine: String, sessionId: String) -> Outcome {
        flush()
        try? handle?.close()
        handle = nil
        defer { try? fileManager.removeItem(at: bodyURL) }
        let marker = Self.droppedMarkerURL(sessionId: sessionId, in: directory)
        let target = Self.fileURL(sessionId: sessionId, in: directory)
        try? fileManager.removeItem(at: target)
        let headerBytes = headerLine.utf8.count + 1
        if isDropped || bodyBytes + headerBytes > capBytes {
            writeMarker(marker)
            return .dropped
        }
        guard lineCount > 0 else { return .empty }
        do {
            var head = Data(headerLine.utf8)
            head.append(0x0A)
            try head.write(to: target, options: [.atomic, .completeFileProtection])
            Self.excludeFromBackup(target)
            let out = try FileHandle(forWritingTo: target)
            defer { try? out.close() }
            try out.seekToEnd()
            let input = try FileHandle(forReadingFrom: bodyURL)
            defer { try? input.close() }
            while let chunk = try input.read(upToCount: 256 * 1024), !chunk.isEmpty {
                try out.write(contentsOf: chunk)
            }
            return .written(target)
        } catch {
            AsdCoreBridge.logger.error("research log not assembled: \(error.localizedDescription, privacy: .public)")
            try? fileManager.removeItem(at: target)
            writeMarker(marker)
            return .dropped
        }
    }

    /// Ends the log without writing it (session abandoned, capture failed).
    public func discard() {
        try? handle?.close()
        handle = nil
        buffer.removeAll()
        try? fileManager.removeItem(at: bodyURL)
    }

    // MARK: - Private

    private func flush() {
        guard let handle, !buffer.isEmpty else { return }
        do {
            try handle.write(contentsOf: buffer)
            buffer.removeAll(keepingCapacity: true)
        } catch {
            // Locked device or full disk: a partial log would not replay, so drop it.
            AsdCoreBridge.logger.error("research log write failed: \(error.localizedDescription, privacy: .public)")
            drop()
        }
    }

    private func drop() {
        isDropped = true
        buffer.removeAll()
        try? handle?.close()
        handle = nil
        try? fileManager.removeItem(at: bodyURL)
    }

    private func writeMarker(_ url: URL) {
        try? Data().write(to: url, options: [.atomic, .completeFileProtection])
    }

    private static func excludeFromBackup(_ location: URL) {
        var url = location
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? url.setResourceValues(values)
    }
}

/// The calls into the Rust core that produce the lines (RFL-1). Swift only moves values:
/// the sample comes from `gaze_raw_log_line` plus the frame's affect, the header from
/// `gaze_raw_log_header_line` (which resolves the device geometry) plus the session fields;
/// both then pass through `gaze_research_log_*_line`, whose output is what is written.
enum ResearchLogLines {
    struct Clip {
        let clipId: String
        let order: Int
        /// `[ScreenAoi]` JSON from `gaze_clip_aois_json`.
        let aoisJSON: String
        let catalogVersion: String?
    }

    private struct Affect: Encodable {
        let socialSmileIntensity: Double?
        let expressivityIndex: Double?
        let surpriseScore: Double?

        init(_ metrics: FacialAffectMetrics) {
            socialSmileIntensity = Self.finite(metrics.socialSmileIntensity)
            expressivityIndex = Self.finite(metrics.expressivityIndex)
            surpriseScore = Self.finite(metrics.surpriseScore)
        }

        private static func finite(_ v: Double) -> Double? { v.isFinite ? v : nil }
    }

    static func sampleLine(record: RawSampleRecord, affect: FacialAffectMetrics?) throws -> String {
        var line = try gazeRawLogLine(record: record)
        if let affect, line.hasSuffix("}") {
            let json = String(decoding: try JSONEncoder().encode(Affect(affect)), as: UTF8.self)
            line.removeLast()
            line += ",\"affect\":\(json)}"
        }
        return try gazeResearchLogSampleLine(sampleJson: line)
    }

    static func headerLine(
        modelIdentifier: String,
        orientation: GazeOrientation,
        sessionId: String,
        clips: [Clip],
        configuration: ResearchLogConfiguration
    ) throws -> String {
        let base = try gazeRawLogHeaderLine(headerInput: GazeRawLogHeaderInput(
            modelIdentifier: modelIdentifier,
            orientation: orientation,
            targets: [],
            appVersion: configuration.appVersion,
            condition: nil
        ))
        guard var object = try JSONSerialization.jsonObject(with: Data(base.utf8)) as? [String: Any] else {
            throw CocoaError(.coderReadCorrupt)
        }
        object["sessionId"] = sessionId
        if let token = configuration.participantToken { object["participantToken"] = token }
        if let bundle = configuration.bundleVersion { object["bundleVersion"] = bundle }
        if let profile = configuration.profileVersion { object["profileVersion"] = profile }
        object["clips"] = try clips.map { clip -> [String: Any] in
            var entry: [String: Any] = [
                "clipId": clip.clipId,
                "order": clip.order,
                "aois": try JSONSerialization.jsonObject(with: Data(clip.aoisJSON.utf8))
            ]
            if let version = clip.catalogVersion { entry["catalogVersion"] = version }
            return entry
        }
        let data = try JSONSerialization.data(withJSONObject: object)
        return try gazeResearchLogHeaderLine(headerJson: String(decoding: data, as: UTF8.self))
    }
}
