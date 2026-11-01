import AsdCore
import CurvAsdGazeKit
import Foundation
import os

// WP-A.5: delivered config bundles (catalog, baselines, ScoringProfile; 02 ADR-B6).
//
// Trust model:
// - The app trusts only the Ed25519 public keys in its Info.plist (`ASDConfigBundle.trustedKeys`,
//   `{keyId, publicKey}`); the private key lives only on the server (`ASD_BUNDLE_SIGNING_KEY`).
//   No keys configured = no delivered bundle is ever used (the bundled default scores).
// - Rotation: several keys may be listed; a bundle names its key by `key_id`. Ship the new key
//   in a release, switch the server, drop the old key in a later release.
// - Every decision (signature over JCS, key id, no rollback below the newest accepted version,
//   optional maximum age `ASDConfigBundle.maxAgeDays`) is the Rust core's
//   (`config_accept_bundle`); Swift does no crypto.
// - A fetched bundle is persisted and takes effect at the next launch, so a session never
//   changes configuration halfway. At launch the persisted bundle is verified again; anything
//   absent, unreadable or refused falls back to the bundled default.

/// The bundle trust configuration of this build (Info.plist `ASDConfigBundle`).
public struct ConfigBundleTrust: Sendable, Equatable {
    public static let infoPlistKey = "ASDConfigBundle"

    public let trustedKeys: [TrustedBundleKey]
    /// Refuse bundles signed longer ago than this; `nil` = no expiry (default: an unchanged
    /// published bundle stays valid, it is not replaced by the bundled default).
    public let maxAge: TimeInterval?

    public init(trustedKeys: [TrustedBundleKey], maxAge: TimeInterval? = nil) {
        self.trustedKeys = trustedKeys
        self.maxAge = maxAge
    }

    /// Delivered bundles are fetched and trusted only with at least one key.
    public var isEnabled: Bool { !trustedKeys.isEmpty }

    /// Reads `{trustedKeys: [{keyId, publicKey}], maxAgeDays?}`; malformed entries are skipped
    /// (a malformed key would be refused by the core anyway).
    public static func load(from info: [String: Any]?) -> ConfigBundleTrust {
        guard let dict = info?[infoPlistKey] as? [String: Any] else { return ConfigBundleTrust(trustedKeys: []) }
        let keys = (dict["trustedKeys"] as? [[String: Any]] ?? []).compactMap { entry -> TrustedBundleKey? in
            guard let id = entry["keyId"] as? String, let key = entry["publicKey"] as? String,
                  !id.isEmpty, !key.isEmpty else { return nil }
            return TrustedBundleKey(keyId: id, publicKey: key)
        }
        let days = (dict["maxAgeDays"] as? NSNumber)?.doubleValue
        return ConfigBundleTrust(trustedKeys: keys, maxAge: days.flatMap { $0 > 0 ? $0 * 86_400 : nil })
    }

    public static var current: ConfigBundleTrust { load(from: Bundle.main.infoDictionary) }
}

/// The last accepted bundle (verbatim bytes, so the signature can be checked again) and the
/// newest version this device accepted (the rollback anchor), in Application Support.
public struct ConfigBundleStore: Sendable {
    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ConfigBundle")

    public let directory: URL

    public init(directory: URL) {
        self.directory = directory
    }

    public static var standard: ConfigBundleStore {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return ConfigBundleStore(directory: base.appendingPathComponent("ConfigBundle", isDirectory: true))
    }

    var bundleURL: URL { directory.appendingPathComponent("bundle.json") }
    var stateURL: URL { directory.appendingPathComponent("state.json") }

    private struct State: Codable {
        var newestAcceptedVersion: String?
        var heldVersion: String?
    }

    private func state() -> State {
        guard let data = try? Data(contentsOf: stateURL),
              let state = try? JSONDecoder().decode(State.self, from: data) else { return State() }
        return state
    }

    /// The newest bundle version this device ever accepted.
    public var newestAcceptedVersion: String? { state().newestAcceptedVersion }

    /// The version of the persisted bundle (sent as `If-None-Match`).
    public var heldVersion: String? { state().heldVersion }

    /// Persists an accepted bundle exactly as received.
    public func save(bundleJSON: Data, version: String) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try bundleJSON.write(to: bundleURL, options: .atomic)
        let next = State(newestAcceptedVersion: version, heldVersion: version)
        try JSONEncoder().encode(next).write(to: stateURL, options: .atomic)
    }

    /// The configuration to score with: the persisted bundle when the core accepts it again
    /// (same keys, no rollback, not expired), else the bundled default.
    public func activeConfiguration(trust: ConfigBundleTrust, now: Date = Date()) -> ScoringConfiguration {
        guard trust.isEnabled, let data = try? Data(contentsOf: bundleURL) else { return .bundled }
        let result = ScoringConfiguration.accept(
            bundleJSON: String(decoding: data, as: UTF8.self),
            trustedKeys: trust.trustedKeys,
            newestAcceptedVersion: newestAcceptedVersion,
            now: now,
            maxAge: trust.maxAge
        )
        switch result {
        case .success(let configuration):
            return configuration
        case .failure(let reason):
            Self.logger.error("Persisted config bundle refused (\(reason.rawValue, privacy: .public)); using the bundled default")
            return .bundled
        }
    }

    /// Installs `activeConfiguration` as the configuration of this launch.
    @discardableResult
    public func installAtLaunch(trust: ConfigBundleTrust = .current, now: Date = Date()) -> ScoringConfiguration {
        let configuration = activeConfiguration(trust: trust, now: now)
        ScoringConfiguration.install(configuration)
        Self.logger.info("Scoring configuration: \(configuration.bundleVersion ?? "bundled", privacy: .public)")
        return configuration
    }
}

