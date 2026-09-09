import SwiftUI

/**
 * High-performance vector canvas rendering smooth anti-aliased Catmull-Rom crease splines.
 */
public struct CreaseSplineOverlayView: View {
    public let splines: [CreaseSpline]

    public init(splines: [CreaseSpline]) {
        self.splines = splines
    }

    public var body: some View {
        Canvas { context, size in
            for spline in splines {
                guard spline.points.count >= 2 else { continue }

                // Map normalized coordinates (0..1) to view canvas dimensions
                let canvasPoints = spline.points.map { pt in
                    CGPoint(x: pt.x * size.width, y: pt.y * size.height)
                }

                let path = Path.catmullRom(points: canvasPoints)
                let color = Color(hex: spline.lineType.hexColor)

                // 1. Outer neon glow layer
                context.stroke(
                    path,
                    with: .color(color.opacity(0.4)),
                    style: StrokeStyle(lineWidth: 6, lineCap: .round, lineJoin: .round)
                )

                // 2. Crisp core vector spline
                context.stroke(
                    path,
                    with: .color(color),
                    style: StrokeStyle(lineWidth: 2.5, lineCap: .round, lineJoin: .round)
                )
            }
        }
        .allowsHitTesting(false)
    }
}

extension Path {
    /**
     * Builds a smooth continuous spline passing through all control points.
     */
    static func catmullRom(points: [CGPoint], alpha: CGFloat = 0.5) -> Path {
        var path = Path()
        guard points.count >= 2 else { return path }

        path.move(to: points[0])
        if points.count == 2 {
            path.addLine(to: points[1])
            return path
        }

        for i in 0..<(points.count - 1) {
            let p0 = i > 0 ? points[i - 1] : points[i]
            let p1 = points[i]
            let p2 = points[i + 1]
            let p3 = (i + 2 < points.count) ? points[i + 2] : p2

            let cp1 = CGPoint(
                x: p1.x + (p2.x - p0.x) / 6.0,
                y: p1.y + (p2.y - p0.y) / 6.0
            )
            let cp2 = CGPoint(
                x: p2.x - (p3.x - p1.x) / 6.0,
                y: p2.y - (p3.y - p1.y) / 6.0
            )

            path.addCurve(to: p2, control1: cp1, control2: cp2)
        }

        return path
    }
}

extension Color {
    init(hex: String) {
        let cleanHex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: cleanHex).scanHexInt64(&int)
        let r, g, b: UInt64
        switch cleanHex.count {
        case 6:
            (r, g, b) = ((int >> 16) & 0xFF, (int >> 8) & 0xFF, int & 0xFF)
        default:
            (r, g, b) = (255, 255, 255)
        }
        self.init(
            .sRGB,
            red: Double(r) / 255.0,
            green: Double(g) / 255.0,
            blue: Double(b) / 255.0,
            opacity: 1.0
        )
    }
}
