import Foundation
import os

public enum DeviceRegistrationError: Error, Equatable, Sendable {
    case emptyEnrolCode
    /// App Attest registration is not built yet; it sits behind a flag (WP-A.4).
    case appAttestUnavailable
    /// No credential, or none with a stored enrol code to re-register with.
    case notRegistered
    /// The server refused the registration (bad, used or revoked code).
    case rejected(status: Int)
    case invalidResponse
    case insecureURL
    case storageFailed
}

public enum DeviceRegistrationMethod: Sendable, Equatable {
    /// One-time code issued by the clinic console (Phase 1 path).
    case enrolCode(String)
    /// Apple App Attest assertion (later phase, off by default).
    case appAttest
}

/// Registers this device with the backend (`POST /v1/devices/register`, `02 §4`)
/// and keeps the returned device token in the Keychain (ADR-B4).
///
/// The token is opaque; it is only ever sent as `Authorization: Bearer` to the
/// host that issued it.
public struct DeviceRegistrationService: Sendable {
    /// App Attest stays off until it is implemented and reviewed.
    public static let appAttestEnabledByDefault = false
    static let registerPath = "v1/devices/register"

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "DeviceRegistrationService")

    public let apiBaseURL: URL
    private let transport: SyncTransport
    private let store: DeviceCredentialStore
    private let clientInfo: SyncClientInfo
    private let appAttestEnabled: Bool
    private let now: @Sendable () -> Date

    public init(
        apiBaseURL: URL,
        transport: SyncTransport,
        store: DeviceCredentialStore,
        clientInfo: SyncClientInfo,
        appAttestEnabled: Bool = DeviceRegistrationService.appAttestEnabledByDefault,
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.apiBaseURL = apiBaseURL
        self.transport = transport
        self.store = store
        self.clientInfo = clientInfo
        self.appAttestEnabled = appAttestEnabled
        self.now = now
    }

    private var apiHost: String { apiBaseURL.host?.lowercased() ?? "" }

    /// The stored credential, only when it was issued by the configured host.
    public func currentCredential() -> DeviceCredential? {
        guard let credential = store.load(), credential.apiHost == apiHost, !credential.deviceToken.isEmpty else {
            return nil
        }
        return credential
    }

    @discardableResult
    public func register(_ method: DeviceRegistrationMethod) async throws -> DeviceCredential {
        let code: String
        switch method {
        case .appAttest:
            // The flag gates the future App Attest path. The attestation itself
            // (DCAppAttestService key + assertion) is not built yet, so even with
            // the flag on this refuses rather than send an unattested request.
            _ = appAttestEnabled
            throw DeviceRegistrationError.appAttestUnavailable
        case .enrolCode(let raw):
            code = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !code.isEmpty else { throw DeviceRegistrationError.emptyEnrolCode }
        }

        let url = apiBaseURL.appendingPathComponent(Self.registerPath)
        do {
            try ResearchStudyConfiguration.validateIngestURL(url)
        } catch {
            throw DeviceRegistrationError.insecureURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 15
        request.setValue("application/json", forHTTPHeaderField: SyncHeader.contentType)
        request.setValue("application/json", forHTTPHeaderField: SyncHeader.accept)
        request.setValue(clientInfo.userAgent, forHTTPHeaderField: SyncHeader.userAgent)
        // Non-session calls use a fresh UUID (ADR-B5).
        request.setValue(UUID().uuidString, forHTTPHeaderField: SyncHeader.idempotencyKey)
        request.httpBody = try JSONSerialization.data(withJSONObject: [
            "enrol_code": code,
            "platform": "iOS",
            "app_version": clientInfo.appVersion
        ], options: [.sortedKeys])

        let (data, response) = try await transport.send(request)
        guard (200...299).contains(response.statusCode) else {
            Self.logger.warning("Device registration refused (HTTP \(response.statusCode, privacy: .public))")
            throw DeviceRegistrationError.rejected(status: response.statusCode)
        }
        guard let body = try? JSONDecoder().decode(RegisterResponse.self, from: data),
              !body.deviceToken.isEmpty, !body.orgId.isEmpty else {
            throw DeviceRegistrationError.invalidResponse
        }

        let credential = DeviceCredential(
            deviceToken: body.deviceToken,
            orgId: body.orgId,
            deviceId: body.deviceId,
            configBundleVersion: body.configBundleVersion,
            apiHost: apiHost,
            enrolCode: code,
            registeredAt: now()
        )
        do {
            try store.save(credential)
        } catch {
            throw DeviceRegistrationError.storageFailed
        }
        Self.logger.info("Device registered with org \(body.orgId, privacy: .public)")
        return credential
    }

    /// One re-registration with the stored enrol code after a 401 (WP-A.4).
    /// A refusal clears the credential so the device shows as needing enrolment;
    /// a transport failure keeps it for the next attempt.
    @discardableResult
    public func reRegister() async throws -> DeviceCredential {
        guard let code = currentCredential()?.enrolCode else {
            store.clear()
            throw DeviceRegistrationError.notRegistered
        }
        do {
            return try await register(.enrolCode(code))
        } catch let error as DeviceRegistrationError {
            store.clear()
            throw error
        }
    }

    public func clear() {
        store.clear()
    }

    private struct RegisterResponse: Decodable {
        let deviceToken: String
        let orgId: String
        let deviceId: String?
        let configBundleVersion: String?

        enum CodingKeys: String, CodingKey {
            case deviceToken = "device_token"
            case orgId = "org_id"
            case deviceId = "device_id"
            case configBundleVersion = "config_bundle_version"
        }
    }
}
