import CurvCameraOverlay
import SwiftUI

/**
 * Large open-hand guide that fills the capture reticle.
 * Shows full palm + fingers so users frame the entire hand, not a close crop.
 */
public struct PalmSilhouetteView: View {
    public let isReady: Bool

    public init(isReady: Bool) {
        self.isReady = isReady
    }

    public var body: some View {
        GeometryReader { proxy in
            let stroke = isReady
                ? OverlayPalette.emerald.opacity(0.55)
                : Color.white.opacity(0.28)
            let fill = isReady
                ? OverlayPalette.emerald.opacity(0.12)
                : Color.white.opacity(0.06)

            ZStack {
                FullHandGuideShape()
                    .fill(fill)
                FullHandGuideShape()
                    .stroke(stroke, style: StrokeStyle(lineWidth: 2.5, lineCap: .round, lineJoin: .round))
            }
            .padding(.horizontal, proxy.size.width * 0.06)
            .padding(.vertical, proxy.size.height * 0.04)
            .frame(width: proxy.size.width, height: proxy.size.height)
        }
    }
}

/// Normalized open right-hand outline (fingers up, wrist down) filling its bounds.
private struct FullHandGuideShape: Shape {
    func path(in rect: CGRect) -> Path {
        let w = rect.width
        let h = rect.height
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
            CGPoint(x: rect.minX + x * w, y: rect.minY + y * h)
        }

        var path = Path()
        // Wrist / palm base (bottom)
        path.move(to: p(0.28, 0.92))
        path.addCurve(to: p(0.72, 0.92), control1: p(0.38, 0.98), control2: p(0.62, 0.98))

        // Pinky side up
        path.addCurve(to: p(0.78, 0.62), control1: p(0.80, 0.86), control2: p(0.82, 0.72))
        // Pinky finger
        path.addCurve(to: p(0.86, 0.18), control1: p(0.84, 0.50), control2: p(0.90, 0.32))
        path.addCurve(to: p(0.78, 0.16), control1: p(0.85, 0.12), control2: p(0.80, 0.12))
        path.addCurve(to: p(0.74, 0.42), control1: p(0.76, 0.24), control2: p(0.74, 0.34))

        // Ring finger
        path.addCurve(to: p(0.70, 0.10), control1: p(0.74, 0.28), control2: p(0.74, 0.16))
        path.addCurve(to: p(0.62, 0.08), control1: p(0.68, 0.04), control2: p(0.64, 0.04))
        path.addCurve(to: p(0.60, 0.38), control1: p(0.60, 0.16), control2: p(0.60, 0.28))

        // Middle finger (tallest)
        path.addCurve(to: p(0.56, 0.04), control1: p(0.60, 0.22), control2: p(0.60, 0.10))
        path.addCurve(to: p(0.48, 0.04), control1: p(0.54, 0.00), control2: p(0.50, 0.00))
        path.addCurve(to: p(0.46, 0.36), control1: p(0.46, 0.12), control2: p(0.46, 0.24))

        // Index finger
        path.addCurve(to: p(0.42, 0.10), control1: p(0.46, 0.22), control2: p(0.46, 0.14))
        path.addCurve(to: p(0.34, 0.12), control1: p(0.40, 0.04), control2: p(0.36, 0.04))
        path.addCurve(to: p(0.34, 0.44), control1: p(0.32, 0.22), control2: p(0.32, 0.34))

        // Thumb (side)
        path.addCurve(to: p(0.18, 0.48), control1: p(0.28, 0.46), control2: p(0.22, 0.44))
        path.addCurve(to: p(0.10, 0.58), control1: p(0.12, 0.50), control2: p(0.08, 0.52))
        path.addCurve(to: p(0.16, 0.66), control1: p(0.10, 0.64), control2: p(0.14, 0.68))
        path.addCurve(to: p(0.28, 0.72), control1: p(0.20, 0.64), control2: p(0.24, 0.66))

        // Close along palm edge to wrist
        path.addCurve(to: p(0.28, 0.92), control1: p(0.26, 0.80), control2: p(0.24, 0.88))
        path.closeSubpath()
        return path
    }
}
