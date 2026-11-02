import CryptoKit
import Foundation
@testable import AsdGazeCollector

// SYNC-9 field harness, device side: a virtual clock, a stub `/v1` ingest server and a
// `SyncTransport` that scripts what the network does to each request. Nothing sleeps:
// latency advances the virtual clock, so a two-minute flapping scenario runs in seconds
// and the same seed always gives the same run.

/// Time as the sync service, the injector and the scenario driver see it.
final class VirtualClock: @unchecked Sendable {
    private let lock = NSLock()
    private var instant: TimeInterval

    init(start: TimeInterval = 1_793_000_000) {
        instant = start
    }

    var now: Date { lock.withLock { Date(timeIntervalSince1970: instant) } }
    var seconds: TimeInterval { lock.withLock { instant } }

    func advance(by seconds: TimeInterval) {
        lock.withLock { instant += max(seconds, 0) }
    }
}

/// The server half: idempotent `POST /v1/sessions` (replays answer `200 duplicate:true`
/// with the upload URL again, CON-17) and proxy-mode `PUT /v1/sessions/<id>/upload/frames`
/// that checks `Content-Digest` (SYNC-6) and records partial bodies as incomplete.
final class StubIngestServer: @unchecked Sendable {
    private let lock = NSLock()
    private var accepted: [String] = []
    private var duplicates = 0
    private var blobs: [String: Data] = [:]
    private var expected: [String: Data] = [:]
    private var digestFailures = 0

    /// Session ids accepted by a first POST, in order (a replay never adds one).
    var acceptedSessionIds: [String] { lock.withLock { accepted } }
    /// POSTs answered `200 duplicate:true` (allowed, counted).
    var duplicatePosts: Int { lock.withLock { duplicates } }
    /// Blobs whose stored bytes are the whole file with a matching digest.
    var completeBlobs: Int { lock.withLock { blobs.filter { expected[$0.key] == $0.value }.count } }
    var incompleteBlobs: Int { lock.withLock { blobs.filter { expected[$0.key] != $0.value }.count } }
    var digestMismatches: Int { lock.withLock { digestFailures } }

    /// The bytes the device will upload for `sessionId`, so completeness can be checked.
    func expectBlob(_ bytes: Data, for sessionId: String) {
        lock.withLock { expected[sessionId] = bytes }
    }

    static func uploadPath(_ sessionId: String) -> String { "/v1/sessions/\(sessionId)/upload/frames" }

    func handle(_ request: URLRequest) -> (Int, [String: String], Data) {
        let path = request.url?.path ?? ""
        let body = RequestLog.body(of: request)
        switch (request.httpMethod ?? "GET", path) {
        case ("POST", "/v1/sessions"):
            guard let object = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
                  let sessionId = object["sessionId"] as? String else {
                return (400, ["Content-Type": "application/problem+json"], Data())
            }
            let replay = lock.withLock { () -> Bool in
                if accepted.contains(sessionId) {
                    duplicates += 1
                    return true
                }
                accepted.append(sessionId)
                return false
            }
            let answer: [String: Any] = [
                "sessionId": sessionId,
                "status": "ingested",
                "duplicate": replay,
                "frames_upload_url": Self.uploadPath(sessionId),
                "report_upload_url": NSNull()
            ]
            return (replay ? 200 : 201, ["Content-Type": "application/json"], jsonData(answer))
        case ("PUT", let p) where p.hasPrefix("/v1/sessions/") && p.hasSuffix("/upload/frames"):
            let sessionId = String(p.dropFirst("/v1/sessions/".count).dropLast("/upload/frames".count))
            if let digest = request.value(forHTTPHeaderField: SyncHeader.contentDigest) {
                let actual = "sha-256=:\(Data(SHA256.hash(data: body)).base64EncodedString()):"
                guard digest == actual else {
                    lock.withLock { digestFailures += 1 }
                    return (400, ["Content-Type": "application/problem+json"],
                            jsonData(["type": "https://schemas.curv.org/asd/problems/digest-mismatch"]))
                }
            }
            lock.withLock { blobs[sessionId] = body }
            return (200, [:], Data())
        default:
            return (404, [:], Data())
        }
    }

    /// A PUT whose connection died after `bytes`: the server keeps what arrived.
    func receivePartial(_ request: URLRequest, bytes: Int) {
        let path = request.url?.path ?? ""
        guard request.httpMethod == "PUT", path.hasSuffix("/upload/frames") else { return }
        let sessionId = String(path.dropFirst("/v1/sessions/".count).dropLast("/upload/frames".count))
        let body = RequestLog.body(of: request)
        lock.withLock { blobs[sessionId] = body.prefix(bytes) }
    }
}

/// A `SyncTransport` that scripts per-request outcomes (SYNC-9).
final class FaultInjectingTransport: SyncTransport, @unchecked Sendable {
    enum Fault: Equatable, Sendable {
        /// No response: a transport error after `latencyMs` (the server may or may not
        /// have processed the request: `reachedServer`).
        case drop(latencyMs: Int, reachedServer: Bool)
        /// Answered by the server after `ms`.
        case latency(ms: Int)
        /// Answered with `code` without reaching the stub server (`Retry-After` and a
        /// body optional).
        case status(Int, retryAfter: Int?, body: Data)
        /// PUT only: the connection dies after `afterBytes` of the body.
        case cutBody(afterBytes: Int)

        static func status(_ code: Int, retryAfter: Int? = nil) -> Fault {
            .status(code, retryAfter: retryAfter, body: Data())
        }

