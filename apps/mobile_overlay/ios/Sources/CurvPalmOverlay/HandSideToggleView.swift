import SwiftUI

public struct HandSideToggleView: View {
    @Binding public var selection: HandSide
    public var autoDetected: HandSide

    public init(selection: Binding<HandSide>, autoDetected: HandSide = .right) {
        self._selection = selection
        self.autoDetected = autoDetected
    }

    /// True if Left is effectively active (either locked to left or auto-detected as left)
    private var isLeftActive: Bool {
        selection == .left || (selection == .auto && autoDetected == .left)
    }

    /// True if Right is effectively active (either locked to right or auto-detected as right)
    private var isRightActive: Bool {
        selection == .right || (selection == .auto && autoDetected == .right)
    }

    public var body: some View {
        HStack(spacing: 2) {
            sideButton(
                title: "L",
                isActive: isLeftActive,
                isLocked: selection == .left,
                onTap: {
                    if selection == .left {
                        selection = .auto // unlock back to auto
                    } else {
                        selection = .left // lock to left
                    }
                }
            )

            sideButton(
                title: "R",
                isActive: isRightActive,
                isLocked: selection == .right,
                onTap: {
                    if selection == .right {
                        selection = .auto // unlock back to auto
                    } else {
                        selection = .right // lock to right
                    }
                }
            )
        }
        .padding(2)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.42))
                .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 0.8))
        )
    }

    @ViewBuilder
    private func sideButton(
        title: String,
        isActive: Bool,
        isLocked: Bool,
        onTap: @escaping () -> Void
    ) -> some View {
        Button(action: onTap) {
            HStack(spacing: 3) {
                Text(title)
                    .font(.system(size: 11, weight: .bold, design: .rounded))

                // Small dot indicating auto-detected active hand (when not locked)
                if isActive && !isLocked && selection == .auto {
                    Circle()
                        .fill(Color(red: 0.20, green: 0.84, blue: 0.29))
                        .frame(width: 4, height: 4)
                }
            }
            .foregroundColor(isLocked ? .black : (isActive ? .white : .white.opacity(0.45)))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                Capsule()
                    .fill(
                        isLocked
                            ? Color.white
                            : (isActive ? Color.white.opacity(0.16) : Color.clear)
                    )
            )
        }
        .buttonStyle(.plain)
    }
}
