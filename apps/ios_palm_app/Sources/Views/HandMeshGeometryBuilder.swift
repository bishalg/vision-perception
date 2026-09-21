import Foundation
import CoreGraphics
import SceneKit
import simd

// MARK: - Result Types

public struct HandMeshGeometryResult {
    public struct Vertex3D {
        public var x: Float
        public var y: Float
        public var z: Float
        public init(x: Float, y: Float, z: Float) {
            self.x = x; self.y = y; self.z = z
        }
    }

    public let vertices: [Vertex3D]
    public let colors: [Float]
    public let indices: [Int32]
    public let landmark3DPositions: [SCNVector3]
}

// MARK: - Hand Mesh Geometry Builder

/// Anatomical hand mesh builder producing a watertight, proportional, scan-accurate 3D surface.
///
/// Design goals:
///  • Fingers: 8-sided elliptical cross-section per joint — matches real finger oval cross-section.
///  • Palm: anatomically-weighted fan with thenar/hypothenar mounds and transverse arch.
///  • Elevation: clamped to ±4.5mm, scaled per mount; finger tips taper smoothly to rounded apex.
///  • Colors: continuous elevation heatmap on palm + skin-tone gradient on finger surfaces.
///  • Orientation: auto-upright so fingers always point to +Y regardless of capture tilt.
public enum HandMeshGeometryBuilder {

    // MARK: - Constants

    /// Number of sides in each finger cross-section ring (8 = smooth ellipse without overdraw cost).
    private static let kRingSides = 8

    // MARK: - Helpers

    private static func clamp(_ v: Float, min lo: Float, max hi: Float) -> Float {
        Swift.max(lo, Swift.min(hi, v))
    }

    /// Elevation colormap: deep blue (hollow) → cyan → emerald → amber → coral (prominent mound).
    public static func elevationColor(t: Float) -> (Float, Float, Float) {
        let c = clamp(t, min: 0.0, max: 1.0)
        switch c {
        case ..<0.25:
            let u = c / 0.25
            return (lerp(0.12, 0.08, u), lerp(0.40, 0.78, u), lerp(0.95, 0.88, u))
        case ..<0.50:
            let u = (c - 0.25) / 0.25
            return (lerp(0.08, 0.22, u), lerp(0.78, 0.86, u), lerp(0.88, 0.38, u))
        case ..<0.75:
            let u = (c - 0.50) / 0.25
            return (lerp(0.22, 0.95, u), lerp(0.86, 0.78, u), lerp(0.38, 0.18, u))
        default:
            let u = (c - 0.75) / 0.25
            return (lerp(0.95, 0.98, u), lerp(0.78, 0.30, u), lerp(0.18, 0.22, u))
        }
    }

    /// Skin-tone gradient for finger surfaces: maps 0 (joint crease) → 1 (mid-phalanx).
    private static func fingerSkinColor(t: Float) -> (Float, Float, Float) {
        let c = clamp(t, min: 0.0, max: 1.0)
        // Warm teal-green scan aesthetic with slight elevation difference crease-to-top
        let r = lerp(0.18, 0.30, c)
        let g = lerp(0.72, 0.90, c)
        let b = lerp(0.58, 0.72, c)
        return (r, g, b)
    }

    private static func lerp(_ a: Float, _ b: Float, _ t: Float) -> Float {
        a + (b - a) * t
    }

    // MARK: - Public API

