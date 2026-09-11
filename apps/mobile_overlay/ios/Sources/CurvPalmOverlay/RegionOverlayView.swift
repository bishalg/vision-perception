import CurvCameraOverlay
import SwiftUI

/**
 * Live sci-fi region overlay: dotted mount outlines + optional line-corridor
 * guides, drawn from the 21 Vision landmarks. Dim by design — UI feel only,
 * never a detection claim. See docs/palm_region_overlay_spec.md.
 */
public struct RegionOverlayView: View {
    public let landmarks: [CGPoint]
    public let isReady: Bool
    public var showCorridors: Bool = false
    public var mirror: Bool = false
    public var imageRect: CGRect? = nil

    /// EMA factor against landmark jitter (Vision noise shimmers dashes).
    private static let smoothing: CGFloat = 0.35

    @State private var smoothed: [CGPoint] = []

    public init(
        landmarks: [CGPoint],
        isReady: Bool,
        showCorridors: Bool = false,
        mirror: Bool = false,
        imageRect: CGRect? = nil
    ) {
        self.landmarks = landmarks
        self.isReady = isReady
        self.showCorridors = showCorridors
        self.mirror = mirror
        self.imageRect = imageRect
    }

    public var body: some View {
        Canvas { context, size in
            let pts = smoothed.count >= 21 ? smoothed : landmarks
            guard let regions = PalmRegionGeometry.regions(from: pts) else { return }
            let rect = imageRect ?? CGRect(origin: .zero, size: size)
            func map(_ p: CGPoint) -> CGPoint {
                var q = CGPoint(x: rect.minX + p.x * rect.width, y: rect.minY + p.y * rect.height)
                if mirror { q.x = rect.maxX - (q.x - rect.minX) }
                return q
            }

            let base: Color = isReady ? OverlayPalette.emerald : Color.white
            let mountStroke = base.opacity(isReady ? 0.55 : 0.34)
            let mountFill = base.opacity(0.07)
            let corridorStroke = base.opacity(isReady ? 0.5 : 0.3)
            let echoStroke = base.opacity(isReady ? 0.22 : 0.14)

            for (_, poly) in regions.mounts {
                let mapped = poly.map(map)
                guard mapped.count >= 3 else { continue }
                var path = Path()
                path.move(to: mapped[0])
                for pt in mapped.dropFirst() { path.addLine(to: pt) }
                path.closeSubpath()
                context.fill(path, with: .color(mountFill))
                context.stroke(
                    path, with: .color(mountStroke),
                    style: StrokeStyle(lineWidth: 1.0, lineCap: .round, lineJoin: .round, dash: [2, 5])
                )
            }

            if showCorridors {
                for (_, center, echoes) in regions.corridors {
                    for echo in echoes {
                        context.stroke(
                            polyline(echo.map(map)), with: .color(echoStroke),
                            style: StrokeStyle(lineWidth: 0.6, lineCap: .round, dash: [1, 6])
                        )
                    }
                    context.stroke(
                        polyline(center.map(map)), with: .color(corridorStroke),
                        style: StrokeStyle(lineWidth: 1.0, lineCap: .round, dash: [5, 4])
                    )
                }
            }
        }
        .allowsHitTesting(false)
        .onChange(of: landmarks) { next in
            guard next.count >= 21 else {
                if smoothed.count >= 21 { smoothed.removeAll() }
                return
            }
            if smoothed.count == next.count {
                let a = Self.smoothing
                smoothed = zip(smoothed, next).map { prev, cur in
                    CGPoint(x: prev.x + (cur.x - prev.x) * a, y: prev.y + (cur.y - prev.y) * a)
                }
            } else {
                smoothed = next
            }
        }
    }

    private func polyline(_ pts: [CGPoint]) -> Path {
        var path = Path()
        guard let first = pts.first else { return path }
        path.move(to: first)
        for pt in pts.dropFirst() { path.addLine(to: pt) }
        return path
    }
}
