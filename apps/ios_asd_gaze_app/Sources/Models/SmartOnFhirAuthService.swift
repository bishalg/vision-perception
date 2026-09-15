import Foundation
import CryptoKit
import AuthenticationServices

/// SMART on FHIR OAuth 2.0 Authorization Code Flow with PKCE (RFC 7636).
/// Enables secure EHR launch (Epic, Cerner) and standalone patient portal login.
///
/// Security: Native iOS apps cannot store client secrets. PKCE generates a
/// one-time code verifier that proves possession of the authorization code.
///
/// Reference: DOC-RES-ASD-006 §2.2, §7 (STAGE 3 Implementation Plan)
@MainActor
public final class SmartOnFhirAuthService: ObservableObject {
    public static let shared = SmartOnFhirAuthService()

    // MARK: - Published State

    @Published public private(set) var authState: AuthState = .idle
    @Published public private(set) var accessToken: String?
    @Published public private(set) var refreshToken: String?
    @Published public private(set) var idToken: String?
    @Published public private(set) var patientId: String?
    @Published public private(set) var fhirBaseURL: URL?
    @Published public private(set) var lastError: String?

    // MARK: - Configuration

    private let keychain = KeychainWrapper.standard
    private let pkceStorageKey = "org.curv.asd.smart.pkce"
    private let tokenStorageKey = "org.curv.asd.smart.tokens"
    private let configStorageKey = "org.curv.asd.smart.config"

    private init() {
        restoreTokens()
    }

    // MARK: - SMART Configuration Discovery

    /// Discovers the SMART on FHIR endpoints from the EHR's well-known configuration.
    /// Must be called before `authorize()` for EHR-launch workflows.
    public func discoverConfiguration(baseURL: URL) async throws -> SmartConfiguration {
        let configURL = baseURL.appendingPathComponent(".well-known/smart-configuration")
        let (data, _) = try await URLSession.shared.data(from: configURL)
        let config = try JSONDecoder().decode(SmartConfiguration.self, from: data)
        self.fhirBaseURL = baseURL
        try saveConfiguration(config)
        return config
    }

    // MARK: - Authorization (EHR Launch / Standalone Patient Launch)

    /// Initiates the OAuth 2.0 Authorization Code Flow with PKCE.
    ///
    /// - Parameters:
    ///   - config: Discovered SMART configuration (or manually constructed).
    ///   - clientId: Public client ID registered with the EHR.
    ///   - redirectURI: Custom URL scheme (e.g., `asd-gaze-app://oauth-callback`).
    ///   - launchToken: EHR `launch` context parameter (EHR launch only; nil for patient launch).
    ///   - scopes: Requested SMART scopes (see `SmartConfiguration.requiredScopes`).
    public func authorize(
        config: SmartConfiguration,
        clientId: String,
        redirectURI: String,
        launchToken: String? = nil,
        scopes: [String] = SmartConfiguration.requiredScopes
    ) async throws {
        guard authState != .authorizing else { return }
        authState = .authorizing

        // 1. Generate PKCE code verifier & challenge (RFC 7636)
        let codeVerifier = generateCodeVerifier()
        let codeChallenge = generateCodeChallenge(from: codeVerifier)
        savePKCE(codeVerifier: codeVerifier)

        // 2. Build authorization URL
        var components = URLComponents(url: config.authorizationEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "client_id", value: clientId),
            URLQueryItem(name: "redirect_uri", value: redirectURI),
            URLQueryItem(name: "scope", value: scopes.joined(separator: " ")),
            URLQueryItem(name: "code_challenge", value: codeChallenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
            URLQueryItem(name: "aud", value: config.fhirBaseURL.absoluteString)
        ]
        if let launchToken = launchToken {
            components.queryItems?.append(URLQueryItem(name: "launch", value: launchToken))
        }
        if let state = UUID().uuidString.split(separator: "-").first.map(String.init) {
            components.queryItems?.append(URLQueryItem(name: "state", value: state))
        }

        guard let authURL = components.url else {
            throw SmartAuthError.invalidAuthorizationURL
        }

        // 3. Present ASWebAuthenticationSession (system browser, shared cookies)
        let session = ASWebAuthenticationSession(url: authURL, callbackURLScheme: URL(string: redirectURI)?.scheme) { [weak self] callbackURL, error in
            Task { @MainActor [weak self] in
                guard let self = self else { return }
                if let error = error {
                    self.authState = .failed(error.localizedDescription)
                    return
                }
                guard let callbackURL = callbackURL else {
                    self.authState = .failed("No callback URL received")
                    return
                }
                do {
                    try await self.handleCallback(callbackURL, config: config, clientId: clientId, redirectURI: redirectURI)
                } catch {
                    self.authState = .failed(error.localizedDescription)
                }
            }
        }
        session.presentationContextProvider = self
        session.prefersEphemeralWebBrowserSession = false
        if !session.start() {
            throw SmartAuthError.sessionStartFailed
        }
    }