        static func problem(_ code: Int, slug: String) -> Fault {
            .status(code, retryAfter: nil, body: jsonData([
                "type": "https://schemas.curv.org/asd/problems/\(slug)",
                "title": "Conflict",
                "status": code
            ]))
        }
    }

    /// Online, then offline, alternately every `period` seconds from `start` until `until`
    /// (virtual time); online afterwards.
    struct FlapPattern: Sendable {
        let start: TimeInterval
        let period: TimeInterval
        let until: TimeInterval
        let startsOnline: Bool

        func isOnline(at t: TimeInterval) -> Bool {
            guard t >= start, t < until else { return true }
            let phase = Int(((t - start) / period).rounded(.down)) % 2
            return startsOnline ? phase == 0 : phase == 1
        }
    }

    /// Seeded faults for requests nothing is scripted for: `dropRate` drops, latency
    /// uniform in `latencyMs`.
    struct RandomFaults: Sendable {
        var dropRate: Double
        var latencyMs: ClosedRange<Int>
    }

    let server: StubIngestServer
    let clock: VirtualClock
    private let lock = NSLock()
    private var scripted: [(method: String, pathSuffix: String, faults: [Fault])] = []
    private var random: RandomFaults?
    private var rng: UInt64
    private var pattern: FlapPattern?
    private var reportedOnline: Bool?
    private var _requests: [(method: String, path: String, at: TimeInterval)] = []
    /// Drives the service's online state (`handlePathUpdate`) when the pattern changes.
    var onPathChange: (@MainActor (Bool) -> Void)?

    init(server: StubIngestServer, clock: VirtualClock, seed: UInt64 = 0x5EED_CAFE) {
        self.server = server
        self.clock = clock
        self.rng = seed
    }

    /// Requests that reached the transport while online: method, path, virtual time.
    var requests: [(method: String, path: String, at: TimeInterval)] { lock.withLock { _requests } }

    func requests(method: String, pathSuffix: String) -> [TimeInterval] {
        requests.filter { $0.method == method && $0.path.hasSuffix(pathSuffix) }.map(\.at)
    }

    /// The next requests matching `method` and a path ending in `pathSuffix` get `faults`,
    /// one each, in order.
    func script(_ method: String, _ pathSuffix: String, _ faults: Fault...) {
        lock.withLock { scripted.append((method, pathSuffix, faults)) }
    }

    func randomFaults(_ faults: RandomFaults?) {
        lock.withLock { random = faults }
    }

    func flapOnline(_ pattern: FlapPattern?) {
        lock.withLock { self.pattern = pattern }
    }

    /// Reports the pattern's state at the current virtual time if it changed.
    @MainActor
    func reportPath() {
        let online = lock.withLock { pattern?.isOnline(at: clock.seconds) ?? true }
        guard online != reportedOnline else { return }
        reportedOnline = online
        onPathChange?(online)
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        try await deliver(request)
    }

    func uploadFile(_ file: URL, to url: URL, headers: [String: String]) async throws -> (Data, HTTPURLResponse) {
        try await deliver(StubSyncTransport.uploadRequest(file, to: url, headers: headers))
    }

    // MARK: - Pipeline

    private func deliver(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        await MainActor.run { reportPath() }
        let online = lock.withLock { pattern?.isOnline(at: clock.seconds) ?? true }
        guard online else { throw URLError(.notConnectedToInternet) }
        let method = request.httpMethod ?? "GET"
        let path = request.url?.path ?? ""
        lock.withLock { _requests.append((method, path, clock.seconds)) }

        switch nextFault(method: method, path: path) {
        case .latency(let ms):
            clock.advance(by: Double(ms) / 1000)
            return respond(request, server.handle(request))
        case .drop(let ms, let reachedServer):
            clock.advance(by: Double(ms) / 1000)
            if reachedServer { _ = server.handle(request) }
            throw URLError(.timedOut)
        case .status(let code, let retryAfter, let body):
            var headers: [String: String] = [:]
            if let retryAfter { headers["Retry-After"] = String(retryAfter) }
            if !body.isEmpty { headers["Content-Type"] = "application/problem+json" }
            return respond(request, (code, headers, body))
        case .cutBody(let bytes):
            server.receivePartial(request, bytes: bytes)
            throw URLError(.networkConnectionLost)
        }
    }

    private func nextFault(method: String, path: String) -> Fault {
        lock.withLock {
            if let index = scripted.firstIndex(where: { $0.method == method && path.hasSuffix($0.pathSuffix) && !$0.faults.isEmpty }) {
                let fault = scripted[index].faults.removeFirst()
                if scripted[index].faults.isEmpty { scripted.remove(at: index) }
                return fault
            }
            guard let random else { return .latency(ms: 0) }
            let latency = random.latencyMs.lowerBound
                + Int(nextRandom() % UInt64(random.latencyMs.count))
            if Double(nextRandom() % 10_000) / 10_000 < random.dropRate {
                return .drop(latencyMs: latency, reachedServer: nextRandom() % 2 == 0)
            }
            return .latency(ms: latency)
        }
    }

    /// xorshift64*: deterministic for a seed. Called under `lock`.
    private func nextRandom() -> UInt64 {
        rng ^= rng >> 12
        rng ^= rng << 25
        rng ^= rng >> 27
        return rng &* 2_685_821_657_736_338_717
    }

    private func respond(_ request: URLRequest, _ answer: (Int, [String: String], Data)) -> (Data, HTTPURLResponse) {
        let response = HTTPURLResponse(url: request.url!, statusCode: answer.0, httpVersion: "HTTP/1.1", headerFields: answer.1)!
        return (answer.2, response)
    }
}
