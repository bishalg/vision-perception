import SwiftUI
import CurvAsdGazeKit

/// Persistent "Simulated data" badge (SIM-8). Renders nothing for real TrueDepth captures.
/// Any UI that displays a session places this next to the session title.
public struct SimulatedDataBadge: View {
    public let captureSource: CaptureSource

    public init(captureSource: CaptureSource) {
        self.captureSource = captureSource
    }

    public var body: some View {
        if captureSource == .simulated {
            HStack(spacing: 5) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 11, weight: .bold))
                Text("Simulated data")
                    .font(.system(size: 12, weight: .bold, design: .rounded))
                Text("Not a real observation")
                    .font(.system(size: 10, weight: .medium))
            }
            .foregroundColor(LiquidGlass.onAccent)
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(Capsule().fill(LiquidGlass.warmAmber))
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Simulated data. Not a real observation.")
        }
    }
}
