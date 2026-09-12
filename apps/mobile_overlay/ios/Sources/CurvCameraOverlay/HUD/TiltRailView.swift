import SwiftUI

/**
 * Small vertical tick on left/right edge showing tilt.
 * left  = CAM (device gyro roll vs gravity)
 * right = Δ hand-to-camera (handRoll - deviceRoll)
 * Line is 2×28 rotated by the angle so user sees physical tilt.
 */
public struct TiltRailView: View {
    public let deviceRollDeg: Float
    public let handRollDeg: Float
    public let isLeft: Bool

    public init(deviceRollDeg: Float, handRollDeg: Float, isLeft: Bool) {
        self.deviceRollDeg = deviceRollDeg
        self.handRollDeg = handRollDeg
        self.isLeft = isLeft
    }

    private var angleDeg: Double {
        if isLeft {
            return Double(deviceRollDeg)
        } else {
            // hand axis relative to camera axis
            var d = Double(handRollDeg - deviceRollDeg)
            // wrap to -180..180
            while d > 180 { d -= 360 }
            while d < -180 { d += 360 }
            return d
        }
    }

    private var degreeText: String {
        String(format: "%.0f°", abs(angleDeg))
    }

    private var isOk: Bool {
        abs(angleDeg) <= (isLeft ? 8 : 10) // green when near vertical
    }

    public var body: some View {
        VStack(spacing: 4) {
            // Icon helper: iPhone/cam symbol on left, Delta (Δ) on right
            if isLeft {
                Image(systemName: "iphone.gen3")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundColor(isOk ? .white.opacity(0.75) : OverlayPalette.amber.opacity(0.9))
                    .shadow(color: .black.opacity(0.6), radius: 3)
            } else {
                Text("Δ")
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(isOk ? .white.opacity(0.8) : OverlayPalette.amber.opacity(0.95))
                    .shadow(color: .black.opacity(0.6), radius: 3)
            }

            // Minimal vertical tick indicator in frame
            ZStack {
                // track
                Capsule()
                    .fill(Color.white.opacity(0.20))
                    .frame(width: 2, height: 30)

                // active rotating line
                Capsule()
                    .fill(isOk ? OverlayPalette.emerald : OverlayPalette.amber)
                    .frame(width: 2.5, height: 24)
                    .rotationEffect(.degrees(angleDeg))
                    .shadow(color: (isOk ? OverlayPalette.emerald : OverlayPalette.amber).opacity(0.75), radius: 4)

                // center pivot dot
                Circle()
                    .fill(Color.white.opacity(0.95))
                    .frame(width: 3, height: 3)
            }
            .frame(width: 20, height: 32)

            // Pure degree value — no English words
            Text(degreeText)
                .font(.system(size: 9, weight: .bold, design: .monospaced))
                .foregroundColor(isOk ? OverlayPalette.emerald : OverlayPalette.amber)
                .shadow(color: .black.opacity(0.7), radius: 3)
                .contentTransition(.numericText())
        }
        .padding(.horizontal, 4)
        .padding(.vertical, 4)
    }
}
