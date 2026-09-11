import CoreGraphics
import Foundation

/**
 * Landmark-anchored palm region geometry for the live overlay.
 * Pure math, no UI: 21 MediaPipe-order points in normalized space
 * (y-down, origin top-left) → 7 mount polygons + 3 line-corridor guides.
 *
 * Anchors mirror `HandAnatomyDecomposer` (digit rays + basal MCP crease)
 * and the future C++ `PalmRegionMap`; keep the fractions in sync.
 * See docs/palm_region_overlay_spec.md §2.
 */
public enum PalmMount: String, CaseIterable {
    case jupiter, saturn, apollo, mercury, venus, luna, mars
}

public enum PalmCorridor: String, CaseIterable {
    case heart, head, life
}

public struct PalmRegions {
    /// Closed polygons (renderer closes the path).
    public let mounts: [(mount: PalmMount, polygon: [CGPoint])]
    /// Open guide polylines + two faint parallel echoes each.
    public let corridors: [(corridor: PalmCorridor, center: [CGPoint], echoes: [[CGPoint]])]

    public var mountPolygon: (PalmMount) -> [CGPoint] {
        { [mounts] m in mounts.first { $0.mount == m }?.polygon ?? [] }
    }
}

public enum PalmRegionGeometry {
    // MARK: - primitives

    static func lerp(_ a: CGPoint, _ b: CGPoint, _ t: CGFloat) -> CGPoint {
        CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t)
    }

    static func mid(_ a: CGPoint, _ b: CGPoint) -> CGPoint { lerp(a, b, 0.5) }

    /// Point dropped from an MCP toward the wrist: unit palm-depth.
    static func depth(_ mcp: CGPoint, _ wrist: CGPoint, _ t: CGFloat) -> CGPoint {
        lerp(mcp, wrist, t)
    }

    // MARK: - entry

    /// Builds regions from 21 normalized landmarks. Nil when input is incomplete.
    public static func regions(from landmarks: [CGPoint]) -> PalmRegions? {
        guard landmarks.count >= 21 else { return nil }
        let wrist = landmarks[0]
        let thumbCmc = landmarks[1]
        let thumbMcp = landmarks[2]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let ringMcp = landmarks[13]
        let pinkyMcp = landmarks[17]

        let idxMid = mid(indexMcp, middleMcp)
        let midMid = mid(middleMcp, ringMcp)
        let ringMid = mid(ringMcp, pinkyMcp)

        // ulnar-outward unit (pinky side, away from palm center)
        let ux = pinkyMcp.x - indexMcp.x, uy = pinkyMcp.y - indexMcp.y
        let ulen = max(1e-4, sqrt(ux * ux + uy * uy))
        let uw = ulen * 0.38
        let ox = ux / ulen * uw, oy = uy / ulen * uw
        func push(_ p: CGPoint) -> CGPoint { CGPoint(x: p.x + ox, y: p.y + oy) }

        // Four finger mounts: MCP row on top, 0.22 palm-depth foot.
        func fingerMount(topA: CGPoint, topB: CGPoint, outward: Bool = false) -> [CGPoint] {
            let a = outward ? push(topA) : topA
            let b = outward ? push(topB) : topB
            return [a, b, depth(b, wrist, 0.22), depth(a, wrist, 0.22)]
        }

        let venus: [CGPoint] = [
            thumbCmc,
            thumbMcp,
            lerp(thumbMcp, wrist, 0.35),
            lerp(wrist, indexMcp, 0.35),
            lerp(indexMcp, thumbMcp, 0.45),
        ]
        let lunaTop = lerp(wrist, pinkyMcp, 0.55)
        let luna: [CGPoint] = [lunaTop, push(lunaTop), push(wrist), wrist]
        let mars: [CGPoint] = [
            depth(middleMcp, wrist, 0.40), depth(pinkyMcp, wrist, 0.40),
            push(depth(pinkyMcp, wrist, 0.58)), push(depth(middleMcp, wrist, 0.58)),
        ]

        let mounts: [(PalmMount, [CGPoint])] = [
            (.jupiter, fingerMount(topA: indexMcp, topB: idxMid)),
            (.saturn, fingerMount(topA: idxMid, topB: midMid)),
            (.apollo, fingerMount(topA: midMid, topB: ringMid)),
            (.mercury, fingerMount(topA: ringMid, topB: pinkyMcp, outward: true)),
            (.venus, venus),
            (.luna, luna),
            (.mars, mars),
        ]

        let lifeOrigin = lerp(indexMcp, thumbMcp, 0.45)
        let heart: [CGPoint] = [
            depth(pinkyMcp, wrist, 0.06), depth(ringMcp, wrist, 0.10),
            depth(middleMcp, wrist, 0.12), depth(indexMcp, wrist, 0.10),
        ]
        let head: [CGPoint] = [
            lerp(lifeOrigin, wrist, 0.18),
            depth(indexMcp, wrist, 0.32), depth(middleMcp, wrist, 0.36),
            depth(ringMcp, wrist, 0.34), depth(pinkyMcp, wrist, 0.30),
        ]
        let life: [CGPoint] = [
            lerp(lifeOrigin, wrist, 0.08),
            lerp(thumbMcp, indexMcp, 0.25),
            lerp(thumbMcp, wrist, 0.35),
            lerp(landmarks[3], wrist, 0.55),
            lerp(thumbCmc, wrist, 0.72),
        ]

        func echoes(_ pts: [CGPoint]) -> [[CGPoint]] {
            let inEcho = pts.map { lerp($0, wrist, 0.05) }
            let outEcho = pts.map { p -> CGPoint in
                let dx = p.x - wrist.x, dy = p.y - wrist.y
                let len = max(1e-4, sqrt(dx * dx + dy * dy))
                let step = len * 0.05
                return CGPoint(x: p.x + dx / len * step, y: p.y + dy / len * step)
            }
            return [inEcho, outEcho]
        }

        return PalmRegions(
            mounts: mounts,
            corridors: [
                (.heart, heart, echoes(heart)),
                (.head, head, echoes(head)),
                (.life, life, echoes(life)),
            ]
        )
    }

    // MARK: - simulator scaffold

    /// Synthetic centered right hand (normalized) for simulator / previews.
    public static func scaffoldLandmarks() -> [CGPoint] {
        let cx: CGFloat = 0.5
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: cx + x, y: y) }
        return [
            p(0.00, 0.86),                       // 0 wrist
            p(-0.20, 0.72), p(-0.24, 0.60), p(-0.30, 0.50), p(-0.34, 0.42), // 1-4 thumb
            p(-0.13, 0.44), p(-0.14, 0.30), p(-0.14, 0.20), p(-0.14, 0.12), // 5-8 index
            p(-0.01, 0.42), p(-0.01, 0.26), p(-0.01, 0.15), p(-0.01, 0.06), // 9-12 middle
            p(0.11, 0.44), p(0.12, 0.29), p(0.12, 0.19), p(0.13, 0.11),     // 13-16 ring
            p(0.21, 0.48), p(0.24, 0.37), p(0.26, 0.29), p(0.27, 0.23),     // 17-20 pinky
        ]
    }

    public static func scaffoldRegions() -> PalmRegions {
        // Scaffold is well-formed by construction; fall back to empty, never nil.
        regions(from: scaffoldLandmarks()) ?? PalmRegions(mounts: [], corridors: [])
    }
}
