@testable import AsdGazeCollector
import Foundation
import Security
import XCTest

/// WP-A.4 / ADR-B4 device plane: enrolment-code registration, Keychain-held device token.
final class DeviceRegistrationServiceTests: XCTestCase {
    static let baseURL = URL(string: "https://api.example.org")!
    static let client = SyncClientInfo(appVersion: "1.0.0", build: "3", osVersion: "18.0")

    private func makeService(
        store: DeviceCredentialStore,
        transport: SyncTransport,
        baseURL: URL = DeviceRegistrationServiceTests.baseURL,
        appAttestEnabled: Bool = false
    ) -> DeviceRegistrationService {
        DeviceRegistrationService(
            apiBaseURL: baseURL,
            transport: transport,
            store: store,
            clientInfo: Self.client,
            appAttestEnabled: appAttestEnabled,
            now: { Date(timeIntervalSince1970: 1_793_000_000) }
        )
    }

    private static let registered: @Sendable (URLRequest) throws -> (Int, [String: String], Data) = { _ in
        (200, ["Content-Type": "application/json"], jsonData([
            "status": "registered",
            "device_id": "dev-1",
            "device_token": "tok-abc",
            "org_id": "org-home",
            "config_bundle_version": "2026.10.1"
        ]))
    }

    func testRegisterPostsEnrolCodeToDevicesRegister() async throws {
        let transport = StubSyncTransport(handler: Self.registered)
        let store = InMemoryDeviceCredentialStore()
        let credential = try await makeService(store: store, transport: transport)
            .register(.enrolCode("  ENROL-1234 \n"))

        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(transport.requests.count, 1)
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.url?.absoluteString, "https://api.example.org/v1/devices/register")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
        XCTAssertEqual(request.value(forHTTPHeaderField: "User-Agent"), "EyeGaze-iOS/1.0.0 (3; iOS 18.0)")
        XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"), "registration carries no bearer")
        XCTAssertNotNil(request.value(forHTTPHeaderField: "X-Idempotency-Key"))

        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: request.httpBody ?? Data()) as? [String: Any])
        XCTAssertEqual(body["enrol_code"] as? String, "ENROL-1234")
        XCTAssertEqual(body["platform"] as? String, "iOS")
        XCTAssertEqual(body["app_version"] as? String, "1.0.0")
        XCTAssertNil(body["attest"])

        XCTAssertEqual(credential.deviceToken, "tok-abc")
        XCTAssertEqual(credential.orgId, "org-home")
        XCTAssertEqual(credential.deviceId, "dev-1")
        XCTAssertEqual(credential.configBundleVersion, "2026.10.1")
        XCTAssertEqual(credential.apiHost, "api.example.org")
        XCTAssertEqual(store.load(), credential)
    }

    func testBaseURLPathPrefixIsKept() async throws {
        let transport = StubSyncTransport(handler: Self.registered)
        _ = try await makeService(
            store: InMemoryDeviceCredentialStore(),
            transport: transport,
            baseURL: URL(string: "https://api.example.org/asd/")!
        ).register(.enrolCode("CODE"))
        XCTAssertEqual(transport.requests.first?.url?.absoluteString, "https://api.example.org/asd/v1/devices/register")
    }

    func testBlankEnrolCodeIsRejectedWithoutANetworkCall() async {
        let transport = StubSyncTransport(handler: Self.registered)
        do {
            _ = try await makeService(store: InMemoryDeviceCredentialStore(), transport: transport).register(.enrolCode("   "))
            XCTFail("expected an error")
        } catch {
            XCTAssertEqual(error as? DeviceRegistrationError, .emptyEnrolCode)
        }
        XCTAssertTrue(transport.requests.isEmpty)
    }

    func testRejectedEnrolCodeStoresNothing() async {
        let transport = StubSyncTransport { _ in (403, [:], jsonData(["detail": "invalid code"])) }
        let store = InMemoryDeviceCredentialStore()
        do {
            _ = try await makeService(store: store, transport: transport).register(.enrolCode("BAD"))
            XCTFail("expected an error")
        } catch {
            XCTAssertEqual(error as? DeviceRegistrationError, .rejected(status: 403))
        }
        XCTAssertNil(store.load())
    }

    func testResponseWithoutTokenIsInvalid() async {
        let transport = StubSyncTransport { _ in (200, [:], jsonData(["org_id": "org-home"])) }
        let store = InMemoryDeviceCredentialStore()
        do {
            _ = try await makeService(store: store, transport: transport).register(.enrolCode("CODE"))
            XCTFail("expected an error")
        } catch {
            XCTAssertEqual(error as? DeviceRegistrationError, .invalidResponse)
        }
        XCTAssertNil(store.load())
    }

    func testAppAttestIsBehindAFlag() async {
        let transport = StubSyncTransport(handler: Self.registered)
        do {
            _ = try await makeService(store: InMemoryDeviceCredentialStore(), transport: transport).register(.appAttest)
            XCTFail("expected an error")
        } catch {
            XCTAssertEqual(error as? DeviceRegistrationError, .appAttestUnavailable)
        }
        XCTAssertTrue(transport.requests.isEmpty)
        XCTAssertFalse(DeviceRegistrationService.appAttestEnabledByDefault)
    }

    /// A token issued by one API host is never presented to another (configuration change).
    func testCredentialIsBoundToTheAPIHost() {
        let foreign = DeviceCredential(
            deviceToken: "tok-other",
            orgId: "org-x",
            deviceId: nil,
            configBundleVersion: nil,
            apiHost: "other.example.net",
            enrolCode: "CODE",
            registeredAt: Date()
        )
        let service = makeService(store: InMemoryDeviceCredentialStore(foreign), transport: StubSyncTransport(handler: Self.registered))
        XCTAssertNil(service.currentCredential())
    }

    func testReRegisterUsesTheStoredEnrolCode() async throws {
        let transport = StubSyncTransport(handler: Self.registered)
        let store = InMemoryDeviceCredentialStore()
        let service = makeService(store: store, transport: transport)
        _ = try await service.register(.enrolCode("CODE-1"))

        let renewed = try await service.reRegister()
        XCTAssertEqual(transport.requests.count, 2)
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: transport.requests[1].httpBody ?? Data()) as? [String: Any])
        XCTAssertEqual(body["enrol_code"] as? String, "CODE-1")
        XCTAssertEqual(renewed.enrolCode, "CODE-1")
    }

    func testReRegisterRejectedClearsTheCredential() async throws {
        let store = InMemoryDeviceCredentialStore()
        let transport = StubSyncTransport(handler: Self.registered)
        let service = makeService(store: store, transport: transport)
        _ = try await service.register(.enrolCode("ONE-TIME"))

        transport.handler = { _ in (410, [:], Data()) }
        do {
            _ = try await service.reRegister()
            XCTFail("expected an error")
        } catch {
            XCTAssertEqual(error as? DeviceRegistrationError, .rejected(status: 410))
        }
        XCTAssertNil(store.load(), "a rejected re-registration leaves the device needing enrolment")
    }

    func testReRegisterOfflineKeepsTheCredential() async throws {
        let store = InMemoryDeviceCredentialStore()
        let transport = StubSyncTransport(handler: Self.registered)
        let service = makeService(store: store, transport: transport)
        _ = try await service.register(.enrolCode("CODE"))

        transport.handler = { _ in throw URLError(.notConnectedToInternet) }
        do {
            _ = try await service.reRegister()
            XCTFail("expected an error")
        } catch {
            XCTAssertTrue(error is URLError)
        }
        XCTAssertNotNil(store.load(), "a network failure is not a rejection")
    }

    func testReRegisterWithoutCredentialIsNotRegistered() async {
        let service = makeService(store: InMemoryDeviceCredentialStore(), transport: StubSyncTransport(handler: Self.registered))
        do {
            _ = try await service.reRegister()
            XCTFail("expected an error")
        } catch {
            XCTAssertEqual(error as? DeviceRegistrationError, .notRegistered)
        }
    }

    // MARK: - Keychain store

    func testKeychainStoreRoundTripsAndClears() throws {
        let store = KeychainDeviceCredentialStore(service: "org.curv.asd.device.tests.\(UUID().uuidString)")
        defer { store.clear() }
        let credential = DeviceCredential(
            deviceToken: "tok-keychain",
            orgId: "org-home",
            deviceId: "dev-9",
            configBundleVersion: "2026.10.1",
            apiHost: "api.example.org",
            enrolCode: "CODE",
            registeredAt: Date(timeIntervalSince1970: 1_793_000_000)
        )
        XCTAssertNil(store.load())
        do {
            try store.save(credential)
        } catch KeychainDeviceCredentialStore.StoreError.keychain(let status) where status == errSecMissingEntitlement {
            throw XCTSkip("Keychain unavailable in this test host (errSecMissingEntitlement)")
        }
        XCTAssertEqual(store.load(), credential)

        let rotated = DeviceCredential(
            deviceToken: "tok-rotated",
            orgId: "org-home",
            deviceId: "dev-9",
            configBundleVersion: nil,
            apiHost: "api.example.org",
            enrolCode: "CODE",
            registeredAt: Date(timeIntervalSince1970: 1_793_000_100)
        )
        try store.save(rotated)
        XCTAssertEqual(store.load(), rotated, "save replaces the existing item")

        store.clear()
        XCTAssertNil(store.load())
    }

    func testKeychainItemIsDeviceBoundAfterFirstUnlock() {
        XCTAssertEqual(
            KeychainDeviceCredentialStore.accessibility as String,
            kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String,
            "queued uploads flush while locked; the token never leaves this device in a backup"
        )
    }
}
