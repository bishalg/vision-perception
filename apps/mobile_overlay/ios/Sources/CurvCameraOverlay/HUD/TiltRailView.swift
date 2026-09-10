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

    private var label: String {
        if isLeft {
            return String(format: "CAM %.0f°", deviceRollDeg)
        } else {
            return String(format: "Δ %.0f°", angleDeg)
        }
    }

    private var isOk: Bool {
        abs(angleDeg) <= (isLeft ? 8 : 10) // green when near vertical
    }

    public var body: some View {
        VStack(spacing: 5) {
            Text(isLeft ? "CAM" : "HAND")
                .font(.system(size: 7, weight: .bold, design: .monospaced))
                .foregroundColor(.white.opacity(0.55))
            // vertical tick
            ZStack {
                // track
                Capsule()
                    .fill(Color.white.opacity(0.12))
                    .frame(width: 2, height: 34)
                // active line
                Capsule()
                    .fill(isOk ? OverlayPalette.emerald : OverlayPalette.amber)
                    .frame(width: 2.5, height: 28)
                    .rotationEffect(.degrees(angleDeg))
                    .shadow(color: (isOk ? OverlayPalette.emerald : OverlayPalette.amber).opacity(0.7), radius: 4)
                // center dot
                Circle()
                    .fill(Color.white.opacity(0.9))
                    .frame(width: 3, height: 3)
            }
            .frame(width: 22, height: 38)
            Text(label)
                .font(.system(size: 8, weight: .bold, design: .monospaced))
                .foregroundColor(isOk ? OverlayPalette.emerald : OverlayPalette.amber)
                .contentTransition(.numericText())
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 7)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.38))
                .overlay(Capsule().stroke(Color.white.opacity(0.14), lineWidth: 1))
        )
    }
}
