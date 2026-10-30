import Foundation
import os
import Security

/// What the backend returned for this device at registration (`02 §2 ADR-B4`).
///
/// `apiHost` binds the token to the host that issued it, so a configuration
/// change never presents one server's token to another. `enrolCode` is kept
/// only so a 401 can re-register once (WP-A.4); it lives in the Keychain with
/// the token, never in defaults or logs.
public struct DeviceCredential: Codable, Equatable, Sendable {
    public let deviceToken: String
    public let orgId: String
    public let deviceId: String?
    public let configBundleVersion: String?
    public let apiHost: String
    public let enrolCode: String?
    public let registeredAt: Date

    public init(
        deviceToken: String,
        orgId: String,
        deviceId: String?,
        configBundleVersion: String?,
        apiHost: String,
        enrolCode: String?,
        registeredAt: Date
    ) {
        self.deviceToken = deviceToken
        self.orgId = orgId
        self.deviceId = deviceId
        self.configBundleVersion = configBundleVersion
        self.apiHost = apiHost
        self.enrolCode = enrolCode
        self.registeredAt = registeredAt
    }
}

public protocol DeviceCredentialStore: Sendable {
    func load() -> DeviceCredential?
    func save(_ credential: DeviceCredential) throws
    func clear()
}

/// Keychain-backed credential store: one generic-password item holding the
/// JSON-encoded `DeviceCredential`.
///
/// Accessible after first unlock (queued uploads flush on reconnect while the
/// device is locked, CNS-4) and `ThisDeviceOnly`, so the token is not restored
/// to another device from a backup.
public struct KeychainDeviceCredentialStore: DeviceCredentialStore {
    public enum StoreError: Error, Equatable {
        case encoding
        case keychain(OSStatus)
    }

    public static let defaultService = "org.curv.asd.device"
    public static let defaultAccount = "device-credential-v1"
    static var accessibility: CFString { kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly }

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "DeviceCredentialStore")

    public let service: String
    public let account: String

    public init(service: String = KeychainDeviceCredentialStore.defaultService,
                account: String = KeychainDeviceCredentialStore.defaultAccount) {
        self.service = service
        self.account = account
    }

    private var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
    }

    public func load() -> DeviceCredential? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess, let data = item as? Data else {
            if status != errSecItemNotFound {
                Self.logger.error("Device credential read failed (status \(status, privacy: .public))")
            }
            return nil
        }
        return try? JSONDecoder().decode(DeviceCredential.self, from: data)
    }

    public func save(_ credential: DeviceCredential) throws {
        guard let data = try? JSONEncoder().encode(credential) else { throw StoreError.encoding }
        SecItemDelete(baseQuery as CFDictionary)
        var add = baseQuery
        add[kSecValueData as String] = data
        add[kSecAttrAccessible as String] = Self.accessibility
        let status = SecItemAdd(add as CFDictionary, nil)
        guard status == errSecSuccess else {
            Self.logger.error("Device credential write failed (status \(status, privacy: .public))")
            throw StoreError.keychain(status)
        }
    }

    public func clear() {
        SecItemDelete(baseQuery as CFDictionary)
    }
}
