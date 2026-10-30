import Foundation
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
}

public struct URLSessionSyncTransport: SyncTransport {
    public let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw URLError(.badServerResponse)
        }
        return (data, http)
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

/// Seconds from a `Retry-After` header. Only the delta-seconds form is honoured;
/// an HTTP-date or garbage falls back to the client's own schedule.
func retryAfterSeconds(_ response: HTTPURLResponse) -> TimeInterval? {
    guard let raw = response.value(forHTTPHeaderField: SyncHeader.retryAfter)?
        .trimmingCharacters(in: .whitespaces),
        let seconds = TimeInterval(raw), seconds >= 0, seconds.isFinite else { return nil }
    // A hostile or broken server must not park the queue for days.
    return min(seconds, 3600)
}
