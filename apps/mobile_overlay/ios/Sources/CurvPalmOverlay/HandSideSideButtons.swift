import SwiftUI
import CurvCameraOverlay

/**
 * Compact bottom-left and bottom-right hand side buttons.
 * Placed on the bottom inside camera view on the same row as tilt/glare telemetry:
 * - Left button:  [ • L ]
 * - Right button: [ R • ]
 * Dot glows emerald green when that hand is active (either auto-detected or manually chosen).
 */
public struct LeftHandSideButton: View {
    @Binding public var selection: HandSide
    public var autoDetected: HandSide

    public init(selection: Binding<HandSide>, autoDetected: HandSide = .left) {
        self._selection = selection
        self.autoDetected = autoDetected
    }

    private var isActive: Bool {
        selection == .left || (selection == .auto && autoDetected == .left)
    }

    public var body: some View {
        Button(action: {
            selection = (selection == .left) ? .auto : .left
        }) {
            HStack(spacing: 4) {
                Circle()
                    .fill(isActive ? OverlayPalette.emerald : Color.white.opacity(0.32))
                    .frame(width: 5, height: 5)
                    .shadow(color: isActive ? OverlayPalette.emerald.opacity(0.85) : .clear, radius: 4)

                Text("L")
                    .font(.system(size: 11, weight: isActive ? .heavy : .bold, design: .rounded))
                    .foregroundColor(isActive ? .white : .white.opacity(0.55))
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(
                Capsule()
                    .fill(Color.black.opacity(0.58))
                    .overlay(
                        Capsule()
                            .stroke(
                                isActive ? OverlayPalette.emerald.opacity(0.8) : Color.white.opacity(0.20),
                                lineWidth: isActive ? 1.4 : 0.8
                            )
                    )
            )
            .shadow(color: Color.black.opacity(0.25), radius: 4, y: 2)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Left Hand")
        .accessibilityValue(isActive ? "Active" : "Inactive")
    }
}

public struct RightHandSideButton: View {
    @Binding public var selection: HandSide
    public var autoDetected: HandSide

    public init(selection: Binding<HandSide>, autoDetected: HandSide = .right) {
        self._selection = selection
        self.autoDetected = autoDetected
    }

    private var isActive: Bool {
        selection == .right || (selection == .auto && autoDetected == .right)
    }

    public var body: some View {
        Button(action: {
            selection = (selection == .right) ? .auto : .right
        }) {
            HStack(spacing: 4) {
                Text("R")
                    .font(.system(size: 11, weight: isActive ? .heavy : .bold, design: .rounded))
                    .foregroundColor(isActive ? .white : .white.opacity(0.55))

                Circle()
                    .fill(isActive ? OverlayPalette.emerald : Color.white.opacity(0.32))
                    .frame(width: 5, height: 5)
                    .shadow(color: isActive ? OverlayPalette.emerald.opacity(0.85) : .clear, radius: 4)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(
                Capsule()
                    .fill(Color.black.opacity(0.58))
                    .overlay(
                        Capsule()
                            .stroke(
                                isActive ? OverlayPalette.emerald.opacity(0.8) : Color.white.opacity(0.20),
                                lineWidth: isActive ? 1.4 : 0.8
                            )
                    )
            )
            .shadow(color: Color.black.opacity(0.25), radius: 4, y: 2)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Right Hand")
        .accessibilityValue(isActive ? "Active" : "Inactive")
    }
}
