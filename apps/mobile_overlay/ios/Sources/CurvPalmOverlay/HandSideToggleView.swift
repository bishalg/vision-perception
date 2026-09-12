import SwiftUI

public struct HandSideToggleView: View {
    @Binding public var selection: HandSide
    public var autoDetected: HandSide
    public var isHandDetected: Bool

    public init(
        selection: Binding<HandSide>,
        autoDetected: HandSide = .right,
        isHandDetected: Bool = false
    ) {
        self._selection = selection
        self.autoDetected = autoDetected
        self.isHandDetected = isHandDetected
    }

    private var isAuto: Bool { selection == .auto }

    public var body: some View {
        HStack(spacing: 3) {
            // AUTO Mode Badge & Button
            Button(action: {
                selection = .auto
            }) {
                HStack(spacing: 4) {
                    if isAuto {
                        Circle()
                            .fill(isHandDetected ? Color(red: 0.20, green: 0.84, blue: 0.29) : Color.yellow.opacity(0.85))
                            .frame(width: 5, height: 5)
                    }

                    Text(isAuto ? (isHandDetected ? (autoDetected == .left ? "AUTO • L" : "AUTO • R") : "AUTO") : "AUTO")
                        .font(.system(size: 11, weight: isAuto ? .bold : .medium, design: .rounded))
                }
                .foregroundColor(isAuto ? .white : .white.opacity(0.45))
                .padding(.horizontal, 9)
                .padding(.vertical, 5)
                .background(
                    Capsule()
                        .fill(isAuto ? (isHandDetected ? Color(red: 0.20, green: 0.84, blue: 0.29).opacity(0.20) : Color.white.opacity(0.12)) : Color.clear)
                        .overlay(
                            Capsule()
                                .stroke(isAuto ? (isHandDetected ? Color(red: 0.20, green: 0.84, blue: 0.29).opacity(0.60) : Color.white.opacity(0.22)) : Color.clear, lineWidth: 1)
                        )
                )
            }
            .buttonStyle(.plain)

            // Divider
            Rectangle()
                .fill(Color.white.opacity(0.18))
                .frame(width: 1, height: 12)

            // L (Left manual lock / active auto indicator)
            Button(action: {
                selection = (selection == .left) ? .auto : .left
            }) {
                HStack(spacing: 2) {
                    Text("L")
                        .font(.system(size: 11, weight: .bold, design: .rounded))
                    if isAuto && isHandDetected && autoDetected == .left {
                        Circle()
                            .fill(Color(red: 0.20, green: 0.84, blue: 0.29))
                            .frame(width: 4, height: 4)
                    }
                }
                .foregroundColor(
                    selection == .left
                        ? .black
                        : ((isAuto && isHandDetected && autoDetected == .left) ? Color(red: 0.20, green: 0.84, blue: 0.29) : .white.opacity(0.45))
                )
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(
                    Capsule()
                        .fill(
                            selection == .left
                                ? Color.white
                                : ((isAuto && isHandDetected && autoDetected == .left) ? Color(red: 0.20, green: 0.84, blue: 0.29).opacity(0.18) : Color.clear)
                        )
                )
            }
            .buttonStyle(.plain)

            // R (Right manual lock / active auto indicator)
            Button(action: {
                selection = (selection == .right) ? .auto : .right
            }) {
                HStack(spacing: 2) {
                    Text("R")
                        .font(.system(size: 11, weight: .bold, design: .rounded))
                    if isAuto && isHandDetected && autoDetected == .right {
                        Circle()
                            .fill(Color(red: 0.20, green: 0.84, blue: 0.29))
                            .frame(width: 4, height: 4)
                    }
                }
                .foregroundColor(
                    selection == .right
                        ? .black
                        : ((isAuto && isHandDetected && autoDetected == .right) ? Color(red: 0.20, green: 0.84, blue: 0.29) : .white.opacity(0.45))
                )
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(
                    Capsule()
                        .fill(
                            selection == .right
                                ? Color.white
                                : ((isAuto && isHandDetected && autoDetected == .right) ? Color(red: 0.20, green: 0.84, blue: 0.29).opacity(0.18) : Color.clear)
                        )
                )
            }
            .buttonStyle(.plain)
        }
        .padding(3)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.45))
                .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 0.8))
        )
        .animation(.easeInOut(duration: 0.2), value: isHandDetected)
        .animation(.easeInOut(duration: 0.2), value: autoDetected)
        .animation(.easeInOut(duration: 0.2), value: selection)
    }
}
