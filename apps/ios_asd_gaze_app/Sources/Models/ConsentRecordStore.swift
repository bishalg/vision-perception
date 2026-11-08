import Foundation
import os

// MARK: - Signed consent record (store-collection-build.md S-10)

/// Who agreed to the consent document.
public enum GuardianRelationship: String, CaseIterable, Codable, Sendable, Identifiable {
    case parent
    case legalGuardian
    case clinicStaff

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .parent: return ConsentL10n.text("consent.relationship.parent", "Parent")
        case .legalGuardian: return ConsentL10n.text("consent.relationship.guardian", "Legal guardian")
        case .clinicStaff: return ConsentL10n.text("consent.relationship.staff", "Clinic staff, with the guardian's permission")
        }
    }
}

/// What the guardian agreed to, and when. Stays on this device.
public struct ConsentRecord: Codable, Equatable, Sendable {
    public let documentVersion: String
    public let documentSha256: String
    public let agreedAt: Date
    public let guardianName: String
    public let relationship: GuardianRelationship
    public let appVersion: String

    public init(
        documentVersion: String,
        documentSha256: String,
        agreedAt: Date,
        guardianName: String,
        relationship: GuardianRelationship,
        appVersion: String
    ) {
        self.documentVersion = documentVersion
        self.documentSha256 = documentSha256
        self.agreedAt = agreedAt
        self.guardianName = guardianName
        self.relationship = relationship
        self.appVersion = appVersion
    }

    /// `true` when this record was signed for exactly `document` (version and text hash).
    public func matches(_ document: ConsentDocument) -> Bool {
        documentVersion == document.version && documentSha256 == document.sha256Hex
    }
}

/// Persists the signed `ConsentRecord` as JSON under Application Support, with
/// `.completeFileProtectionUntilFirstUserAuthentication` and excluded from backups
/// (same pattern as `GazeSessionHistoryStore`, plus the backup flag).
///
/// Migration from the legacy onboarding switch: installs that consented before WP-S.5 have
/// `ChildProfile.hasConsentedPIA == true` but no record here. For them `isCurrent` is `false`
/// on purpose, so the guardian goes through the full consent flow once (S-10: the old switch
/// did not capture name, relationship or document version). The legacy flag is still set by
/// onboarding so `ConsentToken` keeps working; capture is additionally gated on `isCurrent`
/// through `CaptureEntryGate`.
@MainActor
public final class ConsentRecordStore: ObservableObject {
    public static let shared = ConsentRecordStore()

    private static let logger = Logger(subsystem: "com.visionperception.asd", category: "ConsentRecordStore")

    @Published public private(set) var record: ConsentRecord?

    public let fileURL: URL
    private let document: ConsentDocument
    private let now: () -> Date

    public init(
        fileURL: URL? = nil,
        document: ConsentDocument = .current,
        now: @escaping () -> Date = Date.init
    ) {
        self.fileURL = fileURL ?? Self.defaultStorageURL()
        self.document = document
        self.now = now
        self.record = Self.load(from: self.fileURL)
    }

    /// The consent document this store checks records against.
    public var currentDocument: ConsentDocument { document }

    /// A record exists and was signed for the current document version and text.
    public var isCurrent: Bool {
        record?.matches(document) ?? false
    }

    /// A record exists but was signed for an older document: ask again before the next capture.
    public var needsReconsent: Bool {
        record != nil && !isCurrent
    }

    /// Records agreement to the current document. The name is trimmed; an empty name is refused.
    @discardableResult
    public func agree(guardianName: String, relationship: GuardianRelationship) -> ConsentRecord? {
        let name = guardianName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return nil }
        let newRecord = ConsentRecord(
            documentVersion: document.version,
            documentSha256: document.sha256Hex,
            agreedAt: now(),
            guardianName: name,
            relationship: relationship,
            appVersion: Self.appVersion()
        )
        record = newRecord
        persist(newRecord)
        return newRecord
    }

    /// Withdraws consent: removes the record. Capture asks for consent again.
    public func revoke() {
        record = nil
        do {
            if FileManager.default.fileExists(atPath: fileURL.path) {
                try FileManager.default.removeItem(at: fileURL)
            }
        } catch {
            Self.logger.error("Failed to remove consent record: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: Storage

    private static func defaultStorageURL() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return base
            .appendingPathComponent("org.curv.asd", isDirectory: true)
            .appendingPathComponent("consent_record.json")
    }

    private static func load(from url: URL) -> ConsentRecord? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(ConsentRecord.self, from: data)
    }

    private func persist(_ record: ConsentRecord) {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        guard let data = try? encoder.encode(record) else { return }
        do {
            try FileManager.default.createDirectory(
                at: fileURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try data.write(to: fileURL, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
            var url = fileURL
            var values = URLResourceValues()
            values.isExcludedFromBackup = true
            try url.setResourceValues(values)
        } catch {
            Self.logger.error("Failed to persist consent record: \(error.localizedDescription, privacy: .public)")
        }
    }

    private static func appVersion() -> String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "0"
        let build = info?["CFBundleVersion"] as? String ?? "0"
        return "\(short) (\(build))"
    }
}
