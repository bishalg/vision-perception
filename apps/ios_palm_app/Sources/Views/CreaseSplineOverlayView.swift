import SwiftUI

/**
 * Draws crease splines and optional vertex handles for drag-editing.
 * Points are normalized image coords; `imageRect` is the aspect-fit photo frame.
 */
public struct CreaseSplineOverlayView: View {
    public let splines: [CreaseSpline]
    public let imageRect: CGRect?
    public var selectedSplineId: String?
    public var isEditing: Bool
    public var onSelect: ((String) -> Void)?
    public var onMoveVertex: ((String, Int, CGPoint) -> Void)?

    public init(
        splines: [CreaseSpline],
        imageRect: CGRect? = nil,
        selectedSplineId: String? = nil,
        isEditing: Bool = false,
        onSelect: ((String) -> Void)? = nil,
        onMoveVertex: ((String, Int, CGPoint) -> Void)? = nil
    ) {
        self.splines = splines
        self.imageRect = imageRect
        self.selectedSplineId = selectedSplineId
        self.isEditing = isEditing
        self.onSelect = onSelect
        self.onMoveVertex = onMoveVertex
    }

    public var body: some View {
        GeometryReader { proxy in
            let rect = imageRect ?? CGRect(origin: .zero, size: proxy.size)
            ZStack {
                Canvas { context, _ in
                    for spline in splines {
                        guard spline.points.count >= 2 else { continue }
                        let canvasPoints = spline.points.map { ImageCoordinateMapper.map($0, into: rect) }
                        let path = Path.catmullRom(points: canvasPoints)
                        let color = Color(hex: spline.lineType.hexColor)
                        let selected = spline.id == selectedSplineId
                        let dim = isEditing && selectedSplineId != nil && !selected

                        context.stroke(
                            path,
                            with: .color(color.opacity(dim ? 0.15 : 0.4)),
                            style: StrokeStyle(lineWidth: selected ? 8 : 6, lineCap: .round, lineJoin: .round)
                        )
                        context.stroke(
                            path,
                            with: .color(color.opacity(dim ? 0.35 : 1.0)),
                            style: StrokeStyle(lineWidth: selected ? 3.2 : 2.5, lineCap: .round, lineJoin: .round)
                        )
                    }
                }
                .allowsHitTesting(false)

                if isEditing, let selectedId = selectedSplineId,
                   let spline = splines.first(where: { $0.id == selectedId }) {
                    ForEach(Array(spline.points.enumerated()), id: \.offset) { index, norm in
                        let canvasPt = ImageCoordinateMapper.map(norm, into: rect)
                        Circle()
                            .fill(Color(hex: spline.lineType.hexColor))
                            .frame(width: 18, height: 18)
                            .overlay(Circle().stroke(Color.white, lineWidth: 2))
                            .shadow(color: .black.opacity(0.35), radius: 3, y: 1)
                            .position(canvasPt)
                            .gesture(
                                DragGesture(minimumDistance: 0)
                                    .onChanged { value in
                                        let mapped = ImageCoordinateMapper.unmap(value.location, from: rect)
                                        onMoveVertex?(selectedId, index, mapped)
                                    }
                            )
                    }
                }

                // Tap empty areas / lines to select (approximate via nearest vertex).
                if isEditing {
                    Color.clear
                        .contentShape(Rectangle())
                        .gesture(
                            DragGesture(minimumDistance: 0)
                                .onEnded { value in
                                    if let id = nearestSplineId(at: value.location, in: rect) {
                                        onSelect?(id)
                                    }
                                }
                        )
                        .zIndex(-1)
                }
            }
        }
    }

    private func nearestSplineId(at viewPoint: CGPoint, in rect: CGRect) -> String? {
        var best: (String, CGFloat)?
        for spline in splines {
            for pt in spline.points {
                let mapped = ImageCoordinateMapper.map(pt, into: rect)
                let d = hypot(mapped.x - viewPoint.x, mapped.y - viewPoint.y)
                if d < 36, best == nil || d < best!.1 {
                    best = (spline.id, d)
                }
            }
        }
        return best?.0
    }
}

extension Path {
    /**
     * Builds a smooth continuous spline passing through all control points.
     */
    static func catmullRom(points: [CGPoint], alpha: CGFloat = 0.5) -> Path {
        var path = Path()
        guard points.count >= 2 else { return path }
        _ = alpha

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
