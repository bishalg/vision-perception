import SwiftUI

/// Review and signature page (S-10): the full document, the guardian's typed name and
/// relationship, and a single "I agree" button.
public struct ConsentReviewView: View {
    @ObservedObject private var store: ConsentRecordStore
    private let onAgreed: () -> Void
    private let onNotNow: () -> Void

    @State private var guardianName = ""
    @State private var relationship: GuardianRelationship = .parent

    public init(store: ConsentRecordStore = .shared, onAgreed: @escaping () -> Void, onNotNow: @escaping () -> Void) {
        self.store = store
        self.onAgreed = onAgreed
        self.onNotNow = onNotNow
    }

    static let confirmationSentence = ConsentL10n.text(
        "consent.confirm",
        "By agreeing you confirm you are the parent or legal guardian of the child, or clinic staff recording with the guardian's permission."
    )

    private var canAgree: Bool {
        !guardianName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text(ConsentL10n.text("consent.review.title", "Review"))
                    .font(.system(size: 24, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textHeadline)

                ConsentDocumentText(document: store.currentDocument)
                    .glassCard(cornerRadius: 14, padding: 14)

                VStack(alignment: .leading, spacing: 8) {
                    Text(ConsentL10n.text("consent.name.label", "Your full name"))
                        .glassStyle(.headline)
                    TextField(ConsentL10n.text("consent.name.placeholder", "Full name"), text: $guardianName)
                        .textContentType(.name)
                        .textInputAutocapitalization(.words)
                        .autocorrectionDisabled()
                        .padding(12)
                        .background(RoundedRectangle(cornerRadius: 10).fill(LiquidGlass.cardInset))
                        .accessibilityIdentifier(ConsentAccessibility.guardianName)

                    Text(ConsentL10n.text("consent.relationship.label", "You are the child's"))
                        .glassStyle(.headline)
                        .padding(.top, 6)
                    Picker(ConsentL10n.text("consent.relationship.label", "You are the child's"), selection: $relationship) {
                        ForEach(GuardianRelationship.allCases) { item in
                            Text(item.displayName).tag(item)
                        }
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                    .accessibilityIdentifier(ConsentAccessibility.relationship)
                }
                .glassCard(cornerRadius: 14, padding: 14)

                Button {
                    if store.agree(guardianName: guardianName, relationship: relationship) != nil {
                        onAgreed()
                    }
                } label: {
                    Text(ConsentL10n.text("consent.agree", "I agree"))
                        .font(.system(size: 16, weight: .bold, design: .rounded))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(canAgree ? LiquidGlass.accentFill : LiquidGlass.accentFillDisabled)
                        .foregroundColor(canAgree ? LiquidGlass.onAccent : LiquidGlass.textTertiary)
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                .buttonStyle(.plain)
                .disabled(!canAgree)
                .accessibilityIdentifier(ConsentAccessibility.agree)

                Text(Self.confirmationSentence)
                    .glassStyle(.caption)
                    .fixedSize(horizontal: false, vertical: true)

                Button(ConsentL10n.text("consent.notnow", "Not now"), action: onNotNow)
                    .frame(maxWidth: .infinity)
                    .tint(LiquidGlass.celestialTeal)
                    .accessibilityIdentifier(ConsentAccessibility.notNow)
            }
            .padding(24)
            .frame(maxWidth: 640)
            .frame(maxWidth: .infinity)
        }
        .background(LiquidGlass.deepCanvas.ignoresSafeArea())
        .navigationTitle(ConsentL10n.text("consent.review.navtitle", "Review and sign"))
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// The consent document as readable text, section by section.
struct ConsentDocumentText: View {
    let document: ConsentDocument

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach(document.sections) { section in
                VStack(alignment: .leading, spacing: 4) {
                    Text(section.title)
                        .glassStyle(.headline)
                    Text(section.learnMore)
                        .glassStyle(.body)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Text(ConsentL10n.text("consent.version.label", "Document version") + " " + document.version)
                .glassStyle(.caption)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Read-only view of the signed record (reachable from Privacy & Your Data).
public struct ConsentRecordView: View {
    @ObservedObject private var store: ConsentRecordStore

    public init(store: ConsentRecordStore = .shared) {
        self.store = store
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let record = store.record {
                    VStack(alignment: .leading, spacing: 8) {
                        recordRow(ConsentL10n.text("consent.record.name", "Agreed by"), record.guardianName)
                        recordRow(ConsentL10n.text("consent.record.relationship", "Relationship"), record.relationship.displayName)
                        recordRow(ConsentL10n.text("consent.record.date", "Date"), record.agreedAt.formatted(date: .long, time: .shortened))
                        recordRow(ConsentL10n.text("consent.record.version", "Document version"), record.documentVersion)
                        recordRow(ConsentL10n.text("consent.record.app", "App version"), record.appVersion)
                        recordRow(ConsentL10n.text("consent.record.fingerprint", "Document fingerprint"), String(record.documentSha256.prefix(16)))
                        if !store.isCurrent {
                            Text(ConsentL10n.text("consent.record.outdated", "This consent was for an older version. The app will ask again before the next session."))
                                .glassStyle(.subheadline, color: LiquidGlass.warmAmber)
                        }
                    }
                    .glassCard(cornerRadius: 14, padding: 14)

                    Text(store.isCurrent
                         ? ConsentL10n.text("consent.record.doc", "What you agreed to")
                         : ConsentL10n.text("consent.record.doc.current", "The current consent text"))
                        .glassStyle(.sectionHeader)
                    ConsentDocumentText(document: store.currentDocument)
                        .glassCard(cornerRadius: 14, padding: 14)
                } else {
                    Text(ConsentL10n.text("consent.record.none", "No consent is recorded on this device. The app will ask before the next session."))
                        .glassStyle(.body)
                        .glassCard(cornerRadius: 14, padding: 14)
                }
            }
            .padding(20)
        }
        .background(LiquidGlass.deepCanvas.ignoresSafeArea())
        .navigationTitle(ConsentL10n.text("consent.record.title", "Your consent"))
        .navigationBarTitleDisplayMode(.inline)
    }

    private func recordRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).glassStyle(.subheadline)
            Spacer(minLength: 12)
            Text(value)
                .glassStyle(.headline)
                .multilineTextAlignment(.trailing)
        }
    }
}
