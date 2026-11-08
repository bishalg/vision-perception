import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

public enum PermissionPrimerAccessibility {
    public static let view = "permission_primer"
    public static let continueButton = "permission_primer_continue"
    public static let notNow = "permission_primer_not_now"
}

/// In-app primer shown before an iOS permission prompt (S-11): what is used, why, what is
/// never stored, and how to change it later. "Not now" is always available; the app keeps
/// working with the feature off.
public struct PermissionPrimerView: View {
    public let kind: PermissionKind
    private let request: @MainActor () async -> Void
    private let onFinish: @MainActor () -> Void

    @State private var isRequesting = false

    /// - Parameters:
    ///   - request: Shows the system prompt (or runs the system request). Awaited on Continue.
    ///   - onFinish: Called after Continue (once `request` returns) or Not now.
    public init(_ kind: PermissionKind, request: @escaping @MainActor () async -> Void, onFinish: @escaping @MainActor () -> Void = {}) {
        self.kind = kind
        self.request = request
        self.onFinish = onFinish
    }

    public var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    HStack {
                        Spacer()
                        Image(systemName: kind.systemImage)
                            .font(.system(size: 56))
                            .foregroundColor(LiquidGlass.celestialTeal)
                            .accessibilityHidden(true)
                        Spacer()
                    }
                    .padding(.top, 28)

                    Text(kind.title)
                        .font(.system(size: 26, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.textHeadline)
                        .frame(maxWidth: .infinity)

                    PermissionPrimerRow(icon: "checkmark.circle", title: ConsentL10n.text("primer.use", "What we use"), text: kind.whatWeUse)
                    PermissionPrimerRow(icon: "questionmark.circle", title: ConsentL10n.text("primer.why", "Why"), text: kind.why)
                    PermissionPrimerRow(icon: "xmark.shield", title: ConsentL10n.text("primer.never", "Never stored"), text: kind.neverStored)
                    if let note = kind.statusNote {
                        PermissionPrimerRow(icon: "info.circle", title: ConsentL10n.text("primer.note", "Good to know"), text: note)
                    }
                    PermissionPrimerRow(icon: "gearshape", title: ConsentL10n.text("primer.later", "Change it later"), text: kind.howToChangeLater)
                }
                .padding(24)
                .frame(maxWidth: 560)
                .frame(maxWidth: .infinity)
            }

            VStack(spacing: 10) {
                Button {
                    guard !isRequesting else { return }
                    isRequesting = true
                    Task { @MainActor in
                        await request()
                        isRequesting = false
                        onFinish()
                    }
                } label: {
                    Text(ConsentL10n.text("primer.continue", "Continue"))
                        .font(.system(size: 16, weight: .bold, design: .rounded))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(LiquidGlass.accentFill)
                        .foregroundColor(LiquidGlass.onAccent)
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                .buttonStyle(.plain)
                .disabled(isRequesting)
                .accessibilityIdentifier(PermissionPrimerAccessibility.continueButton)

                Button(ConsentL10n.text("primer.notnow", "Not now"), action: onFinish)
                    .tint(LiquidGlass.celestialTeal)
                    .accessibilityIdentifier(PermissionPrimerAccessibility.notNow)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
        }
        .background(LiquidGlass.deepCanvas.ignoresSafeArea())
        .accessibilityIdentifier(PermissionPrimerAccessibility.view)
    }
}

private struct PermissionPrimerRow: View {
    let icon: String
    let title: String
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: icon)
                .font(.system(size: 18, weight: .semibold))
                .foregroundColor(LiquidGlass.celestialTeal)
                .frame(width: 24)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).glassStyle(.headline)
                Text(text)
                    .glassStyle(.body)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// Presents `PermissionPrimerView` as a sheet while `kind` is non-nil. Call sites set the
/// binding instead of calling the system request directly; the sheet runs `request` on
/// Continue and clears the binding on Continue or Not now.
public struct PermissionPrimerSheet: ViewModifier {
    @Binding var kind: PermissionKind?
    let request: @MainActor (PermissionKind) async -> Void
    let onFinish: @MainActor (PermissionKind) -> Void

    public func body(content: Content) -> some View {
        content.sheet(item: $kind) { item in
            PermissionPrimerView(
                item,
                request: { await request(item) },
                onFinish: {
                    kind = nil
                    onFinish(item)
                }
            )
            .interactiveDismissDisabled()
        }
    }
}

public extension View {
    /// Shows the permission primer for `kind` before the system prompt (S-11).
    func permissionPrimer(
        _ kind: Binding<PermissionKind?>,
        request: @escaping @MainActor (PermissionKind) async -> Void,
        onFinish: @escaping @MainActor (PermissionKind) -> Void = { _ in }
    ) -> some View {
        modifier(PermissionPrimerSheet(kind: kind, request: request, onFinish: onFinish))
    }
}

/// Opens this app's page in iOS Settings.
@MainActor
public enum AppSettingsLink {
    public static func open() {
        #if canImport(UIKit)
        if let url = URL(string: UIApplication.openSettingsURLString) {
            UIApplication.shared.open(url)
        }
        #endif
    }
}
