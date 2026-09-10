import SwiftUI

public struct HandSideToggleView: View {
    @Binding public var selection: HandSide
    public var autoDetected: HandSide

    public init(selection: Binding<HandSide>, autoDetected: HandSide = .right) {
        self._selection = selection
        self.autoDetected = autoDetected
    }

    public var body: some View {
        HStack(spacing: 0) {
            ForEach(HandSide.allCases, id: \.rawValue) { side in
                Button {
                    selection = side
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: side.icon)
                            .font(.system(size: 10, weight: .bold))
                        Text(side.displayName)
                            .font(.system(size: 11, weight: .semibold))
                        if side == .auto && autoDetected != .auto {
                            Text("· \(autoDetected.displayName)")
                                .font(.system(size: 9, weight: .medium))
                                .opacity(0.6)
                        }
                    }
                    .foregroundColor(selection == side ? .black : .white.opacity(0.85))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(
                        Capsule()
                            .fill(selection == side ? Color.white : Color.clear)
                            .overlay(Capsule().stroke(Color.white.opacity(selection == side ? 0 : 0.22), lineWidth: 1))
                    )
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .background(
            Capsule()
                .fill(.ultraThinMaterial)
                .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 1))
        )
    }
}
