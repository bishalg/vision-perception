import CoreGraphics
import Foundation

/**
 * Maps normalized image coordinates (0…1, top-left origin) into a view that
 * displays the image with aspect-fit or aspect-fill.
 */
enum ImageCoordinateMapper {
    enum ContentMode {
        case fit
        case fill
    }

    static func imageRect(
        imageSize: CGSize,
        viewSize: CGSize,
        mode: ContentMode
    ) -> CGRect {
        guard imageSize.width > 1, imageSize.height > 1,
              viewSize.width > 1, viewSize.height > 1
        else {
            return CGRect(origin: .zero, size: viewSize)
        }

        let imageAspect = imageSize.width / imageSize.height
        let viewAspect = viewSize.width / viewSize.height

        switch mode {
        case .fit:
            if imageAspect > viewAspect {
                let width = viewSize.width
                let height = width / imageAspect
                return CGRect(
                    x: 0,
                    y: (viewSize.height - height) * 0.5,
                    width: width,
                    height: height
                )
            } else {
                let height = viewSize.height
                let width = height * imageAspect
                return CGRect(
                    x: (viewSize.width - width) * 0.5,
                    y: 0,
                    width: width,
                    height: height
                )
            }
        case .fill:
            if imageAspect > viewAspect {
                let height = viewSize.height
                let width = height * imageAspect
                return CGRect(
                    x: (viewSize.width - width) * 0.5,
                    y: 0,
                    width: width,
                    height: height
                )
            } else {
                let width = viewSize.width
                let height = width / imageAspect
                return CGRect(
                    x: 0,
                    y: (viewSize.height - height) * 0.5,
                    width: width,
                    height: height
                )
            }
        }
    }

    /// Returns the image draw rect transformed by a zoom scale and pan offset,
    /// scaling relative to the view's center.
    static func transformedImageRect(
        imageSize: CGSize,
        viewSize: CGSize,
        mode: ContentMode,
        zoomScale: CGFloat,
        panOffset: CGSize
    ) -> CGRect {
        let base = imageRect(imageSize: imageSize, viewSize: viewSize, mode: mode)
        let cx = viewSize.width * 0.5
        let cy = viewSize.height * 0.5
        let w = base.width * zoomScale
        let h = base.height * zoomScale
        let x = cx + (base.minX - cx) * zoomScale + panOffset.width
        let y = cy + (base.minY - cy) * zoomScale + panOffset.height
        return CGRect(x: x, y: y, width: w, height: h)
    }

    static func map(_ point: CGPoint, into imageRect: CGRect) -> CGPoint {
        CGPoint(
            x: imageRect.minX + point.x * imageRect.width,
            y: imageRect.minY + point.y * imageRect.height
        )
    }

    /// Inverse of `map` — view point → normalized image coords (clamped 0…1).
    static func unmap(_ point: CGPoint, from imageRect: CGRect) -> CGPoint {
        guard imageRect.width > 1, imageRect.height > 1 else { return .zero }
        let nx = (point.x - imageRect.minX) / imageRect.width
        let ny = (point.y - imageRect.minY) / imageRect.height
        return CGPoint(
            x: min(1, max(0, nx)),
            y: min(1, max(0, ny))
        )
    }

    // MARK: - Spline Geometry & Smoothing

    /// 5-tap Gaussian weighted smoothing filter: [0.06, 0.24, 0.40, 0.24, 0.06]
    static func smoothPoints(_ pts: [CGPoint]) -> [CGPoint] {
        guard pts.count > 4 else { return pts }
        var result: [CGPoint] = []
        result.append(pts[0])
        result.append(pts[1])
        for i in 2..<(pts.count - 2) {
            let p0 = pts[i - 2]
            let p1 = pts[i - 1]
            let p2 = pts[i]
            let p3 = pts[i + 1]
            let p4 = pts[i + 2]
            let sx = (p0.x * 0.06) + (p1.x * 0.24) + (p2.x * 0.40) + (p3.x * 0.24) + (p4.x * 0.06)
            let sy = (p0.y * 0.06) + (p1.y * 0.24) + (p2.y * 0.40) + (p3.y * 0.24) + (p4.y * 0.06)
            result.append(CGPoint(x: sx, y: sy))
        }
        result.append(pts[pts.count - 2])
        result.append(pts[pts.count - 1])
        return result
    }

    static func distanceToSegment(point: CGPoint, a: CGPoint, b: CGPoint) -> CGFloat {
        let apx = point.x - a.x
        let apy = point.y - a.y
        let abx = b.x - a.x
        let aby = b.y - a.y
        let ab2 = abx * abx + aby * aby
        guard ab2 > 1e-6 else { return hypot(apx, apy) }
        let t = min(max((apx * abx + apy * aby) / ab2, 0), 1)
        let proj = CGPoint(x: a.x + abx * t, y: a.y + aby * t)
        return hypot(point.x - proj.x, point.y - proj.y)
    }

    static func evaluateSplinePoints(points: [CGPoint], samplesPerSegment: Int = 12) -> [CGPoint] {
        guard points.count >= 2 else { return points }
        if points.count == 2 { return points }
        var result: [CGPoint] = []
        for i in 0..<(points.count - 1) {
            let p0 = i > 0 ? points[i - 1] : points[i]
            let p1 = points[i]
            let p2 = points[i + 1]
            let p3 = (i + 2 < points.count) ? points[i + 2] : p2
            for s in 0..<samplesPerSegment {
                let t = CGFloat(s) / CGFloat(samplesPerSegment)
                let t2 = t * t
                let t3 = t2 * t
                let x = 0.5 * ((2 * p1.x) + (-p0.x + p2.x) * t + (2 * p0.x - 5 * p1.x + 4 * p2.x - p3.x) * t2 + (-p0.x + 3 * p1.x - 3 * p2.x + p3.x) * t3)
                let y = 0.5 * ((2 * p1.y) + (-p0.y + p2.y) * t + (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * t2 + (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * t3)
                result.append(CGPoint(x: x, y: y))
            }
        }
        result.append(points.last!)
        return result
    }

    static func distanceToSpline(point: CGPoint, normPoints: [CGPoint], in rect: CGRect) -> CGFloat {
        let pts = normPoints.map { map($0, into: rect) }
        guard pts.count >= 2 else { return 9999 }
        var minDist: CGFloat = 9999
        let dense = evaluateSplinePoints(points: pts, samplesPerSegment: 12)
        for i in 0..<(dense.count - 1) {
            let d = distanceToSegment(point: point, a: dense[i], b: dense[i + 1])
            if d < minDist { minDist = d }
        }
        return minDist
    }
}
