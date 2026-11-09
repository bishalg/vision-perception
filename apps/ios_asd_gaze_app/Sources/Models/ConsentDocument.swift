import CryptoKit
import Foundation

// MARK: - Guardian consent document (store-collection-build.md S-10, S-13)

/// Copy lookup for the consent and privacy surfaces (WP-S.5).
///
/// Every user-facing string in the consent flow goes through `ConsentL10n.text(_:_:)` with a
/// stable key and its English text. Today it returns the English text; when the copy is
/// translated, the keys move into `Localizer.strings` and this function routes through
/// `Localizer.tr`. The consent document hash (`ConsentDocument.sha256Hex`) is taken over the
/// text the guardian actually read, so a translation is a new document version.
public enum ConsentL10n {
    public static func text(_ key: String, _ english: String) -> String {
        _ = key
        return english
    }
}

/// One page of the visual consent flow: icon, title, a short summary and the long
/// "Learn more" text. Shaped like ResearchKit's `ORKConsentSection` (summary + content).
public struct ConsentSection: Identifiable, Equatable, Sendable {
    public let id: String
    public let systemImage: String
    public let title: String
    public let summary: String
    public let learnMore: String

    public init(id: String, systemImage: String, title: String, summary: String, learnMore: String) {
        self.id = id
        self.systemImage = systemImage
        self.title = title
        self.summary = summary
        self.learnMore = learnMore
    }
}

/// The guardian consent document for observation capture (S-10). It is not a research
/// consent: no study, no ethics approval, no investigator (guideline 5.1.3 framing).
///
/// Bump `version` whenever any section text changes. `ConsentRecordStore.isCurrent` compares
/// both the version and the SHA-256 of `fullText`, so a changed text without a version bump
/// still asks the guardian again before the next capture.
public struct ConsentDocument: Equatable, Sendable {
    public let version: String
    public let sections: [ConsentSection]

    public init(version: String, sections: [ConsentSection]) {
        self.version = version
        self.sections = sections
    }

    public static let currentVersion = "2026-11-08.2"

    public static let current = ConsentDocument(version: currentVersion, sections: standardSections)

    /// The full text the guardian reviews before agreeing (and that is hashed).
    public var fullText: String {
        var parts = [ConsentL10n.text("consent.doc.header", "EarlyGaze guardian consent") + " (" + version + ")"]
        for section in sections {
            parts.append(section.title + "\n" + section.summary + "\n" + section.learnMore)
        }
        return parts.joined(separator: "\n\n")
    }

    /// Lower-case hex SHA-256 of `fullText` (UTF-8).
    public var sha256Hex: String {
        SHA256.hash(data: Data(fullText.utf8)).map { String(format: "%02x", locale: FormatLocale.posix, $0) }.joined()
    }

    /// The single sentence about interpretation (S-13). Shared with the pre-session screen.
    public static let interpretationSentence = ConsentL10n.text(
        "consent.interpretation",
        "Only a qualified professional, using the clinic's analysis tools, can interpret an observation."
    )

    // MARK: Sections (plain language, about grade 6 reading level)

