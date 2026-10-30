import Foundation
@testable import AsdGazeCollector

/// Scripted `SyncTransport` for tests that need no URL loading system at all.
/// Each request is answered by `handler`; every request is recorded.
final class StubSyncTransport: SyncTransport, @unchecked Sendable {
    private let lock = NSLock()
    private var _requests: [URLRequest] = []
    var handler: @Sendable (URLRequest) throws -> (Int, [String: String], Data)

    init(handler: @escaping @Sendable (URLRequest) throws -> (Int, [String: String], Data)) {
        self.handler = handler
    }

    var requests: [URLRequest] {
        lock.withLock { _requests }
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        lock.withLock { _requests.append(request) }
        let (status, headers, body) = try handler(request)
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
        return (body, response)
    }
}

/// Process-memory credential store; the Keychain store has its own test.
final class InMemoryDeviceCredentialStore: DeviceCredentialStore, @unchecked Sendable {
    private let lock = NSLock()
    private var credential: DeviceCredential?
    private(set) var saveCount = 0

    init(_ credential: DeviceCredential? = nil) {
        self.credential = credential
    }

    func load() -> DeviceCredential? {
        lock.lock(); defer { lock.unlock() }
        return credential
    }

    func save(_ credential: DeviceCredential) throws {
        lock.lock(); defer { lock.unlock() }
        self.credential = credential
        saveCount += 1
    }

    func clear() {
        lock.lock(); defer { lock.unlock() }
        credential = nil
    }
}

/// Thread-safe log of requests seen by `MockURLProtocol`, with bodies read
/// from `httpBodyStream` (URLSession hands protocols a stream, not `httpBody`).
final class RequestLog: @unchecked Sendable {
    struct Entry {
        let method: String
        let url: URL
        let headers: [String: String]
        let body: Data
    }

    private let lock = NSLock()
    private var _entries: [Entry] = []

    var entries: [Entry] {
        lock.lock(); defer { lock.unlock() }
        return _entries
    }

    func record(_ request: URLRequest) -> Entry {
        let entry = Entry(
            method: request.httpMethod ?? "GET",
            url: request.url!,
            headers: request.allHTTPHeaderFields ?? [:],
            body: Self.body(of: request)
        )
        lock.lock()
        _entries.append(entry)
        lock.unlock()
        return entry
    }

    func entries(path: String) -> [Entry] {
        entries.filter { $0.url.path == path }
    }

    static func body(of request: URLRequest) -> Data {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: buffer.count)
            if read <= 0 { break }
            data.append(buffer, count: read)
        }
        return data
    }
}

func httpResponse(_ request: URLRequest, _ status: Int, headers: [String: String]? = nil) -> HTTPURLResponse {
    HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
}

func jsonData(_ object: Any) -> Data {
    (try? JSONSerialization.data(withJSONObject: object)) ?? Data()
}
