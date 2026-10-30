import Darwin
import Foundation
import os

/// The research study this build belongs to, read from the `ASDResearchStudy`
/// Info.plist dictionary (CNS-9, ADR-0002 decision 5).
///
/// There is no study at launch (H7), so the default build has no such key and
/// `current` is `nil`. With `nil`, the research card, the consent flow, the
/// Settings research rows and every upload path are absent. A configuration
/// can only be created through validation: all six fields present and non-blank,
/// and an `https` ingest URL whose host is a DNS name (IP literals bypass ATS,
/// so the scheme check never relies on ATS; trap 10).
///
/// Expected Info.plist shape:
/// ```
/// <key>ASDResearchStudy</key>
/// <dict>
///     <key>ingestURL</key>             <string>https://...</string>
///     <key>studyIdentifier</key>       <string>...</string>
///     <key>irbProtocolId</key>         <string>...</string>
///     <key>institution</key>           <string>...</string>
///     <key>principalInvestigator</key> <string>...</string>
///     <key>consentVersion</key>        <string>...</string>
///     <!-- optional, WP-A.4: base of the /v1 device-plane API -->
///     <key>apiBaseURL</key>            <string>https://...</string>
/// </dict>
/// ```
///
/// `apiBaseURL` is optional. When present it obeys the same https/DNS rule as
/// `ingestURL`, and an invalid value turns the whole configuration off (fail
/// closed). When absent the `/v1/sessions` client stays off.
public struct ResearchStudyConfiguration: Sendable, Equatable {
    public static let infoPlistKey = "ASDResearchStudy"

    /// Placed in study-shaped records (task results, export packages) created
    /// while no study is configured. Never a real study id.
    public static let unconfiguredStudyIdentifier = "none"

    /// Optional Info.plist key for the `/v1` API base (WP-A.4). Not a `Field`:
    /// every `Field` is required.
    public static let apiBaseURLKey = "apiBaseURL"

    public enum Field: String, CaseIterable, Sendable {
        case ingestURL
        case studyIdentifier
        case irbProtocolId
        case institution
        case principalInvestigator
        case consentVersion
    }

    public enum ValidationError: Error, Equatable, Sendable {
        case missingField(String)
        case invalidURL(String)
        case insecureScheme(String?)
        case missingHost
        case ipLiteralHost(String)
        case loopbackHost(String)
        case credentialsInURL
    }

    public let ingestURL: URL
    public let studyIdentifier: String
    public let irbProtocolId: String
    public let institution: String
    public let principalInvestigator: String
    public let consentVersion: String
    /// Base of the device-plane API (`/v1/devices/register`, `/v1/sessions`), or
    /// `nil` when this build does not upload sessions (WP-A.4).
    public let apiBaseURL: URL?

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ResearchStudyConfiguration")

    public init(
        ingestURL: URL,
        studyIdentifier: String,
        irbProtocolId: String,
        institution: String,
        principalInvestigator: String,
        consentVersion: String,
        apiBaseURL: URL? = nil
    ) throws {
        try Self.validateIngestURL(ingestURL)
        if let apiBaseURL {
            try Self.validateIngestURL(apiBaseURL)
        }
        self.ingestURL = ingestURL
        self.apiBaseURL = apiBaseURL
        self.studyIdentifier = try Self.required(studyIdentifier, .studyIdentifier)
        self.irbProtocolId = try Self.required(irbProtocolId, .irbProtocolId)
        self.institution = try Self.required(institution, .institution)
        self.principalInvestigator = try Self.required(principalInvestigator, .principalInvestigator)
        self.consentVersion = try Self.required(consentVersion, .consentVersion)
    }