/// Fetches `GET /v1/config/bundle` with the device bearer and keeps the bundle when the core
/// accepts it. Request headers and the meaning of the answer come from `asd-sync`.
public struct ConfigBundleClient: Sendable {
    public enum Outcome: Equatable, Sendable {
        /// A newer bundle was accepted and persisted; it scores from the next launch.
        case updated(version: String)
        case notModified
        /// The server has no bundle for this device; the held one (or the default) stays.
        case noBundle
        /// The bundle was refused; nothing changed.
        case rejected(BundleRejection)
        /// The build trusts no key: delivered bundles are off.
        case disabled
        case notRegistered
        /// Transport failure or a status worth retrying later.
        case unavailable
    }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ConfigBundle")

    public let apiBaseURL: URL
    public let registration: DeviceRegistrationService
    public let store: ConfigBundleStore
    public let trust: ConfigBundleTrust
    private let transport: SyncTransport
    private let clientInfo: SyncClientInfo
    private let now: @Sendable () -> Date

    public init(
        apiBaseURL: URL,
        transport: SyncTransport,
        registration: DeviceRegistrationService,
        clientInfo: SyncClientInfo,
        store: ConfigBundleStore = .standard,
        trust: ConfigBundleTrust = .current,
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.apiBaseURL = apiBaseURL
        self.transport = transport
        self.registration = registration
        self.clientInfo = clientInfo
        self.store = store
        self.trust = trust
        self.now = now
    }

    func makeRequest(token: String) throws -> URLRequest {
        let built: AsdSyncRequest
        do {
            built = try configBuildBundleRequest(deviceToken: token, heldVersion: store.heldVersion, client: clientInfo.asdClientInfo)
        } catch {
            throw SessionSyncError.notRegistered
        }
        var request = try URLRequest(built, apiBaseURL: apiBaseURL)
        if built.body.isEmpty { request.httpBody = nil }
        return request
    }

    /// One fetch. A 401/403 re-registers once and retries.
    public func refresh() async -> Outcome {
        guard trust.isEnabled else { return .disabled }
        guard var credential = registration.currentCredential() else { return .notRegistered }
        do {
            var (data, outcome) = try await send(makeRequest(token: credential.deviceToken))
            if case .reauthenticate = outcome {
                credential = try await registration.reRegister()
                (data, outcome) = try await send(makeRequest(token: credential.deviceToken))
            }
            switch outcome {
            case .fresh:
                return accept(data)
            case .notModified:
                return .notModified
            case .noBundle:
                return .noBundle
            case .reauthenticate:
                return .notRegistered
            case .retryAfter, .fatal:
                return .unavailable
            }
        } catch is DeviceRegistrationError {
            return .notRegistered
        } catch SessionSyncError.notRegistered {
            return .notRegistered
        } catch {
            Self.logger.info("Config bundle fetch failed: \(String(describing: type(of: error)), privacy: .public)")
            return .unavailable
        }
    }

    private func accept(_ data: Data) -> Outcome {
        let result = ScoringConfiguration.accept(
            bundleJSON: String(decoding: data, as: UTF8.self),
            trustedKeys: trust.trustedKeys,
            newestAcceptedVersion: store.newestAcceptedVersion,
            now: now(),
            maxAge: trust.maxAge
        )
        switch result {
        case .failure(let reason):
            Self.logger.error("Delivered config bundle refused: \(reason.rawValue, privacy: .public)")
            return .rejected(reason)
        case .success(let configuration):
            guard let version = configuration.bundleVersion else { return .rejected(.malformed) }
            do {
                try store.save(bundleJSON: data, version: version)
            } catch {
                Self.logger.error("Config bundle \(version, privacy: .public) not persisted")
                return .unavailable
            }
            return .updated(version: version)
        }
    }

    private func send(_ request: URLRequest) async throws -> (Data, AsdConfigBundleOutcome) {
        let (data, response) = try await transport.send(request)
        let headers = response.allHeaderFields.compactMap { name, value -> AsdHttpHeader? in
            guard let name = name as? String else { return nil }
            return AsdHttpHeader(name: name, value: "\(value)")
        }
        let status = UInt16(exactly: response.statusCode) ?? 0
        return (data, configClassifyBundleResponse(status: status, headers: headers))
    }
}
