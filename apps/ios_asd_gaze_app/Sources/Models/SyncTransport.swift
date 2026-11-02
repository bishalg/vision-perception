import Foundation
import os
import UIKit

/// The single seam between the sync client and the network (WP-A.4).
///
/// Production uses `URLSessionSyncTransport`; tests use a scripted stub or a
/// `URLSession` whose protocol classes answer every request, so no test reaches
/// a real host.
public protocol SyncTransport: Sendable {
    /// Sends `request` and returns the body with the HTTP response. Throws only
    /// for transport failures (offline, timeout, TLS); every HTTP status is returned.
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)

    /// `PUT`s the bytes of `file` to `url` with `headers`, streamed from disk (never the
    /// whole file in memory) so a background session can resume it after the app is
    /// killed (SYNC-5). Same error contract as `send`.
    func uploadFile(_ file: URL, to url: URL, headers: [String: String]) async throws -> (Data, HTTPURLResponse)
}

public struct URLSessionSyncTransport: SyncTransport {
    public let session: URLSession
    /// Blob uploads go through the process's one background session when set (SYNC-5);
    /// `nil` streams them through `session` in the foreground (tests, previews).
    public let backgroundUploader: BackgroundBlobUploader?

    public init(session: URLSession = .shared, backgroundUploader: BackgroundBlobUploader? = nil) {
        self.session = session
        self.backgroundUploader = backgroundUploader
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw URLError(.badServerResponse)
        }
        return (data, http)
    }

    public func uploadFile(_ file: URL, to url: URL, headers: [String: String]) async throws -> (Data, HTTPURLResponse) {
        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        request.timeoutInterval = 60
        for (name, value) in headers {
            request.setValue(value, forHTTPHeaderField: name)
        }
        if let backgroundUploader {
            return try await backgroundUploader.upload(request, fromFile: file)
        }
        let size = try FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber
        guard let stream = InputStream(url: file) else { throw URLError(.fileDoesNotExist) }
        request.httpBodyStream = stream
        if let size {
            request.setValue(size.stringValue, forHTTPHeaderField: "Content-Length")
        }
        return try await send(request)
    }
}

/// The app's one background `URLSession` for blob `PUT`s (SYNC-5, trap 12).
///
/// iOS allows a single session per identifier in a process, so there is exactly one
/// instance (`shared`), created on first use (the sync service at launch, or the app
/// delegate when the system relaunches the app for finished uploads). Uploads run from
/// the queue's encrypted file, so the system finishes them after the app is suspended or
/// killed. A task is labelled with its file path: a relaunch that finds the same file
/// still uploading waits for that task instead of starting a second one, and a task that
/// finishes with nobody waiting (the app was relaunched) is reported to `orphanHandler`,
/// which marks the blob done in the queue.
public final class BackgroundBlobUploader: Sendable {
    /// Fixed for the life of the app: changing it orphans in-flight uploads.
    public static let identifier = "org.curv.asd.sync.blobs"

    public static let shared = BackgroundBlobUploader()

    /// The background configuration (`isDiscretionary = false`, launch events on).
    public static func makeConfiguration(identifier: String = BackgroundBlobUploader.identifier) -> URLSessionConfiguration {
        let configuration = URLSessionConfiguration.background(withIdentifier: identifier)
        configuration.isDiscretionary = false
        configuration.sessionSendsLaunchEvents = true
        return configuration
    }

    /// The system's completion handler from `handleEventsForBackgroundURLSession`, called
    /// once the session has delivered every pending event.
    @MainActor private static var pendingEventsCompletion: (() -> Void)?

    private let delegate: BackgroundUploadDelegate
    private let session: URLSession

    private init() {
        delegate = BackgroundUploadDelegate()
        session = URLSession(configuration: Self.makeConfiguration(), delegate: delegate, delegateQueue: nil)
        delegate.onEventsFinished = {
            Task { @MainActor in
                let completion = BackgroundBlobUploader.pendingEventsCompletion
                BackgroundBlobUploader.pendingEventsCompletion = nil
                completion?()
            }
        }
    }

    /// Called by the app delegate for `identifier`; returns `false` for another session.
    @MainActor
    public static func handleEventsForBackgroundURLSession(identifier: String, completionHandler: @escaping () -> Void) -> Bool {
        guard identifier == Self.identifier else { return false }
        pendingEventsCompletion = completionHandler
        // Re-creates the session (and its delegate) if this launch has not yet.
        _ = shared.session
        return true
    }

    /// Receives `(file path, HTTP status)` for uploads that finished with nobody waiting.
    public func setOrphanHandler(_ handler: @escaping @Sendable (String, Int) -> Void) {
        delegate.setOrphanHandler(handler)
    }

    func upload(_ request: URLRequest, fromFile file: URL) async throws -> (Data, HTTPURLResponse) {
        let key = file.standardizedFileURL.path
        let tasks = await session.allTasks
        if let running = tasks.first(where: { $0.taskDescription == key && $0.state == .running }) {
            return try await delegate.wait(for: running.taskIdentifier)
        }
        let task = session.uploadTask(with: request, fromFile: file)
        task.taskDescription = key
        return try await delegate.start(task)
    }
}

