import SwiftUI

public enum BeforeYouBeginAccessibility {
    public static let view = "before_you_begin"
    public static let begin = "before_you_begin_start"
    public static let cancel = "before_you_begin_cancel"
}

/// Pre-session screen shown before positioning (S-13): the not-diagnostic block, a short
/// checklist, and what this session records given the current opt-ins. It has no
/// dependency on the wizard; the caller passes the two actions.
public struct BeforeYouBeginView: View {
    @ObservedObject private var optIns: FeatureOptIns
    private let onBegin: () -> Void
    private let onCancel: (() -> Void)?

    /// - Parameter onCancel: `nil` hides Cancel (for example when the screen is the root of a tab).
    public init(optIns: FeatureOptIns = .shared, onBegin: @escaping () -> Void, onCancel: (() -> Void)?) {
        self.optIns = optIns
        self.onBegin = onBegin
        self.onCancel = onCancel
    }

    public struct ChecklistItem: Hashable, Sendable {
        public let icon: String
        public let text: String
    }

    /// The four checklist items, in order.
    public static let checklist: [ChecklistItem] = [
        ChecklistItem(icon: "speaker.slash", text: ConsentL10n.text("begin.check.quiet", "A quiet room with few distractions.")),
        ChecklistItem(icon: "ruler", text: ConsentL10n.text("begin.check.distance", "Your child is seated 35 to 65 cm from the device.")),
        ChecklistItem(icon: "sun.max", text: ConsentL10n.text("begin.check.brightness", "Screen brightness is turned up.")),
        ChecklistItem(icon: "hand.raised", text: ConsentL10n.text("begin.check.stop", "You can stop at any time.")),
    ]

    /// What this session records, reflecting the opt-ins.
    @MainActor
    public static func recordedItems(optIns: FeatureOptIns) -> [String] {
        var items = [
            ConsentL10n.text("begin.rec.gaze", "Where your child looks on the screen"),
            ConsentL10n.text("begin.rec.head", "Head position and distance"),
            ConsentL10n.text("begin.rec.face", "Facial-expression measurements, for example how much your child smiles"),
            ConsentL10n.text("begin.rec.time", "Session time and length"),
        ]
        if optIns.clinicDelivery {
            items.append(ConsentL10n.text("begin.rec.delivery", "Delivered to your clinic once this device is enrolled"))
        } else {
            items.append(ConsentL10n.text("begin.rec.local", "Kept on this device only"))
        }
        return items
    }

    public var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Text(ConsentL10n.text("begin.title", "Before you begin"))
                        .font(.system(size: 26, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.textHeadline)

                    VStack(alignment: .leading, spacing: 6) {
                        Text(ClaimsCopy.notDiagnostic)
                            .glassStyle(.headline)
                        Text(ClaimsCopy.clinicReviews)
                            .glassStyle(.body)
                        Text(ConsentDocument.interpretationSentence)
                            .glassStyle(.body)
                    }
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .glassCard(cornerRadius: 14, padding: 14)
                    .accessibilityElement(children: .combine)

                    VStack(alignment: .leading, spacing: 10) {
                        Text(ConsentL10n.text("begin.check.title", "Check")).glassStyle(.sectionHeader)
                        ForEach(Self.checklist, id: \.text) { item in
                            Label {
                                Text(item.text).glassStyle(.body)
                            } icon: {
                                Image(systemName: item.icon).foregroundColor(LiquidGlass.celestialTeal)
                            }
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .glassCard(cornerRadius: 14, padding: 14)

                    VStack(alignment: .leading, spacing: 10) {
                        Text(ConsentL10n.text("begin.rec.title", "This session records")).glassStyle(.sectionHeader)
                        ForEach(Self.recordedItems(optIns: optIns), id: \.self) { item in
                            Label {
                                Text(item).glassStyle(.body)
                            } icon: {
                                Image(systemName: "checkmark.circle").foregroundColor(LiquidGlass.emeraldMint)
                            }
                        }
                        Text(ConsentL10n.text("begin.rec.never", "No photos, video or audio are ever saved."))
                            .glassStyle(.subheadline)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .glassCard(cornerRadius: 14, padding: 14)
                }
                .padding(24)
                .frame(maxWidth: 640)
                .frame(maxWidth: .infinity)
            }

            VStack(spacing: 10) {
                Button(action: onBegin) {
                    Text(ConsentL10n.text("begin.start", "Begin"))
                        .font(.system(size: 16, weight: .bold, design: .rounded))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(LiquidGlass.accentFill)
                        .foregroundColor(LiquidGlass.onAccent)
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier(BeforeYouBeginAccessibility.begin)

                if let onCancel {
                    Button(ConsentL10n.text("begin.cancel", "Cancel"), action: onCancel)
                        .tint(LiquidGlass.celestialTeal)
                        .accessibilityIdentifier(BeforeYouBeginAccessibility.cancel)
                }
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
        }
        .background(LiquidGlass.deepCanvas.ignoresSafeArea())
        .accessibilityIdentifier(BeforeYouBeginAccessibility.view)
    }
}