    public static func buildMeshGeometry(
        landmarks: [CGPoint],
        padDepths: [String: Float],
        referencePalmWidthMm: Float = 80.0,
        elevationScale: Float = 1.0
    ) -> HandMeshGeometryResult {
        guard landmarks.count >= 21 else {
            return HandMeshGeometryResult(vertices: [], colors: [], indices: [], landmark3DPositions: [])
        }

        // MARK: 1. Coordinate Transform: upright + metric

        let p0 = landmarks[0]   // wrist
        let p9 = landmarks[9]   // middle MCP
        let p5 = landmarks[5]   // index MCP
        let p17 = landmarks[17] // pinky MCP

        // Pivot on anatomical palm center (between wrist and middle MCP)
        let pivot = CGPoint(x: (p0.x + p9.x) * 0.5, y: (p0.y + p9.y) * 0.5)

        // Rotation angle to make fingers point straight up (+Y)
        let dirX = Float(p9.x - p0.x)
        let dirY = Float(p9.y - p0.y)
        let upAngle = atan2(dirX, -dirY)

        // Pixel breadth of knuckle row → physical scale (80 mm reference)
        let knucklePx = hypot(Float(p17.x - p5.x), Float(p17.y - p5.y))
        let metricScale: Float = knucklePx > 0.01 ? (0.080 / knucklePx) : 0.18

        let cosA = cos(-upAngle)
        let sinA = sin(-upAngle)

        let transform: (CGPoint) -> (Float, Float) = { pt in
            let dx = Float(pt.x - pivot.x)
            let dy = Float(pt.y - pivot.y)
            let rx = cosA * dx - sinA * dy
            let ry = sinA * dx + cosA * dy
            return (rx * metricScale, -ry * metricScale) // flip Y: image→3D up
        }

        var vertices: [HandMeshGeometryResult.Vertex3D] = []
        var colors: [Float] = []
        var landmark3D: [SCNVector3] = []

        // MARK: 2. 21 Landmark Vertices with Anatomically-Assigned Elevations

        // Elevation assignment: each landmark maps to its nearest mount or finger segment
        let elevationMm: (Int) -> Float = { idx in
            switch idx {
            case 0:  // wrist — slightly concave
                return clamp(padDepths["wrist"] ?? -0.3, min: -1.5, max: 0.5)
            case 1, 2:  // thumb metacarpal → thenar mount
                return clamp(padDepths["thenar"] ?? padDepths["venus"] ?? 2.6, min: -2.5, max: 4.0)
            case 3, 4:  // thumb phalanges — taper from mount
                return clamp(padDepths["thenar"] ?? 2.0, min: -2.5, max: 4.0) * (idx == 3 ? 0.7 : 0.4)
            case 5, 6:  // index MCP → Jupiter mount
                return clamp(padDepths["radial_interdigital"] ?? padDepths["jupiter"] ?? 1.8, min: -2.5, max: 4.0)
            case 7, 8:  // index phalanges
                return lerp(clamp(padDepths["jupiter"] ?? 1.8, min: -2.5, max: 4.0), 0.3, idx == 7 ? 0.4 : 0.7)
            case 9, 10:  // middle MCP → Saturn mount
                return clamp(padDepths["central_interdigital"] ?? padDepths["saturn"] ?? 1.5, min: -2.5, max: 4.0)
            case 11, 12:  // middle phalanges
                return lerp(clamp(padDepths["saturn"] ?? 1.5, min: -2.5, max: 4.0), 0.3, idx == 11 ? 0.4 : 0.7)
            case 13, 14:  // ring MCP → Apollo mount
                return clamp(padDepths["ring_interdigital"] ?? padDepths["apollo"] ?? 1.6, min: -2.5, max: 4.0)
            case 15, 16:  // ring phalanges
                return lerp(clamp(padDepths["apollo"] ?? 1.6, min: -2.5, max: 4.0), 0.3, idx == 15 ? 0.4 : 0.7)
            case 17, 18:  // pinky MCP → Mercury/hypothenar
                return clamp(padDepths["ulnar_interdigital"] ?? padDepths["mercury"] ?? 1.2, min: -2.5, max: 4.0)
            case 19, 20:  // pinky phalanges
                return lerp(clamp(padDepths["mercury"] ?? 1.2, min: -2.5, max: 4.0), 0.2, idx == 19 ? 0.4 : 0.7)
            default:
                return 0.3
            }
        }

        for idx in 0..<21 {
            let (x, y) = transform(landmarks[idx])
            let zMm = elevationMm(idx)
            let z = (zMm * 0.001) * elevationScale
            vertices.append(.init(x: x, y: y, z: z))
            landmark3D.append(SCNVector3(x, y, z))
            let t = clamp((zMm + 2.0) / 6.0, min: 0.0, max: 1.0)
            let (r, g, b) = elevationColor(t: t)
            colors.append(contentsOf: [r, g, b, 1.0])
        }

        // MARK: 3. Palm Mount Vertices (anatomically weighted)

        let v0 = vertices[0]; let v1 = vertices[1]; let v2 = vertices[2]
        let v5 = vertices[5]; let v9 = vertices[9]; let v17 = vertices[17]

        // Palmar fossa (concave center)
        let palmCenterIdx = Int32(vertices.count)
        let fossaZMm = clamp(padDepths["palmar_fossa"] ?? padDepths["mars"] ?? -1.2, min: -2.5, max: 0.0)
        let fossaZ = (fossaZMm * 0.001) * elevationScale
        vertices.append(.init(
            x: v0.x * 0.30 + v5.x * 0.25 + v9.x * 0.25 + v17.x * 0.20,
            y: v0.y * 0.30 + v5.y * 0.25 + v9.y * 0.25 + v17.y * 0.20,
            z: fossaZ
        ))
        let (pcR, pcG, pcB) = elevationColor(t: clamp((fossaZMm + 2.0) / 6.0, min: 0.0, max: 1.0))
        colors.append(contentsOf: [pcR, pcG, pcB, 1.0])

        // Thenar eminence (Venus mount)
        let thenarIdx = Int32(vertices.count)
        let thenarZMm = clamp((padDepths["thenar"] ?? padDepths["venus"] ?? 2.8) + 0.5, min: 0.5, max: 4.5)
        let thenarZ = (thenarZMm * 0.001) * elevationScale
        vertices.append(.init(
            x: v0.x * 0.28 + v1.x * 0.42 + v2.x * 0.30,
            y: v0.y * 0.28 + v1.y * 0.42 + v2.y * 0.30,
            z: thenarZ
        ))
        let (thR, thG, thB) = elevationColor(t: clamp((thenarZMm + 2.0) / 6.0, min: 0.0, max: 1.0))
        colors.append(contentsOf: [thR, thG, thB, 1.0])

        // Hypothenar eminence (Luna mount)
        let hypothenarIdx = Int32(vertices.count)
        let lunaZMm = clamp(padDepths["hypothenar"] ?? padDepths["luna"] ?? 2.0, min: 0.2, max: 3.8)
        let lunaZ = (lunaZMm * 0.001) * elevationScale
        vertices.append(.init(
            x: v0.x * 0.42 + v17.x * 0.58,
            y: v0.y * 0.42 + v17.y * 0.58,
            z: lunaZ
        ))
        let (hyR, hyG, hyB) = elevationColor(t: clamp((lunaZMm + 2.0) / 6.0, min: 0.0, max: 1.0))
        colors.append(contentsOf: [hyR, hyG, hyB, 1.0])

        // Additional palm arch vertex (transverse metacarpal arch midpoint)
        let archIdx = Int32(vertices.count)
        let archZMm = clamp((padDepths["saturn"] ?? 1.5) * 0.6, min: -1.0, max: 2.5)
        let archZ = (archZMm * 0.001) * elevationScale
        vertices.append(.init(
            x: (v5.x + v9.x + v17.x) / 3.0,
            y: (v5.y + v9.y + v17.y) / 3.0,
            z: archZ
        ))
        let (arR, arG, arB) = elevationColor(t: clamp((archZMm + 2.0) / 6.0, min: 0.0, max: 1.0))
        colors.append(contentsOf: [arR, arG, arB, 1.0])

        var indices: [Int32] = []

        // MARK: 4. Winding Order (left vs right hand)

        let crossZ = (v9.x - v0.x) * (v5.y - v0.y) - (v9.y - v0.y) * (v5.x - v0.x)
        let isRightHand = crossZ < 0

        func addTri(_ a: Int32, _ b: Int32, _ c: Int32) {
            if isRightHand {
                indices.append(contentsOf: [a, b, c])
            } else {
                indices.append(contentsOf: [a, c, b])
            }
        }

        // MARK: 5. Watertight Palm Plate Fan

        // Main palm fan: center → perimeter ring
        // Perimeter: 0 → thenar → 1 → 2 → 5 → arch → 9 → 13 → 17 → hypothenar → 0
        let palmRing: [Int32] = [0, thenarIdx, 1, 2, 5, archIdx, 9, 13, 17, hypothenarIdx, 0]
        for i in 0..<(palmRing.count - 1) {
            addTri(palmCenterIdx, palmRing[i], palmRing[i + 1])
        }
        // Fill the gap between thumb MCP (2) and index MCP (5) across palm surface
        addTri(2, 5, palmCenterIdx)
        // Fill MCP-to-arch bridges for natural knuckle pad surface
        addTri(5, archIdx, 9)
        addTri(9, archIdx, 13)

        // MARK: 6. Five Volumetric Digits: 8-Sided Elliptical Cross-Section

        let digitChains: [[Int]] = [
            [1, 2, 3, 4],       // Thumb
            [5, 6, 7, 8],       // Index
            [9, 10, 11, 12],    // Middle
            [13, 14, 15, 16],   // Ring
            [17, 18, 19, 20]    // Pinky
        ]
        // Physical base half-widths (palmar–dorsal oval: width ≈ 1.3 × depth)
        let baseHalfW: [Float] = [0.0050, 0.0044, 0.0046, 0.0042, 0.0036]
        let baseHalfD: [Float] = [0.0038, 0.0034, 0.0036, 0.0032, 0.0028]

        for (dIdx, chain) in digitChains.enumerated() {
            let bHW = baseHalfW[dIdx]
            let bHD = baseHalfD[dIdx]
            let n = kRingSides

            var jointRings: [[Int32]] = []  // n indices per joint

            for (segIdx, jointIdx) in chain.enumerated() {
                let jPt = vertices[jointIdx]
                // Taper ratio: 1.0 at base → 0.52 at fingertip
                let taper = 1.0 - Float(segIdx) * (0.48 / Float(chain.count - 1))
                let hw = bHW * taper   // half-width in local XY plane perp to finger
                let hd = bHD * taper   // half-depth in Z direction (dorsal-palmar)

                // Compute finger axis direction in XY plane
                let nextIdx = segIdx < chain.count - 1 ? chain[segIdx + 1] : jointIdx
                let prevIdx = segIdx > 0 ? chain[segIdx - 1] : jointIdx
                let ptA = vertices[prevIdx]; let ptB = vertices[nextIdx]
                var axDx = ptB.x - ptA.x; var axDy = ptB.y - ptA.y
                let axLen = sqrt(axDx * axDx + axDy * axDy)
                if axLen > 0.0001 { axDx /= axLen; axDy /= axLen } else { axDx = 0; axDy = 1 }

                // Perpendicular in XY plane (lateral direction)
                let perpX = -axDy; let perpY = axDx

                // Build 8-sided elliptical ring around joint center
                // Ring vertices ordered CCW looking from fingertip toward wrist:
                // angle 0 = lateral, π/2 = dorsal (+ Z), π = medial, 3π/2 = palmar (- Z)
                var ringIndices: [Int32] = []
                for k in 0..<n {
                    let angle = Float(k) * (2.0 * Float.pi) / Float(n)
                    let cosK = cos(angle); let sinK = sin(angle)
                    // Ellipse: lateral–medial axis = hw, dorsal–palmar axis = hd
                    let offsetX = perpX * hw * cosK
                    let offsetY = perpY * hw * cosK
                    let offsetZ = hd * sinK

                    // Elevation color blends from mount-level at base to light tip color
                    let skinT = Float(segIdx) / Float(chain.count - 1)
                    let (sr, sg, sb) = fingerSkinColor(t: skinT)
                    // Dorsal side slightly lighter (palmar offset is negative Z)
                    let brightness: Float = sinK > 0 ? 1.05 : 0.88
                    let vIdx = Int32(vertices.count)
                    vertices.append(.init(
                        x: jPt.x + offsetX,
                        y: jPt.y + offsetY,
                        z: jPt.z + offsetZ
                    ))
                    colors.append(contentsOf: [
                        clamp(sr * brightness, min: 0, max: 1),
                        clamp(sg * brightness, min: 0, max: 1),
                        clamp(sb * brightness, min: 0, max: 1),
                        1.0
                    ])
                    ringIndices.append(vIdx)
                }
                jointRings.append(ringIndices)
            }

            // Triangulate tube segments between consecutive joint rings
            for s in 0..<(chain.count - 1) {
                let rA = jointRings[s]; let rB = jointRings[s + 1]
                for k in 0..<n {
                    let k1 = (k + 1) % n
                    addTri(rA[k], rA[k1], rB[k])
                    addTri(rA[k1], rB[k1], rB[k])
                }
            }

            // Rounded Apex Cap: fan from axis-projected tip point
            let tipJointIdx = chain.last!
            let prevJointIdx = chain[chain.count - 2]
            let tipPt = vertices[tipJointIdx]
            let prevPt = vertices[prevJointIdx]
            var tipAxDx = tipPt.x - prevPt.x; var tipAxDy = tipPt.y - prevPt.y
            let tipAxLen = sqrt(tipAxDx * tipAxDx + tipAxDy * tipAxDy)
            if tipAxLen > 0.0001 { tipAxDx /= tipAxLen; tipAxDy /= tipAxLen }

            let apexHW = baseHalfW[dIdx] * 0.52 * 0.5
            let apexIdx = Int32(vertices.count)
            vertices.append(.init(
                x: tipPt.x + tipAxDx * apexHW,
                y: tipPt.y + tipAxDy * apexHW,
                z: tipPt.z
            ))
            colors.append(contentsOf: [0.38, 0.92, 0.74, 1.0])

            let lastRing = jointRings.last!
            for k in 0..<n {
                let k1 = (k + 1) % n
                addTri(lastRing[k], lastRing[k1], apexIdx)
            }

            // Base cap: close the tube at the MCP root with a fan to the MCP landmark
            let mcpIdx = Int32(chain[0])
            let firstRing = jointRings[0]
            for k in 0..<n {
                let k1 = (k + 1) % n
                addTri(firstRing[k1], firstRing[k], mcpIdx)
            }
        }

        return HandMeshGeometryResult(
            vertices: vertices,
            colors: colors,
            indices: indices,
            landmark3DPositions: landmark3D
        )
    }

