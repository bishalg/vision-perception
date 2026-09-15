import Foundation
import CurvAsdGazeKit

/// Writes ARFaceAnchor blend-shape coefficients to a local CSV for later Core ML
/// training. Numeric coefficients only — never camera frames or video.
public final class BlendShapeCSVLogger: @unchecked Sendable {
    public static let shared = BlendShapeCSVLogger()

    public static let maxFileBytes: Int = 50 * 1024 * 1024
    public static let minInterval: TimeInterval = 0.2

    private let fileManager: FileManager
    private var handle: FileHandle?
    private var currentURL: URL?
    private var lastWrite: Date = .distantPast
    private var observer: NSObjectProtocol?
    private let lock = NSLock()
    private let iso = ISO8601DateFormatter()

    public init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    }

    public var directoryURL: URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.temporaryDirectory
        let dir = base.appendingPathComponent("BlendShapeML", isDirectory: true)
        try? fileManager.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    public func startListening() {
        guard observer == nil else { return }
        observer = NotificationCenter.default.addObserver(
            forName: GazeSessionController.didUpdateFaceBlendShapesNotification,
            object: nil,
            queue: nil
        ) { [weak self] note in
            self?.handle(note)
        }
    }

    public func stopListening() {
        if let observer {
            NotificationCenter.default.removeObserver(observer)
            self.observer = nil
        }
        lock.lock()
        try? handle?.synchronize()
        try? handle?.close()
        handle = nil
        lock.unlock()
    }

    /// Builds a CSV header + one data row (used by tests and the live logger).
    public func csvLine(
        timestamp: Date,
        sessionId: String,
        isRecording: Bool,
        shapes: [String: Double]
    ) -> String {
        let values = FaceBlendShapeCSV.columns.map { key in
            csvNumber(shapes[key] ?? 0)
        }
        let fields = [
            iso.string(from: timestamp),
            csvEscape(sessionId),
            isRecording ? "1" : "0"
        ] + values
        return fields.joined(separator: ",")
    }

    public var headerLine: String {
        (["timestamp_iso", "session_id", "is_recording"] + FaceBlendShapeCSV.columns)
            .joined(separator: ",")
    }

    private func handle(_ note: Notification) {
        let recording = (note.userInfo?[GazeSessionController.isRecordingUserInfoKey] as? Bool) ?? false
        guard recording else { return }
        let now = Date()
        lock.lock()
        defer { lock.unlock() }
        guard now.timeIntervalSince(lastWrite) >= Self.minInterval else { return }
        lastWrite = now
        let shapes = (note.userInfo?[GazeSessionController.blendShapesUserInfoKey] as? [String: Double]) ?? [:]
        let sessionId = (note.userInfo?[GazeSessionController.sessionIdUserInfoKey] as? String) ?? "unknown"
        append(line: csvLine(timestamp: now, sessionId: sessionId, isRecording: recording, shapes: shapes))
    }

    private func append(line: String) {
        ensureFileUnlocked()
        guard let handle else { return }
        if let data = (line + "\n").data(using: .utf8) {
            try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        }
        rotateIfNeededUnlocked()
    }

    private func ensureFileUnlocked() {
        if handle != nil { return }
        let url = directoryURL.appendingPathComponent("blendshapes-\(fileStamp()).csv")
        if !fileManager.fileExists(atPath: url.path) {
            let header = headerLine + "\n"
            try? header.write(to: url, atomically: true, encoding: .utf8)
        }
        handle = try? FileHandle(forWritingTo: url)
        currentURL = url
    }

    private func rotateIfNeededUnlocked() {
        guard let url = currentURL,
              let attrs = try? fileManager.attributesOfItem(atPath: url.path),
              let size = attrs[.size] as? NSNumber,
              size.intValue > Self.maxFileBytes else { return }
        try? handle?.synchronize()
        try? handle?.close()
        handle = nil
        currentURL = nil
    }

    private func fileStamp() -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyyMMdd-HHmmss"
        return f.string(from: Date())
    }

    private func csvNumber(_ value: Double) -> String {
        String(format: "%.5f", value)
    }

    private func csvEscape(_ value: String) -> String {
        if value.contains(",") || value.contains("\"") {
            return "\"\(value.replacingOccurrences(of: "\"", with: "\"\""))\""
        }
        return value
    }
}

/// Stable column order matching `ARFaceAnchor.BlendShapeLocation` raw values.
public enum FaceBlendShapeCSV: Sendable {
    public static let columns: [String] = [
        "browDownLeft", "browDownRight", "browInnerUp", "browOuterUpLeft", "browOuterUpRight",
        "cheekPuff", "cheekSquintLeft", "cheekSquintRight",
        "eyeBlinkLeft", "eyeBlinkRight",
        "eyeLookDownLeft", "eyeLookDownRight", "eyeLookInLeft", "eyeLookInRight",
        "eyeLookOutLeft", "eyeLookOutRight", "eyeLookUpLeft", "eyeLookUpRight",
        "eyeSquintLeft", "eyeSquintRight", "eyeWideLeft", "eyeWideRight",
        "jawForward", "jawLeft", "jawOpen", "jawRight",
        "mouthClose", "mouthDimpleLeft", "mouthDimpleRight", "mouthFrownLeft", "mouthFrownRight",
        "mouthFunnel", "mouthLeft", "mouthLowerDownLeft", "mouthLowerDownRight",
        "mouthPressLeft", "mouthPressRight", "mouthPucker", "mouthRight",
        "mouthRollLower", "mouthRollUpper", "mouthShrugLower", "mouthShrugUpper",
        "mouthSmileLeft", "mouthSmileRight", "mouthStretchLeft", "mouthStretchRight",
        "mouthUpperUpLeft", "mouthUpperUpRight",
        "noseSneerLeft", "noseSneerRight",
        "tongueOut"
    ]
}