    // MARK: - Token Exchange

    private func handleCallback(
        _ url: URL,
        config: SmartConfiguration,
        clientId: String,
        redirectURI: String
    ) async throws {
        guard let codeVerifier = loadPKCE() else {
            throw SmartAuthError.missingCodeVerifier
        }

        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let code = components.queryItems?.first(where: { $0.name == "code" })?.value else {
            throw SmartAuthError.missingAuthorizationCode
        }

        var request = URLRequest(url: config.tokenEndpoint)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        let body = [
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirectURI,
            "client_id": clientId,
            "code_verifier": codeVerifier
        ].map { "\($0.key)=\($0.value)" }.joined(separator: "&")
        request.httpBody = body.data(using: .utf8)

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            throw SmartAuthError.tokenExchangeFailed
        }

        let tokenResponse = try JSONDecoder().decode(TokenResponse.self, from: data)
        try saveTokens(tokenResponse)

        // Decode id_token for patient context
        if let idToken = tokenResponse.idToken,
           let payload = decodeJWT(idToken),
           let patient = payload["patient"] as? String {
            self.patientId = patient
        }

        clearPKCE()
        authState = .authenticated
    }

    // MARK: - Token Refresh

    /// Refreshes the access token using the stored refresh token.
    public func refreshAccessToken(config: SmartConfiguration, clientId: String) async throws {
        guard let refreshToken = refreshToken else {
            throw SmartAuthError.noRefreshToken
        }
        var request = URLRequest(url: config.tokenEndpoint)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        let body = [
            "grant_type": "refresh_token",
            "refresh_token": refreshToken,
            "client_id": clientId
        ].map { "\($0.key)=\($0.value)" }.joined(separator: "&")
        request.httpBody = body.data(using: .utf8)

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200 else {
            // Refresh token expired → require re-auth
            await logout()
            throw SmartAuthError.tokenRefreshFailed
        }
        let tokenResponse = try JSONDecoder().decode(TokenResponse.self, from: data)
        try saveTokens(tokenResponse)
    }

    // MARK: - Logout & Cleanup

    /// Clears all tokens and revokes the session.
    public func logout() async {
        // Best-effort revocation (if EHR supports it)
        if let config = loadConfiguration(),
           let revokeEndpoint = config.revocationEndpoint,
           let accessToken = accessToken {
            var req = URLRequest(url: revokeEndpoint)
            req.httpMethod = "POST"
            req.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
            req.httpBody = "token=\(accessToken)&token_type_hint=access_token".data(using: .utf8)
            _ = try? await URLSession.shared.data(for: req)
        }
        clearAll()
        authState = .idle
    }

    // MARK: - FHIR Request Helper

    /// Adds Bearer Authorization header to any FHIR request.
    public func authorizeRequest(_ request: inout URLRequest) {
        if let token = accessToken {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
    }

    // MARK: - PKCE Generation (RFC 7636)

    private func generateCodeVerifier() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64URLEncodedString()
    }

    private func generateCodeChallenge(from verifier: String) -> String {
        let data = Data(verifier.utf8)
        let hash = SHA256.hash(data: data)
        return Data(hash).base64URLEncodedString()
    }

    // MARK: - Secure Keychain Storage

    private func savePKCE(codeVerifier: String) {
        keychain[codeVerifier] = pkceStorageKey
    }

    private func loadPKCE() -> String? {
        keychain[pkceStorageKey]
    }

    private func clearPKCE() {
        keychain.removeObject(forKey: pkceStorageKey)
    }

    private struct TokenBundle: Codable {
        let accessToken: String
        let refreshToken: String?
        let idToken: String?
        let expiresAt: Date
    }

    private func saveTokens(_ response: TokenResponse) throws {
        let bundle = TokenBundle(
            accessToken: response.accessToken,
            refreshToken: response.refreshToken,
            idToken: response.idToken,
            expiresAt: Date().addingTimeInterval(TimeInterval(response.expiresIn))
        )
        let data = try JSONEncoder().encode(bundle)
        keychain[data] = tokenStorageKey
        self.accessToken = response.accessToken
        self.refreshToken = response.refreshToken
        self.idToken = response.idToken
    }

    private func restoreTokens() {
        guard let data = keychain.data(forKey: tokenStorageKey),
              let bundle = try? JSONDecoder().decode(TokenBundle.self, from: data),
              bundle.expiresAt > Date() else {
            clearAll()
            return
        }
        self.accessToken = bundle.accessToken
        self.refreshToken = bundle.refreshToken
        self.idToken = bundle.idToken
        self.authState = .authenticated
    }

    private func clearAll() {
        keychain.removeObject(forKey: tokenStorageKey)
        keychain.removeObject(forKey: pkceStorageKey)
        self.accessToken = nil
        self.refreshToken = nil
        self.idToken = nil
        self.patientId = nil
    }

    private func loadConfiguration() -> SmartConfiguration? {
        guard let data = keychain.data(forKey: configStorageKey),
              let config = try? JSONDecoder().decode(SmartConfiguration.self, from: data) else {
            return nil
        }
        return config
    }

    private func saveConfiguration(_ config: SmartConfiguration) throws {
        let data = try JSONEncoder().encode(config)
        keychain[data] = configStorageKey
    }

    // MARK: - JWT Decode (id_token)

    private func decodeJWT(_ token: String) -> [String: Any]? {
        let segments = token.split(separator: ".")
        guard segments.count == 3,
              let payloadData = base64URLDecode(String(segments[1])) else { return nil }
        return try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
    }

    private func base64URLDecode(_ string: String) -> Data? {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = 4 - base64.count % 4
        if padding < 4 { base64 += String(repeating: "=", count: padding) }
        return Data(base64Encoded: base64)
    }
}