    // MARK: - Area-Weighted Normal Computation

    /// Computes smooth, area-weighted vertex normals for soft Phong shading.
    public static func computeNormals(
        vertices: [HandMeshGeometryResult.Vertex3D],
        indices: [Int32]
    ) -> [SCNVector3] {
        var normals = [simd_float3](repeating: simd_float3(0, 0, 1), count: vertices.count)

        var i = 0
        while i < indices.count - 2 {
            let i0 = Int(indices[i]); let i1 = Int(indices[i + 1]); let i2 = Int(indices[i + 2])
            if i0 < vertices.count && i1 < vertices.count && i2 < vertices.count {
                let p0 = simd_float3(vertices[i0].x, vertices[i0].y, vertices[i0].z)
                let p1 = simd_float3(vertices[i1].x, vertices[i1].y, vertices[i1].z)
                let p2 = simd_float3(vertices[i2].x, vertices[i2].y, vertices[i2].z)
                // Area-weighted normal (magnitude of cross product = 2× triangle area)
                let n = simd_cross(p1 - p0, p2 - p0)
                normals[i0] += n; normals[i1] += n; normals[i2] += n
            }
            i += 3
        }

        return normals.map {
            let l = simd_length($0)
            let unit = l > 0.0001 ? $0 / l : simd_float3(0, 0, 1)
            return SCNVector3(unit.x, unit.y, unit.z)
        }
    }
}
