import SwiftUI

/**
 * Draws crease splines and optional vertex handles for drag-editing.
 * Points are normalized image coords; `imageRect` is the aspect-fit photo frame.
 * v2: robust hit testing via named coordinate space, 28pt handles, long-press to select.
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
                        context.stroke(path, with: .color(color.opacity(dim ? 0.12 : 0.30)), style: StrokeStyle(lineWidth: selected ? 10 : 7, lineCap: .round, lineJoin: .round))
                        context.stroke(path, with: .color(color.opacity(dim ? 0.42 : 1.0)), style: StrokeStyle(lineWidth: selected ? 3.6 : 2.8, lineCap: .round, lineJoin: .round))
                        if selected && isEditing {
                            context.stroke(path, with: .color(Color.white.opacity(0.22)), style: StrokeStyle(lineWidth: 1, lineCap: .round, lineJoin: .round, dash: [6,4]))
                        }
                    }
                }
                .allowsHitTesting(false)

                if isEditing, let selectedId = selectedSplineId,
                   let spline = splines.first(where: { $0.id == selectedId }) {
                    ForEach(Array(spline.points.enumerated()), id: \.offset) { index, norm in
                        let canvasPt = ImageCoordinateMapper.map(norm, into: rect)
                        let isEnd = index == 0 || index == spline.points.count - 1
                        ZStack {
                            Circle().fill(Color(hex: spline.lineType.hexColor)).frame(width: isEnd ? 28 : 24, height: isEnd ? 28 : 24)
                                .overlay(Circle().stroke(Color.white, lineWidth: 2.5))
                                .shadow(color: .black.opacity(0.45), radius: 4, y: 2)
                            if isEnd { Circle().fill(Color.white).frame(width: 6, height: 6) }
                        }
                        .contentShape(Circle())
                        .position(canvasPt)
                        .gesture(
                            DragGesture(minimumDistance: 2, coordinateSpace: .named("creaseCanvas"))
                                .onChanged { value in
                                    let mapped = ImageCoordinateMapper.unmap(value.location, from: rect)
                                    onMoveVertex?(selectedId, index, mapped)
                                }
                        )
                    }
                }

                if isEditing {
                    Color.clear
                        .contentShape(Rectangle())
                        .simultaneousGesture(
                            DragGesture(minimumDistance: 0, coordinateSpace: .named("creaseCanvas"))
                                .onEnded { value in
                                    if hypot(value.translation.width, value.translation.height) < 12 {
                                        if let id = nearestSplineId(at: value.location, in: rect) {
                                            onSelect?(id)
                                        }
                                    }
                                }
                        )
                }
            }
            .coordinateSpace(name: "creaseCanvas")
        }
    }

    private func nearestSplineId(at viewPoint: CGPoint, in rect: CGRect) -> String? {
        var best: (String, CGFloat)?
        for spline in splines {
            let pts = spline.points.map { ImageCoordinateMapper.map($0, into: rect) }
            if pts.count < 2 { continue }
            for i in 0..<(pts.count-1) {
                let d = distanceToSegment(point: viewPoint, a: pts[i], b: pts[i+1])
                if d < 22, best == nil || d < best!.1 { best = (spline.id, d) }
            }
            for pt in pts {
                let d = hypot(pt.x - viewPoint.x, pt.y - viewPoint.y)
                if d < 28, best == nil || d < best!.1 { best = (spline.id, d) }
            }
        }
        return best?.0
    }

    private func distanceToSegment(point: CGPoint, a: CGPoint, b: CGPoint) -> CGFloat {
        let apx = point.x - a.x; let apy = point.y - a.y
        let abx = b.x - a.x; let aby = b.y - a.y
        let ab2 = abx*abx + aby*aby
        guard ab2 > 1e-6 else { return hypot(apx, apy) }
        var t = (apx*abx + apy*aby) / ab2; t = min(max(t, 0), 1)
        let proj = CGPoint(x: a.x + abx*t, y: a.y + aby*t)
        return hypot(point.x - proj.x, point.y - proj.y)
    }
}

extension Path {
    static func catmullRom(points: [CGPoint], alpha: CGFloat = 0.5) -> Path {
        var path = Path()
        guard points.count >= 2 else { return path }
        _ = alpha
        path.move(to: points[0])
        if points.count == 2 { path.addLine(to: points[1]); return path }
        for i in 0..<(points.count - 1) {
            let p0 = i > 0 ? points[i - 1] : points[i]
            let p1 = points[i]
            let p2 = points[i + 1]
            let p3 = (i + 2 < points.count) ? points[i + 2] : p2
            let cp1 = CGPoint(x: p1.x + (p2.x - p0.x) / 6.0, y: p1.y + (p2.y - p0.y) / 6.0)
            let cp2 = CGPoint(x: p2.x - (p3.x - p1.x) / 6.0, y: p2.y - (p3.y - p1.y) / 6.0)
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
        switch cleanHex.count { case 6: (r, g, b) = ((int >> 16) & 0xFF, (int >> 8) & 0xFF, int & 0xFF); default: (r, g, b) = (255, 255, 255) }
        self.init(.sRGB, red: Double(r) / 255.0, green: Double(g) / 255.0, blue: Double(b) / 255.0, opacity: 1.0)
    }
}