    static let standardSections: [ConsentSection] = [
        ConsentSection(
            id: "overview",
            systemImage: "eye",
            title: ConsentL10n.text("consent.overview.title", "Overview"),
            summary: ConsentL10n.text("consent.overview.summary", "EarlyGaze records a short observation of where your child looks while they watch short clips, for your clinic to review."),
            learnMore: [
                ConsentL10n.text("consent.overview.more.1", "Your child sits in front of this iPhone or iPad and watches a few short clips."),
                ConsentL10n.text("consent.overview.more.2", "The front camera follows where on the screen your child looks."),
                ConsentL10n.text("consent.overview.more.3", "The app saves these measurements as one session."),
                ConsentL10n.text("consent.overview.more.4", "The app does not show you a result. Your clinic looks at each session."),
                ClaimsCopy.notDiagnostic,
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "recorded",
            systemImage: "list.bullet.rectangle",
            title: ConsentL10n.text("consent.recorded.title", "What is recorded"),
            summary: ConsentL10n.text("consent.recorded.summary", "Where your child looks on the screen, and a few measurements about the session."),
            learnMore: [
                ConsentL10n.text("consent.recorded.more.1", "Gaze points: the spots on the screen your child looks at, many times each second."),
                ConsentL10n.text("consent.recorded.more.2", "Head position and distance from the screen."),
                ConsentL10n.text("consent.recorded.more.3", "Facial-expression measurements, for example how much your child smiles."),
                ConsentL10n.text("consent.recorded.more.4", "The time the session started and how long it lasted."),
                ConsentL10n.text("consent.recorded.more.5", "The age bracket you enter for your child."),
                ConsentL10n.text("consent.recorded.more.6", "Summary measures worked out from the looking data, such as how much of the time was spent looking at people rather than shapes, and a note on how usable the recording was. The app does not show or interpret these; your clinic reviews them."),
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "neverRecorded",
            systemImage: "video.slash",
            title: ConsentL10n.text("consent.never.title", "What is never recorded"),
            summary: ConsentL10n.text("consent.never.summary", "No photos, no video and no audio of the session."),
            learnMore: [
                ConsentL10n.text("consent.never.more.1", "The app never saves photos, video or audio of the session."),
                ConsentL10n.text("consent.never.more.2", "Camera frames are processed in memory and thrown away right after."),
                ConsentL10n.text("consent.never.more.3", "Your child's name never leaves this device."),
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "dataLocation",
            systemImage: "lock.iphone",
            title: ConsentL10n.text("consent.where.title", "Where the data goes"),
            summary: ConsentL10n.text("consent.where.summary", "Sessions stay on this device, encrypted, unless your clinic enrols it."),
            learnMore: [
                ConsentL10n.text("consent.where.more.1", "Sessions are encrypted on this device and left out of iCloud and computer backups."),
                ConsentL10n.text("consent.where.more.2", "Nothing leaves this device unless your clinic enrols it with a clinic code."),
                ConsentL10n.text("consent.where.more.3", "After that, sessions are sent to your clinic's console over an encrypted connection."),
                ConsentL10n.text("consent.where.more.4", "We never see camera images."),
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "access",
            systemImage: "person.2",
            title: ConsentL10n.text("consent.access.title", "Who can see it"),
            summary: ConsentL10n.text("consent.access.summary", "You, on this device, and your clinic once it enrols the device."),
            learnMore: [
                ConsentL10n.text("consent.access.more.1", "You can see sessions on this device."),
                ConsentL10n.text("consent.access.more.2", "Your clinic can see sessions once it has enrolled this device."),
                ConsentL10n.text("consent.access.more.3", "There is no advertising, no analytics and no tracking."),
                ConsentL10n.text("consent.access.more.4", "Your data is never sold."),
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "choices",
            systemImage: "switch.2",
            title: ConsentL10n.text("consent.choices.title", "Your choices"),
            summary: ConsentL10n.text("consent.choices.summary", "Every optional feature is off until you turn it on."),
            learnMore: [
                ConsentL10n.text("consent.choices.more.1", "Optional features stay off until you turn them on in Privacy & Your Data in Settings."),
                ConsentL10n.text("consent.choices.more.2", "The app keeps working with them off."),
                ConsentL10n.text("consent.choices.more.3", "You can stop a session at any time."),
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "withdrawing",
            systemImage: "arrow.uturn.backward.circle",
            title: ConsentL10n.text("consent.withdraw.title", "Withdrawing"),
            summary: ConsentL10n.text("consent.withdraw.summary", "You can delete everything at any time."),
            learnMore: [
                ConsentL10n.text("consent.withdraw.more.1", "You can delete all data on this device from Settings at any time."),
                ConsentL10n.text("consent.withdraw.more.2", "You can also withdraw your consent there. The app will ask again before the next session."),
                ConsentL10n.text("consent.withdraw.more.3", "To delete sessions already sent to your clinic, ask your clinic."),
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "notDiagnostic",
            systemImage: "stethoscope",
            title: ConsentL10n.text("consent.notdx.title", "Not a diagnostic tool"),
            summary: ClaimsCopy.notDiagnostic,
            learnMore: [
                ClaimsCopy.notDiagnostic,
                ClaimsCopy.clinicReviews,
                ConsentDocument.interpretationSentence,
            ].joined(separator: " ")
        ),
        ConsentSection(
            id: "time",
            systemImage: "clock",
            title: ConsentL10n.text("consent.time.title", "Time"),
            summary: ConsentL10n.text("consent.time.summary", "About 2 to 5 minutes per session."),
            learnMore: [
                ConsentL10n.text("consent.time.more.1", "Each session takes about 2 to 5 minutes."),
                ConsentL10n.text("consent.time.more.2", "If your child gets upset or tired, stop and try another day."),
            ].joined(separator: " ")
        ),
    ]
}