/// Delegate state behind a lock: waiters per task, response bodies, and results that
/// arrived before anyone waited.
private final class BackgroundUploadDelegate: NSObject, URLSessionDataDelegate, Sendable {
    private struct State {
        var waiters: [Int: CheckedContinuation<(Data, HTTPURLResponse), Error>] = [:]
        var bodies: [Int: Data] = [:]
        var finished: [Int: Result<(Data, HTTPURLResponse), Error>] = [:]
        var orphanHandler: (@Sendable (String, Int) -> Void)?
        var onEventsFinished: (@Sendable () -> Void)?
    }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "BackgroundBlobUploader")
    private let state = OSAllocatedUnfairLock(initialState: State())

    var onEventsFinished: (@Sendable () -> Void)? {
        get { state.withLock { $0.onEventsFinished } }
        set { state.withLock { $0.onEventsFinished = newValue } }
    }

    func setOrphanHandler(_ handler: @escaping @Sendable (String, Int) -> Void) {
        state.withLock { $0.orphanHandler = handler }
    }

    func start(_ task: URLSessionUploadTask) async throws -> (Data, HTTPURLResponse) {
        try await withCheckedThrowingContinuation { continuation in
            state.withLock { $0.waiters[task.taskIdentifier] = continuation }
            task.resume()
        }
    }

    func wait(for taskIdentifier: Int) async throws -> (Data, HTTPURLResponse) {
        try await withCheckedThrowingContinuation { continuation in
            let early = state.withLock { state -> Result<(Data, HTTPURLResponse), Error>? in
                if let result = state.finished.removeValue(forKey: taskIdentifier) { return result }
                state.waiters[taskIdentifier] = continuation
                return nil
            }
            if let early { continuation.resume(with: early) }
        }
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        state.withLock { $0.bodies[dataTask.taskIdentifier, default: Data()].append(data) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        let id = task.taskIdentifier
        let result: Result<(Data, HTTPURLResponse), Error>
        let (waiter, body, orphanHandler) = state.withLock { state in
            (state.waiters.removeValue(forKey: id), state.bodies.removeValue(forKey: id) ?? Data(), state.orphanHandler)
        }
        if let error {
            result = .failure(error)
        } else if let http = task.response as? HTTPURLResponse {
            result = .success((body, http))
        } else {
            result = .failure(URLError(.badServerResponse))
        }
        if let waiter {
            waiter.resume(with: result)
            return
        }
        // Nobody waits in this process (relaunch): keep the result briefly for a late
        // `wait(for:)` and tell the queue.
        state.withLock { state in
            state.finished[id] = result
            if state.finished.count > 64, let oldest = state.finished.keys.min() {
                state.finished.removeValue(forKey: oldest)
            }
        }
        if case .success(let (_, http)) = result, let path = task.taskDescription {
            Self.logger.info("Background upload finished after relaunch (HTTP \(http.statusCode, privacy: .public))")
            orphanHandler?(path, http.statusCode)
        }
    }

    func urlSessionDidFinishEvents(forBackgroundURLSession session: URLSession) {
        state.withLock { $0.onEventsFinished }?()
    }
}

/// Header names of the device-plane contract (`02 §2 ADR-B4, ADR-B5`).
public enum SyncHeader {
    public static let authorization = "Authorization"
    public static let idempotencyKey = "X-Idempotency-Key"
    public static let schemaVersion = "X-Schema-Version"
    public static let userAgent = "User-Agent"
    public static let contentType = "Content-Type"
    public static let accept = "Accept"
    public static let retryAfter = "Retry-After"
    /// RFC 9530 digest of a blob body (SYNC-6).
    public static let contentDigest = "Content-Digest"
}

/// App identity sent as `User-Agent` and in device registration.
public struct SyncClientInfo: Sendable, Equatable {
    public let appVersion: String
    public let build: String
    public let osVersion: String

    public init(appVersion: String, build: String, osVersion: String) {
        self.appVersion = appVersion
        self.build = build
        self.osVersion = osVersion
    }

    /// `EyeGaze-iOS/<marketing version> (<build>; iOS <os version>)`. No device
    /// model, name or identifier.
    public var userAgent: String {
        "EyeGaze-iOS/\(appVersion) (\(build); iOS \(osVersion))"
    }

    @MainActor
    public static var current: SyncClientInfo {
        let info = Bundle.main.infoDictionary ?? [:]
        return SyncClientInfo(
            appVersion: info["CFBundleShortVersionString"] as? String ?? "0",
            build: info["CFBundleVersion"] as? String ?? "0",
            osVersion: UIDevice.current.systemVersion
        )
    }
}

// `Retry-After` parsing (delta-seconds only, capped at 3600 s) moved to asd-sync
// (`classify_response`, CON-7); `SessionSyncClient` reads it from the outcome.