// MARK: - Supporting Types

public enum AuthState: Equatable {
    case idle
    case authorizing
    case authenticated
    case failed(String)
}

public struct SmartConfiguration: Codable, Sendable {
    public let authorizationEndpoint: URL
    public let tokenEndpoint: URL
    public let revocationEndpoint: URL?
    public let fhirBaseURL: URL
    public let scopesSupported: [String]
    public let capabilities: [String]

    public static let requiredScopes = [
        "openid",
        "profile",
        "launch/patient",
        "patient/Patient.read",
        "patient/Observation.read",
        "patient/Observation.write",
        "patient/QuestionnaireResponse.write",
        "patient/DocumentReference.write",
        "offline_access"
    ]
}

public struct TokenResponse: Codable, Sendable {
    public let accessToken: String
    public let tokenType: String
    public let expiresIn: Int
    public let refreshToken: String?
    public let idToken: String?
    public let scope: String?

    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case tokenType = "token_type"
        case expiresIn = "expires_in"
        case refreshToken = "refresh_token"
        case idToken = "id_token"
        case scope
    }
}

public enum SmartAuthError: LocalizedError {
    case invalidAuthorizationURL
    case sessionStartFailed
    case missingCodeVerifier
    case missingAuthorizationCode
    case tokenExchangeFailed
    case noRefreshToken
    case tokenRefreshFailed

    public var errorDescription: String? {
        switch self {
        case .invalidAuthorizationURL: return "Invalid SMART authorization URL"
        case .sessionStartFailed: return "Failed to start authentication session"
        case .missingCodeVerifier: return "PKCE code verifier not found"
        case .missingAuthorizationCode: return "Authorization code not in callback"
        case .tokenExchangeFailed: return "Token exchange failed"
        case .noRefreshToken: return "No refresh token available"
        case .tokenRefreshFailed: return "Token refresh failed; please re-authenticate"
        }
    }
}

// MARK: - Keychain Wrapper (minimal)

private final class KeychainWrapper {
    static let standard = KeychainWrapper()

    subscript(_ key: String) -> String? {
        get {
            var query = baseQuery(key)
            query[kSecReturnData as String] = kCFBooleanTrue
            query[kSecMatchLimit as String] = kSecMatchLimitOne
            var item: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &item)
            guard status == errSecSuccess, let data = item as? Data else { return nil }
            return String(data: data, encoding: .utf8)
        }
        set {
            if let value = newValue, let data = value.data(using: .utf8) {
                setData(data, for: key)
            } else {
                removeObject(forKey: key)
            }
        }
    }

    subscript(_ key: String) -> Data? {
        get {
            var query = baseQuery(key)
            query[kSecReturnData as String] = kCFBooleanTrue
            query[kSecMatchLimit as String] = kSecMatchLimitOne
            var item: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &item)
            guard status == errSecSuccess, let data = item as? Data else { return nil }
            return data
        }
        set {
            if let data = newValue { setData(data, for: key) } else { removeObject(forKey: key) }
        }
    }

    func data(forKey key: String) -> Data? { self[key] }

    private func baseQuery(_ key: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: key,
            kSecAttrService as String: "org.curv.asd.smart"
        ]
    }

    private func setData(_ data: Data, for key: String) {
        var query = baseQuery(key)
        SecItemDelete(query as CFDictionary)
        query[kSecValueData as String] = data
        query[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        SecItemAdd(query as CFDictionary, nil)
    }

    func removeObject(forKey key: String) {
        SecItemDelete(baseQuery(key) as CFDictionary)
    }
}

// MARK: - Base64 URL Encoding (RFC 4648 §5)

private extension Data {
    func base64URLEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

private extension String {
    func base64URLDecoded() -> Data? {
        var base64 = self
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = 4 - count % 4
        if padding < 4 { base64 += String(repeating: "=", count: padding) }
        return Data(base64Encoded: base64)
    }
}

// MARK: - ASWebAuthenticationPresentationContextProviding

extension SmartOnFhirAuthService: ASWebAuthenticationPresentationContextProviding {
    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        ASPresentationAnchor()
    }
}