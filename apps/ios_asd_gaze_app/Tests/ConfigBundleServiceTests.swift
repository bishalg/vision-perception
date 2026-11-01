@testable import AsdGazeCollector
import CurvAsdGazeKit
import Foundation
import XCTest

/// WP-A.5: config bundle fetch, verification through the Rust core, persistence of the last
/// good bundle and the bundled fallback. Fixtures `Tests/Fixtures/config-bundle/` are signed
/// with fixed test seeds by `rust/crates/asd-sync/tests/config_bundle.rs` (never a production
/// key); the delivered profile differs from the default (`greenFloor` 0.80).
final class ConfigBundleServiceTests: XCTestCase {
    static let dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .appendingPathComponent("Fixtures/config-bundle")
    static let api = URL(string: "https://api.example.org")!
    /// A day after the valid fixture was signed.
    static let now = Date(timeIntervalSince1970: 1_793_577_600)

    private var tempDir: URL!

    override func setUpWithError() throws {
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ConfigBundleTests-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDir)
        ScoringConfiguration.install(.bundled)
    }

    static func data(_ name: String) throws -> Data {
        try Data(contentsOf: dir.appendingPathComponent(name))
    }

    static func key(_ role: String) throws -> TrustedBundleKey {
        try XCTUnwrap(JSONDecoder().decode([String: TrustedBundleKey].self, from: data("test-keys.json"))[role])
    }

    static func trust(maxAge: TimeInterval? = nil) throws -> ConfigBundleTrust {
        ConfigBundleTrust(trustedKeys: [try key("trusted")], maxAge: maxAge)
    }

    private var store: ConfigBundleStore { ConfigBundleStore(directory: tempDir) }

    private func client(
        trust: ConfigBundleTrust,
        credential: Bool = true,
        handler: @escaping @Sendable (URLRequest) throws -> (Int, [String: String], Data)
    ) -> (ConfigBundleClient, StubSyncTransport) {
        let transport = StubSyncTransport(handler: handler)
        let info = SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")
        let creds = InMemoryDeviceCredentialStore(credential ? DeviceCredential(
            deviceToken: "tok", orgId: "org", deviceId: "dev", configBundleVersion: nil,
            apiHost: "api.example.org", enrolCode: nil, registeredAt: Self.now
        ) : nil)
        let registration = DeviceRegistrationService(apiBaseURL: Self.api, transport: transport, store: creds, clientInfo: info)
        let client = ConfigBundleClient(
            apiBaseURL: Self.api, transport: transport, registration: registration, clientInfo: info,
            store: store, trust: trust, now: { Self.now }
        )
        return (client, transport)
    }

    // MARK: - Trust configuration

    func testTrustReadsKeysAndMaxAgeFromInfoPlist() {
        let info: [String: Any] = ["ASDConfigBundle": [
            "trustedKeys": [["keyId": "k1", "publicKey": "AAAA"], ["keyId": "", "publicKey": "x"], ["keyId": "k2"]],
            "maxAgeDays": 30
        ]]
        let trust = ConfigBundleTrust.load(from: info)
        XCTAssertEqual(trust.trustedKeys, [TrustedBundleKey(keyId: "k1", publicKey: "AAAA")])
        XCTAssertEqual(trust.maxAge, 30 * 86_400)
        XCTAssertFalse(ConfigBundleTrust.load(from: [:]).isEnabled, "no keys: delivered bundles are off")
        // The shipped build trusts no key yet: the bundled configuration scores.
        XCTAssertEqual(ConfigBundleTrust.load(from: Bundle.main.infoDictionary).trustedKeys, [])
    }

    // MARK: - Fetch and verify

    func testValidBundleIsVerifiedPersistedAndUsedFromTheNextLaunch() async throws {
        let body = try Self.data("bundle-valid.json")
        let (client, transport) = client(trust: try Self.trust()) { _ in (200, ["ETag": "\"2026.11.01-2\""], body) }
        let outcome = await client.refresh()
        XCTAssertEqual(outcome, .updated(version: "2026.11.01-2"))
        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertEqual(request.url?.absoluteString, "https://api.example.org/v1/config/bundle")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer tok")
        XCTAssertNil(request.value(forHTTPHeaderField: "If-None-Match"), "nothing held yet")
        XCTAssertEqual(store.heldVersion, "2026.11.01-2")

        // Activation happens at launch: the persisted bundle is verified again and installed.
        let active = store.installAtLaunch(trust: try Self.trust(), now: Self.now)
        XCTAssertEqual(active.bundleVersion, "2026.11.01-2")
        XCTAssertEqual(ScoringConfiguration.active.bundleVersion, "2026.11.01-2")

        // The next fetch asks for changes only.
        let (again, transport2) = self.client(trust: try Self.trust()) { _ in (304, [:], Data()) }
        let second = await again.refresh()
        XCTAssertEqual(second, .notModified)
        XCTAssertEqual(transport2.requests.first?.value(forHTTPHeaderField: "If-None-Match"), "\"2026.11.01-2\"")
    }

    func testTamperedBundleIsRefusedAndNothingChanges() async throws {
        let tampered = String(decoding: try Self.data("bundle-valid.json"), as: UTF8.self)
            .replacingOccurrences(of: "\"greenFloor\": 0.8", with: "\"greenFloor\": 0.1")
        let (client, _) = client(trust: try Self.trust()) { _ in (200, [:], Data(tampered.utf8)) }
        let outcome = await client.refresh()
        XCTAssertEqual(outcome, .rejected(.badSignature))
        XCTAssertNil(store.heldVersion)
        XCTAssertEqual(store.activeConfiguration(trust: try Self.trust(), now: Self.now).source, .bundled)
    }

    func testWrongKeyAndUnknownKeyAreRefused() async throws {
        for (name, reason) in [("bundle-wrong-key.json", BundleRejection.badSignature), ("bundle-unknown-key.json", .unknownKey)] {
            let body = try Self.data(name)
            let (client, _) = client(trust: try Self.trust()) { _ in (200, [:], body) }
            let outcome = await client.refresh()
            XCTAssertEqual(outcome, .rejected(reason), name)
        }
        XCTAssertNil(store.heldVersion)
    }

    func testRollbackToAnOlderBundleIsRefused() async throws {
        try store.save(bundleJSON: Self.data("bundle-valid.json"), version: "2026.11.01-2")
        let older = try Self.data("bundle-older.json")
        let (client, _) = client(trust: try Self.trust()) { _ in (200, [:], older) }
        let outcome = await client.refresh()
        XCTAssertEqual(outcome, .rejected(.rollback))
        XCTAssertEqual(store.heldVersion, "2026.11.01-2", "the newer bundle stays")
    }

    func testExpiredBundleFallsBackToTheBundledDefault() throws {
        try store.save(bundleJSON: Self.data("bundle-older.json"), version: "2026.10.30-1")
        XCTAssertEqual(store.activeConfiguration(trust: try Self.trust(), now: Self.now).bundleVersion, "2026.10.30-1")
        let strict = try Self.trust(maxAge: 2 * 86_400)
        XCTAssertEqual(store.activeConfiguration(trust: strict, now: Self.now).source, .bundled)
    }

    func testOfflineKeepsTheHeldBundleAndNoneMeansBundled() async throws {
        // Nothing held, offline: the bundled default.
        let (offline, _) = client(trust: try Self.trust()) { _ in throw URLError(.notConnectedToInternet) }
        let first = await offline.refresh()
        XCTAssertEqual(first, .unavailable)
        XCTAssertEqual(store.installAtLaunch(trust: try Self.trust(), now: Self.now).source, .bundled)

        // A held bundle survives an offline launch and a 404.
        try store.save(bundleJSON: Self.data("bundle-valid.json"), version: "2026.11.01-2")
        let second = await offline.refresh()
        XCTAssertEqual(second, .unavailable)
        let (none, _) = client(trust: try Self.trust()) { _ in (404, [:], Data()) }
        let third = await none.refresh()
        XCTAssertEqual(third, .noBundle)
        XCTAssertEqual(store.installAtLaunch(trust: try Self.trust(), now: Self.now).bundleVersion, "2026.11.01-2")
    }

    func testCorruptOrUntrustedPersistedBundleFallsBack() throws {
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        try Data("{".utf8).write(to: store.bundleURL)
        XCTAssertEqual(store.activeConfiguration(trust: try Self.trust(), now: Self.now).source, .bundled)
        try store.save(bundleJSON: Self.data("bundle-valid.json"), version: "2026.11.01-2")
        // A build that no longer trusts that key (rotated out) ignores the stored bundle.
        let rotated = ConfigBundleTrust(trustedKeys: [try Self.key("untrusted")])
        XCTAssertEqual(store.activeConfiguration(trust: rotated, now: Self.now).source, .bundled)
        XCTAssertEqual(store.activeConfiguration(trust: ConfigBundleTrust(trustedKeys: []), now: Self.now).source, .bundled)
    }

    func testDisabledOrUnregisteredDoesNotFetch() async throws {
        let (disabled, t1) = client(trust: ConfigBundleTrust(trustedKeys: [])) { _ in (200, [:], Data()) }
        let off = await disabled.refresh()
        XCTAssertEqual(off, .disabled)
        XCTAssertTrue(t1.requests.isEmpty)
        let (unregistered, t2) = client(trust: try Self.trust(), credential: false) { _ in (200, [:], Data()) }
        let none = await unregistered.refresh()
        XCTAssertEqual(none, .notRegistered)
        XCTAssertTrue(t2.requests.isEmpty)
    }
}
