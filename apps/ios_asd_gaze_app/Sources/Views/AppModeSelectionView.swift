import SwiftUI

// MARK: - First-Launch App Mode Selection

/// Shown once on first app launch (and reachable from Settings): choose whether the device
/// is used at home by a parent/caregiver or in a clinical/professional setting.
///
/// Mobile-first: single column, two large touch-target cards, explicit Continue to avoid
/// an accidental choice, and a persistent "change anytime" escape hatch.
public struct AppModeSelectionView: View {
    /// Called with the chosen mode once the user taps Continue.
    public let onContinue: (AppMode) -> Void

    @State private var selected: AppMode?

    public init(onContinue: @escaping (AppMode) -> Void) {
        self.onContinue = onContinue
    }

    public var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    header
                    ForEach(AppMode.allCases) { mode in
                        modeCard(mode)
                    }
                    changeLaterNote
                }
                .padding(.horizontal, 20)
                .padding(.top, 24)
                .padding(.bottom, 16)
            }

            continueButton
                .padding(.horizontal, 20)
                .padding(.vertical, 12)
                .background(.ultraThinMaterial)
        }
        .background(
            LinearGradient(
                colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            .ignoresSafeArea()
        )
        .preferredColorScheme(.dark)
    }

    // MARK: Header

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: "eye.circle.fill")
                .font(.system(size: 40))
                .foregroundStyle(
                    LinearGradient(
                        colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                        startPoint: .topLeading, endPoint: .bottomTrailing
                    )
                )
            Text("How will you use Curv?")
                .font(.system(size: 26, weight: .bold, design: .rounded))
            Text("This tailors the app to your setting. You can switch it anytime in Settings.")
                .font(.system(size: 14))
                .foregroundColor(.secondary)
        }
        .accessibilityElement(children: .combine)
    }

    // MARK: Mode Cards

    private func modeCard(_ mode: AppMode) -> some View {
        let isSelected = selected == mode
        return Button {
            withAnimation(.easeInOut(duration: 0.2)) {
                selected = mode
            }
        } label: {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: mode.systemImage)
                    .font(.system(size: 26))
                    .foregroundColor(isSelected ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal)
                    .frame(width: 52, height: 52)
                    .background(
                        Circle().fill((isSelected ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal).opacity(0.12))
                    )

                VStack(alignment: .leading, spacing: 5) {
                    Text(mode.displayName)
                        .font(.system(size: 17, weight: .bold, design: .rounded))
                        .foregroundColor(.primary)
                    Text(mode.tagline)
                        .font(.system(size: 13))
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(mode.details, id: \.self) { detail in
                            HStack(alignment: .top, spacing: 6) {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 9, weight: .bold))
                                    .foregroundColor(LiquidGlass.emeraldMint)
                                    .padding(.top, 3)
                                Text(detail)
                                    .font(.system(size: 12))
                                    .foregroundColor(.secondary)
                            }
                        }
                    }
                    .padding(.top, 3)
                }

                Spacer(minLength: 0)

                Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: 22))
                    .foregroundColor(isSelected ? LiquidGlass.emeraldMint : .secondary)
                    .padding(.top, 4)
            }
            .padding(16)
            .frame(minHeight: 120, alignment: .topLeading)
            .background(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(LiquidGlass.cardBackground)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(
                        isSelected ? LiquidGlass.emeraldMint : LiquidGlass.cardBorder,
                        lineWidth: isSelected ? 1.6 : 1
                    )
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(mode.displayName) mode")
        .accessibilityHint(mode.tagline)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    private var changeLaterNote: some View {
        HStack(spacing: 6) {
            Image(systemName: "gearshape")
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            Text("You can switch between Parent and Clinic mode later in Settings.")
                .font(.system(size: 12))
                .foregroundColor(.secondary)
        }
        .padding(.top, 2)
    }

    // MARK: Continue

    private var continueButton: some View {
        Button {
            guard let mode = selected else { return }
            onContinue(mode)
        } label: {
            Text(selected == nil ? "Select a mode to continue" : "Continue as \(selected!.shortName)")
                .font(.system(size: 16, weight: .bold, design: .rounded))
                .foregroundColor(selected == nil ? .secondary : .black)
                .frame(maxWidth: .infinity, minHeight: 52)
                .background(
                    Capsule().fill(
                        selected == nil
                            ? AnyShapeStyle(Color.secondary.opacity(0.18))
                            : AnyShapeStyle(LinearGradient(
                                colors: [LiquidGlass.emeraldMint, LiquidGlass.celestialTeal],
                                startPoint: .leading, endPoint: .trailing
                            ))
                    )
                )
        }
        .buttonStyle(.plain)
        .disabled(selected == nil)
        .accessibilityLabel("Continue")
    }
}