    /// Parses the contents of the `ASDResearchStudy` dictionary.
    public init(dictionary: [String: Any]) throws {
        func string(_ field: Field) throws -> String {
            guard let value = dictionary[field.rawValue] as? String else {
                throw ValidationError.missingField(field.rawValue)
            }
            return try Self.required(value, field)
        }
        let rawURL = try string(.ingestURL)
        guard let url = URL(string: rawURL) else {
            throw ValidationError.invalidURL(rawURL)
        }
        var apiBaseURL: URL?
        if let rawAPI = dictionary[Self.apiBaseURLKey] {
            guard let string = rawAPI as? String else {
                throw ValidationError.invalidURL(String(describing: rawAPI))
            }
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty, let parsed = URL(string: trimmed) else {
                throw ValidationError.invalidURL(string)
            }
            apiBaseURL = parsed
        }
        try self.init(
            ingestURL: url,
            studyIdentifier: try string(.studyIdentifier),
            irbProtocolId: try string(.irbProtocolId),
            institution: try string(.institution),
            principalInvestigator: try string(.principalInvestigator),
            consentVersion: try string(.consentVersion),
            apiBaseURL: apiBaseURL
        )
    }

    // MARK: - Loading

    /// The configuration compiled into this build, or `nil` (the default).
    public static let current: ResearchStudyConfiguration? = load(bundle: .main)

    /// Whether any research surface may exist in this build.
    public static var isResearchEnabled: Bool { current != nil }

    public static func load(bundle: Bundle) -> ResearchStudyConfiguration? {
        load(from: bundle.infoDictionary)
    }

    /// `nil` when the key is absent or the configuration is invalid; an invalid
    /// configuration is logged so a misconfigured study build is diagnosable.
    public static func load(from infoDictionary: [String: Any]?) -> ResearchStudyConfiguration? {
        guard let raw = infoDictionary?[infoPlistKey] else { return nil }
        guard let dictionary = raw as? [String: Any] else {
            logger.error("\(infoPlistKey, privacy: .public) is not a dictionary; research features stay off")
            return nil
        }
        do {
            return try ResearchStudyConfiguration(dictionary: dictionary)
        } catch {
            logger.error("Invalid \(infoPlistKey, privacy: .public) (\(String(describing: error), privacy: .public)); research features stay off")
            return nil
        }
    }

    // MARK: - Validation

    /// Accepts only `https` URLs with a DNS host name: no IP literals (v4, v6 or
    /// numeric shorthand), no loopback names, no embedded credentials.
    public static func validateIngestURL(_ url: URL) throws {
        let scheme = url.scheme?.lowercased()
        guard scheme == "https" else {
            throw ValidationError.insecureScheme(scheme)
        }
        guard url.user == nil, url.password == nil else {
            throw ValidationError.credentialsInURL
        }
        guard var host = url.host?.lowercased(), !host.isEmpty else {
            throw ValidationError.missingHost
        }
        if host.hasPrefix("["), host.hasSuffix("]") {
            host = String(host.dropFirst().dropLast())
        }
        if isIPLiteral(host) {
            throw ValidationError.ipLiteralHost(host)
        }
        if host == "localhost" || host.hasSuffix(".localhost") {
            throw ValidationError.loopbackHost(host)
        }
    }

    /// True for dotted IPv4, IPv6, and the numeric shorthands URL loaders accept
    /// (`127.1`, `2130706433`, `0x7f.0.0.1`). A real DNS name never ends in a
    /// numeric label, because top-level domains are alphabetic.
    static func isIPLiteral(_ host: String) -> Bool {
        var v4 = in_addr()
        var v6 = in6_addr()
        if inet_pton(AF_INET, host, &v4) == 1 || inet_pton(AF_INET6, host, &v6) == 1 {
            return true
        }
        if host.contains(":") { return true }
        let trimmed = host.hasSuffix(".") ? String(host.dropLast()) : host
        guard let last = trimmed.split(separator: ".").last else { return false }
        let label = last.lowercased()
        if label.allSatisfy(\.isNumber) { return true }
        if label.hasPrefix("0x"), label.dropFirst(2).allSatisfy(\.isHexDigit) { return true }
        return false
    }

    private static func required(_ value: String, _ field: Field) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw ValidationError.missingField(field.rawValue) }
        return trimmed
    }
}
