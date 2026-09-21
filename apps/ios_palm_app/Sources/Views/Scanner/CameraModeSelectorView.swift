import SwiftUI
import CurvCameraOverlay

// MARK: - Mode Capsule Selector

/**
 * Top camera mode selector capsule.
 * Replaces the old "AUTO - L R" reticle header.
 * Switches between: Still Photo · 3D Orbit · Close-Up · Fingertips.
 * Each pill shows a step-number badge or a green checkmark when done.
 */
public struct CameraModeSelectorView: View {
    public let selectedMode: PalmScanMode
    public var completedModes: Set<PalmScanMode> = []
    public let onSelect: (PalmScanMode) -> Void

    public init(
        selectedMode: PalmScanMode,
        completedModes: Set<PalmScanMode> = [],
        onSelect: @escaping (PalmScanMode) -> Void
    ) {
        self.selectedMode = selectedMode
        self.completedModes = completedModes
        self.onSelect = onSelect
    }

    public var body: some View {
        HStack(spacing: 4) {
            ForEach(PalmScanMode.allCases) { mode in
                ModePillButton(
                    mode: mode,
                    isSelected: mode == selectedMode,
                    isDone: completedModes.contains(mode),
                    onSelect: { onSelect(mode) }
                )
            }
        }
        .padding(3)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.65))
                .overlay(Capsule().stroke(Color.white.opacity(0.24), lineWidth: 0.8))
        )
        .shadow(color: Color.black.opacity(0.35), radius: 8, y: 3)
    }
}

// MARK: - Mode Pill Button

private struct ModePillButton: View {
    let mode: PalmScanMode
    let isSelected: Bool
    let isDone: Bool
    let onSelect: () -> Void

    private var pillBackground: Color {
        guard isSelected else { return Color.clear }
        switch mode {
        case .orbitScan:   return Color.cyan.opacity(0.35)
        case .stillPhoto:  return OverlayPalette.emerald.opacity(0.35)
        case .closeUpMosaic: return Color.orange.opacity(0.30)
        case .fingertipScan: return Color.purple.opacity(0.30)
        }
    }

    private var pillStroke: Color {
        guard isSelected else { return Color.clear }
        switch mode {
        case .orbitScan:    return Color.cyan
        case .stillPhoto:   return OverlayPalette.emerald
        case .closeUpMosaic: return Color.orange.opacity(0.85)
        case .fingertipScan: return Color.purple.opacity(0.85)
        }
    }

    var body: some View {
        Button(action: {
            withAnimation(.spring(response: 0.28, dampingFraction: 0.82)) {
                onSelect()
            }
        }) {
            HStack(spacing: 4) {
                if isDone {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundColor(OverlayPalette.emerald)
                } else {
                    Text("\(mode.stepNumber)")
                        .font(.system(size: 10, weight: .bold, design: .rounded))
                        .foregroundColor(isSelected ? .black : .white.opacity(0.8))
                        .frame(width: 14, height: 14)
                        .background(
                            Circle().fill(isSelected ? Color.white : Color.white.opacity(0.2))
                        )
                }

                Text(mode.shortTitle)
                    .font(.system(size: 11, weight: isSelected ? .bold : .medium, design: .rounded))
                    .foregroundColor(isSelected ? .white : .white.opacity(0.70))
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(pillBackground)
                    .overlay(
                        Capsule()
                            .stroke(pillStroke, lineWidth: 1)
                    )
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(mode.displayName)
        .accessibilityValue(isSelected ? "Selected" : "")
    }
}
