import CurvCameraOverlay
import SwiftUI

/**
 * Beautiful open-hand guide — glassy fill + gradient stroke,
 * fingertip highlights, faint crease hints, pulsing when searching.
 * Mirrors horizontally for left hand so thumb is on the correct side.
 */
public struct PalmSilhouetteView: View {
    public let isReady: Bool
    public var handSide: HandSide = .auto
    public var detectedIsLeft: Bool? = nil
    /// When true (hand is detected by vision tracker), the static silhouette guide smoothly fades out.
    public var isHandDetected: Bool = false

    // resolved handedness for mirroring
    private var mirror: Bool {
        switch handSide {
        case .left: return true
        case .right: return false
        case .auto: return detectedIsLeft ?? false
        }
    }

    public init(
        isReady: Bool,
        handSide: HandSide = .auto,
        detectedIsLeft: Bool? = nil,
        isHandDetected: Bool = false
    ) {
        self.isReady = isReady
        self.handSide = handSide
        self.detectedIsLeft = detectedIsLeft
        self.isHandDetected = isHandDetected
    }

    public var body: some View {
        GeometryReader { proxy in
            let fillBase: Color = isReady ? OverlayPalette.emerald.opacity(0.13) : Color.white.opacity(0.05)
            let stroke: Color = isReady ? OverlayPalette.emerald : Color.white.opacity(0.38)

            ZStack {
                // outer glow
                FullHandGuideShape()
                    .fill(
                        RadialGradient(
                            colors: [fillBase, Color.clear],
                            center: .center,
                            startRadius: 10,
                            endRadius: proxy.size.width * 0.62
                        )
                    )
                    .blur(radius: isReady ? 14 : 9)
                    .scaleEffect(isReady ? 1.02 : 1.0)

                FullHandGuideShape()
                    .fill(
                        LinearGradient(
                            colors: [Color.white.opacity(isReady ? 0.14 : 0.08), fillBase],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    )
                    .overlay(
                        // faint palm crease hints inside the silhouette
                        HintCreases()
                            .stroke(Color.white.opacity(0.14), style: StrokeStyle(lineWidth: 0.9, lineCap: .round, dash: [5,5]))
                            .padding(proxy.size.width * 0.10)
                    )

                FullHandGuideShape()
                    .stroke(
                        LinearGradient(
                            colors: [Color.white.opacity(0.72), stroke, Color.white.opacity(0.42)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        style: StrokeStyle(lineWidth: isReady ? 2.8 : 2.1, lineCap: .round, lineJoin: .round)
                    )
                    .shadow(color: stroke.opacity(isReady ? 0.55 : 0.18), radius: isReady ? 14 : 8, y: 2)

                // wrist bracelet subtle arc
                WristBraceShape()
                    .stroke(Color.white.opacity(0.26), style: StrokeStyle(lineWidth: 1.2, lineCap: .round, dash: [3,4]))
                    .padding(.horizontal, proxy.size.width * 0.12)

                // fingertip + knuckle sparkles
                FingertipDots()
                    .fill(Color.white.opacity(isReady ? 0.92 : 0.62))
                    .shadow(color: Color.white.opacity(0.65), radius: 2.2)
            }
            .scaleEffect(x: mirror ? -1 : 1, y: 1, anchor: .center)
            .padding(.horizontal, proxy.size.width * 0.055)
            .padding(.vertical, proxy.size.height * 0.038)
            .frame(width: proxy.size.width, height: proxy.size.height)
            .scaleEffect(isReady ? 1.0 : 0.985)
            .animation(.easeInOut(duration: 0.55).repeatForever(autoreverses: true), value: isReady)
            // Dynamic helper: smoothly disappears once user's real hand is detected
            .opacity(isHandDetected ? 0.0 : 1.0)
            .animation(.easeInOut(duration: 0.35), value: isHandDetected)
        }
    }
}

/// Faint hint of heart/head/life creases inside the palm — purely decorative guide.
private struct HintCreases: Shape {
    func path(in rect: CGRect) -> Path {
        let w = rect.width; let h = rect.height
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x*w, y: rect.minY + y*h) }
        var path = Path()
        // heart-like top arc
        path.move(to: p(0.22, 0.42))
        path.addCurve(to: p(0.78, 0.40), control1: p(0.32,0.34), control2: p(0.64,0.34))
        // head-like mid
        path.move(to: p(0.20, 0.52))
        path.addCurve(to: p(0.76, 0.51), control1: p(0.34,0.48), control2: p(0.62,0.48))
        // life-like thumb wrap
        path.move(to: p(0.36, 0.46))
        path.addCurve(to: p(0.38, 0.76), control1: p(0.28,0.56), control2: p(0.26,0.68))
        return path
    }
}
private struct WristBraceShape: Shape {
    func path(in rect: CGRect) -> Path {
        let w = rect.width; let h = rect.height
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x*w, y: rect.minY + y*h) }
        var path = Path()
        path.move(to: p(0.28, 0.88))
        path.addCurve(to: p(0.72, 0.88), control1: p(0.38,0.90), control2: p(0.62,0.90))
        return path
    }
}
private struct FingertipDots: Shape {
    func path(in rect: CGRect) -> Path {
        let w = rect.width; let h = rect.height
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x*w, y: rect.minY + y*h) }
        // Anatomically aligned: thumb tip, index tip, middle tip, ring tip, pinky tip
        let tips: [CGPoint] = [p(0.12,0.35), p(0.38,0.11), p(0.52,0.04), p(0.66,0.09), p(0.81,0.18)]
        // MCP knuckles: thumb MCP, index MCP, middle MCP, ring MCP, pinky MCP
        let mcp: [CGPoint] = [p(0.22,0.58), p(0.38,0.42), p(0.51,0.38), p(0.63,0.40), p(0.74,0.44)]
        var path = Path()
        for pt in tips + mcp {
            path.addEllipse(in: CGRect(x: pt.x-1.7, y: pt.y-1.7, width: 3.4, height: 3.4))
        }
        // palm center glow dot
        path.addEllipse(in: CGRect(x: rect.midX-2.2, y: rect.midY+rect.height*0.06-2.2, width: 4.4, height: 4.4))
        return path
    }
}
/// Normalized open right-hand outline (fingers up, wrist down) filling its bounds.
/// Features anatomically correct outward-angled thumb, web space, and natural digit curves.
private struct FullHandGuideShape: Shape {
    func path(in rect: CGRect) -> Path {
        let w = rect.width
        let h = rect.height
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
            CGPoint(x: rect.minX + x * w, y: rect.minY + y * h)
        }
        var path = Path()
        // Wrist base
        path.move(to: p(0.28, 0.92))
        path.addCurve(to: p(0.72, 0.92), control1: p(0.38, 0.98), control2: p(0.62, 0.98))
        // Ulnar side (pinky / hypothenar)
        path.addCurve(to: p(0.78, 0.62), control1: p(0.80, 0.86), control2: p(0.82, 0.72))
        // Pinky lateral edge & tip
        path.addCurve(to: p(0.86, 0.18), control1: p(0.84, 0.50), control2: p(0.90, 0.32))
        path.addCurve(to: p(0.78, 0.16), control1: p(0.85, 0.12), control2: p(0.80, 0.12))
        path.addCurve(to: p(0.74, 0.42), control1: p(0.76, 0.24), control2: p(0.74, 0.34))
        // Ring finger
        path.addCurve(to: p(0.70, 0.10), control1: p(0.74, 0.28), control2: p(0.74, 0.16))
        path.addCurve(to: p(0.62, 0.08), control1: p(0.68, 0.04), control2: p(0.64, 0.04))
        path.addCurve(to: p(0.60, 0.38), control1: p(0.60, 0.16), control2: p(0.60, 0.28))
        // Middle finger
        path.addCurve(to: p(0.56, 0.04), control1: p(0.60, 0.22), control2: p(0.60, 0.10))
        path.addCurve(to: p(0.48, 0.04), control1: p(0.54, 0.00), control2: p(0.50, 0.00))
        path.addCurve(to: p(0.46, 0.36), control1: p(0.46, 0.12), control2: p(0.46, 0.24))
        // Index finger
        path.addCurve(to: p(0.42, 0.10), control1: p(0.46, 0.22), control2: p(0.46, 0.14))
        path.addCurve(to: p(0.34, 0.12), control1: p(0.40, 0.04), control2: p(0.36, 0.04))
        path.addCurve(to: p(0.34, 0.44), control1: p(0.32, 0.22), control2: p(0.32, 0.34))
        // ── Anatomically Correct Thumb ──
        // First web space: smooth concave cleft between index and thumb
        path.addCurve(to: p(0.27, 0.47), control1: p(0.32, 0.46), control2: p(0.30, 0.47))
        // Inner thumb margin: rises outward & upward toward thumb tip
        path.addCurve(to: p(0.18, 0.38), control1: p(0.24, 0.46), control2: p(0.20, 0.41))
        // Rounded thumb tip: angled outward ~35°
        path.addCurve(to: p(0.10, 0.35), control1: p(0.15, 0.33), control2: p(0.11, 0.33))
        // Outer thumb edge: distal phalanx down through interphalangeal joint
        path.addCurve(to: p(0.13, 0.46), control1: p(0.09, 0.38), control2: p(0.11, 0.42))
        // Thumb metacarpal outer contour
        path.addCurve(to: p(0.17, 0.58), control1: p(0.13, 0.50), control2: p(0.15, 0.54))
        // Thenar eminence (ball of the thumb) sweeping into wrist
        path.addCurve(to: p(0.23, 0.74), control1: p(0.19, 0.64), control2: p(0.21, 0.69))
        path.addCurve(to: p(0.28, 0.92), control1: p(0.25, 0.81), control2: p(0.26, 0.88))
        path.closeSubpath()
        return path
    }
}
