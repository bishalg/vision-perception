import SwiftUI

public enum ConsentAccessibility {
    public static let flow = "consent_flow"
    public static let next = "consent_next"
    public static let learnMore = "consent_learn_more"
    public static let cancel = "consent_cancel"
    public static let guardianName = "consent_guardian_name"
    public static let relationship = "consent_relationship"
    public static let agree = "consent_agree"
    public static let notNow = "consent_not_now"
    public static let reviewOutdated = "consent_review_outdated"
}

/// ResearchKit-style visual consent (S-10): one page per `ConsentSection` with a large
/// symbol, title, summary and a "Learn more" sheet, then the review and signature page.
public struct ConsentFlowView: View {
    @ObservedObject private var store: ConsentRecordStore
    private let document: ConsentDocument
    private let onAgreed: () -> Void
    private let onCancel: () -> Void

    @State private var page = 0
    @State private var learnMoreSection: ConsentSection?
    @State private var showReview = false

    public init(
        store: ConsentRecordStore = .shared,
        onAgreed: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.store = store
        self.document = store.currentDocument
        self.onAgreed = onAgreed
        self.onCancel = onCancel
    }

    private var isLastPage: Bool { page >= document.sections.count - 1 }

    public var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                if store.needsReconsent {
                    Text(ConsentL10n.text("consent.updated", "We updated this consent. Please review it again before the next session."))
                        .glassStyle(.subheadline)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity)
                        .padding(10)
                        .background(LiquidGlass.warmAmber.opacity(LiquidGlass.tintOpacity))
                }

                TabView(selection: $page) {
                    ForEach(Array(document.sections.enumerated()), id: \.element.id) { index, section in
                        sectionPage(section, isFirst: index == 0)
                            .tag(index)
                    }
                }
                .tabViewStyle(.page(indexDisplayMode: .always))
                .indexViewStyle(.page(backgroundDisplayMode: .always))

                Button {
                    if isLastPage {
                        showReview = true
                    } else {
                        withAnimation { page += 1 }
                    }
                } label: {
                    Text(isLastPage
                         ? ConsentL10n.text("consent.review", "Review and sign")
                         : ConsentL10n.text("consent.next", "Next"))
                        .font(.system(size: 16, weight: .bold, design: .rounded))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(LiquidGlass.accentFill)
                        .foregroundColor(LiquidGlass.onAccent)
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                .buttonStyle(.plain)
                .padding(.horizontal, 24)
                .padding(.vertical, 16)
                .readableColumn()
                .accessibilityIdentifier(ConsentAccessibility.next)
            }
            .background(LiquidGlass.deepCanvas.ignoresSafeArea())
            .navigationTitle(ConsentL10n.text("consent.navtitle", "Consent"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(ConsentL10n.text("consent.cancel", "Not now"), action: onCancel)
                        .accessibilityIdentifier(ConsentAccessibility.cancel)
                }
            }
            .navigationDestination(isPresented: $showReview) {
                ConsentReviewView(store: store, onAgreed: onAgreed, onNotNow: onCancel)
            }
            .sheet(item: $learnMoreSection) { section in
                ConsentLearnMoreSheet(section: section)
            }
        }
        .accessibilityIdentifier(ConsentAccessibility.flow)
    }

    private func sectionPage(_ section: ConsentSection, isFirst: Bool) -> some View {
        ScrollView {
            VStack(spacing: 18) {
                Image(systemName: section.systemImage)
                    .font(.system(size: 64, weight: .regular))
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .padding(.top, 36)
                    .accessibilityHidden(true)
                Text(section.title)
                    .font(.system(size: 26, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textHeadline)
                    .multilineTextAlignment(.center)
                Text(section.summary)
                    .font(.system(size: 17))
                    .foregroundColor(LiquidGlass.textPrimary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                Button(ConsentL10n.text("consent.learnmore", "Learn more")) {
                    learnMoreSection = section
                }
                .font(.system(size: 15, weight: .semibold))
                .tint(LiquidGlass.celestialTeal)
                .accessibilityIdentifier(ConsentAccessibility.learnMore)

                if isFirst {
                    // S-13: the disclaimer appears on the consent overview.
                    NotDiagnosticFooter()
                        .padding(.top, 8)
                }
            }
            .padding(.horizontal, 28)
            .padding(.bottom, 48)
            .readableColumn()
        }
    }
}

/// S-25: consent, primer, before-you-begin, privacy and onboarding screens keep their content
/// in one centred column of readable width, so text does not run edge to edge on iPad.
public enum ConsentLayout {
    public static let readableWidth: CGFloat = 640
}

extension View {
    /// Constrains the content to `ConsentLayout.readableWidth` and centres it (S-25).
    func readableColumn(alignment: Alignment = .center) -> some View {
        frame(maxWidth: ConsentLayout.readableWidth, alignment: alignment)
            .frame(maxWidth: .infinity)
    }
}

/// The long text behind "Learn more" on a consent page.
struct ConsentLearnMoreSheet: View {
    let section: ConsentSection
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Label(section.title, systemImage: section.systemImage)
                        .font(.system(size: 20, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.textHeadline)
                    Text(section.learnMore)
                        .font(.system(size: 16))
                        .foregroundColor(LiquidGlass.textPrimary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(24)
                .readableColumn(alignment: .leading)
            }
            .background(LiquidGlass.deepCanvas.ignoresSafeArea())
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(ConsentL10n.text("consent.done", "Done")) { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

/// Shown instead of capture when `CaptureEntryGate` reports `.consentOutdated` (S-10): the
/// consent changed or was withdrawn. Offers "Review consent", which opens `ConsentFlowView`.
public struct ConsentOutdatedView: View {
    @ObservedObject private var store: ConsentRecordStore
    @State private var showConsentFlow = false

    public init(store: ConsentRecordStore = .shared) {
        self.store = store
    }

    public var body: some View {
        let content = UnsupportedDeviceContent.make(for: .consentOutdated)
        ZStack {
            LiquidGlass.deepCanvas.ignoresSafeArea()
            VStack(spacing: 16) {
                Image(systemName: content.systemImage)
                    .font(.system(size: 44, weight: .semibold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .accessibilityHidden(true)
                Text(content.title)
                    .font(.system(size: 20, weight: .bold, design: .rounded))
                    .multilineTextAlignment(.center)
                    .foregroundColor(LiquidGlass.textPrimary)
                Text(content.message)
                    .font(.system(size: 14))
                    .multilineTextAlignment(.center)
                    .foregroundColor(LiquidGlass.textSecondary)
                Button(ConsentL10n.text("consent.outdated.review", "Review consent")) {
                    showConsentFlow = true
                }
                .buttonStyle(.borderedProminent)
                .tint(LiquidGlass.accentFill)
                .foregroundStyle(LiquidGlass.onAccent)
                .accessibilityIdentifier(ConsentAccessibility.reviewOutdated)
            }
            .padding(28)
            .readableColumn()
        }
        .fullScreenCover(isPresented: $showConsentFlow) {
            ConsentFlowView(
                store: store,
                onAgreed: { showConsentFlow = false },
                onCancel: { showConsentFlow = false }
            )
        }
    }
}
