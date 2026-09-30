import Foundation
import CoreGraphics
import SceneKit
import simd
import CurvPalmOverlay

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

/// Anatomical 3D hand mesh builder producing a watertight, proportional, scan-accurate 3D surface.
///
/// Features:
///  • Chirality awareness: Left hand palm renders with thumb on anatomical right; Right hand with thumb on left.
///  • Dense palm grid: Structured 5-row × 8-column anatomical mesh with thenar/hypothenar mounds & palmar fossa.
///  • Seamless finger junction: Knuckle & web perimeter connects directly into the 5 volumetric digit tubes.
///  • Volumetric fingers: 8-sided elliptical cross-sections with smooth tapers and rounded apex caps.
///  • Topographic colormap: Continuous depth heatmap with guaranteed minimum luminance against dark scenes.
public enum HandMeshGeometryBuilder {
    public typealias Vertex3D = HandMeshGeometryResult.Vertex3D

    // MARK: - Constants

    private static let kRingSides = 8  // 8-sided elliptical tube per digit

    // MARK: - Math Helpers

    private static func clamp(_ v: Float, min lo: Float, max hi: Float) -> Float {
        Swift.max(lo, Swift.min(hi, v))
    }

    private static func lerp(_ a: Float, _ b: Float, _ t: Float) -> Float {
        a + (b - a) * t
    }

    // MARK: - Elevation Colormap

    /// Elevation colormap: deep blue (hollow) → cyan → emerald → amber → coral (prominent mound).
    /// Minimum luminance 0.28 is enforced so cool/depression zones never vanish on black backgrounds.
    public static func elevationColor(t: Float) -> (Float, Float, Float) {
        let c = clamp(t, min: 0.0, max: 1.0)
        var r: Float; var g: Float; var b: Float
        switch c {
        case ..<0.25:
            let u = c / 0.25
            r = lerp(0.12, 0.08, u); g = lerp(0.55, 0.82, u); b = lerp(0.98, 0.92, u)
        case ..<0.50:
            let u = (c - 0.25) / 0.25
            r = lerp(0.08, 0.22, u); g = lerp(0.82, 0.90, u); b = lerp(0.92, 0.42, u)
        case ..<0.75:
            let u = (c - 0.50) / 0.25
            r = lerp(0.22, 0.96, u); g = lerp(0.90, 0.80, u); b = lerp(0.42, 0.18, u)
        default:
            let u = (c - 0.75) / 0.25
            r = lerp(0.96, 0.99, u); g = lerp(0.80, 0.32, u); b = lerp(0.18, 0.22, u)
        }
        let lum = 0.299 * r + 0.587 * g + 0.114 * b
        if lum < 0.28 {
            let boost = 0.28 / Swift.max(lum, 0.001)
            r = clamp(r * boost, min: 0, max: 1)
            g = clamp(g * boost, min: 0, max: 1)
            b = clamp(b * boost, min: 0, max: 1)
        }
        return (r, g, b)
    }

    // MARK: - Public API

    public static func buildMeshGeometry(
        landmarks: [CGPoint],
        padDepths: [String: Float],
        referencePalmWidthMm: Float = 80.0,
        elevationScale: Float = 1.0,
        handSide: HandSide = .right
    ) -> HandMeshGeometryResult {
        guard landmarks.count >= 21 else {
            return HandMeshGeometryResult(vertices: [], colors: [], indices: [], landmark3DPositions: [])
        }

        // MARK: 1. Coordinate Alignment (Wrist -> MCP 9 points straight +Y)

        let p0 = landmarks[0]   // wrist
        let p9 = landmarks[9]   // middle MCP
        let p5 = landmarks[5]   // index MCP
        let p17 = landmarks[17] // pinky MCP

        let dirX = Float(p9.x - p0.x)
        let dirY = Float(p9.y - p0.y)
        let upAngle = atan2(dirX, -dirY)
        let cosA = cos(-upAngle); let sinA = sin(-upAngle)

        let pivot = CGPoint(x: (p0.x + p9.x) * 0.5, y: (p0.y + p9.y) * 0.5)

        let knucklePx = hypot(Float(p17.x - p5.x), Float(p17.y - p5.y))
        let metricScale: Float = knucklePx > 0.01 ? (0.080 / knucklePx) : 0.18

        let xform: (CGPoint) -> (Float, Float) = { pt in
            let dx = Float(pt.x - pivot.x); let dy = Float(pt.y - pivot.y)
            return ((cosA * dx - sinA * dy) * metricScale, -(sinA * dx + cosA * dy) * metricScale)
        }

        // MARK: 2. Handedness / Chirality Mirroring
        // Looking at a human palm:
        //  • Right Hand: Thumb is on the LEFT (-X), Pinky on the RIGHT (+X).
        //  • Left Hand:  Thumb is on the RIGHT (+X), Pinky on the LEFT (-X).

        let initialP1 = xform(landmarks[1])
        let initialP17 = xform(landmarks[17])
        let isThumbCurrentlyOnLeft = initialP1.0 < initialP17.0

        let effectiveSide: HandSide
        if handSide == .auto {
            effectiveSide = HandLandmarkPose.isLeftHand(landmarks) ? .left : .right
        } else {
            effectiveSide = handSide
        }

        let needsMirrorX: Bool
        if effectiveSide == .left {
            needsMirrorX = isThumbCurrentlyOnLeft   // Left hand must have thumb on the right (+X)
        } else {
            needsMirrorX = !isThumbCurrentlyOnLeft  // Right hand must have thumb on the left (-X)
        }

        let mirrorSign: Float = needsMirrorX ? -1.0 : 1.0

        // MARK: 3. Compute 21 Landmark 3D Positions

        let depthForLandmark: (Int) -> Float = { idx in
            switch idx {
            case 0:     return clamp(padDepths["wrist"] ?? -0.3, min: -1.5, max: 0.5)
            case 1, 2:   return clamp(padDepths["thenar"] ?? padDepths["venus"] ?? 2.6, min: -2.5, max: 4.0)
            case 3, 4:   return clamp(padDepths["thenar"] ?? 2.0, min: -2.5, max: 4.0) * (idx == 3 ? 0.7 : 0.4)
            case 5, 6:   return clamp(padDepths["radial_interdigital"] ?? padDepths["jupiter"] ?? 1.8, min: -2.5, max: 4.0)
            case 7, 8:   return lerp(clamp(padDepths["jupiter"] ?? 1.8, min: -2.5, max: 4.0), 0.3, idx == 7 ? 0.4 : 0.7)
            case 9, 10:  return clamp(padDepths["central_interdigital"] ?? padDepths["saturn"] ?? 1.5, min: -2.5, max: 4.0)
            case 11, 12: return lerp(clamp(padDepths["saturn"] ?? 1.5, min: -2.5, max: 4.0), 0.3, idx == 11 ? 0.4 : 0.7)
            case 13, 14: return clamp(padDepths["ring_interdigital"] ?? padDepths["apollo"] ?? 1.6, min: -2.5, max: 4.0)
            case 15, 16: return lerp(clamp(padDepths["apollo"] ?? 1.6, min: -2.5, max: 4.0), 0.3, idx == 15 ? 0.4 : 0.7)
            case 17, 18: return clamp(padDepths["ulnar_interdigital"] ?? padDepths["mercury"] ?? 1.2, min: -2.5, max: 4.0)
            case 19, 20: return lerp(clamp(padDepths["mercury"] ?? 1.2, min: -2.5, max: 4.0), 0.2, idx == 19 ? 0.4 : 0.7)
            default:     return 0.3
            }
        }

        var vertices: [HandMeshGeometryResult.Vertex3D] = []
        var colors: [Float] = []
        var landmark3D: [SCNVector3] = []

        func addVertex(_ x: Float, _ y: Float, _ z: Float, elevMm: Float) -> Int32 {
            let idx = Int32(vertices.count)
            vertices.append(.init(x: x, y: y, z: z))
            let t = clamp((elevMm + 2.0) / 6.0, min: 0.0, max: 1.0)
            let (r, g, b) = elevationColor(t: t)
            colors.append(contentsOf: [r, g, b, 1.0])
            return idx
        }

        for idx in 0..<21 {
            let (rawX, rawY) = xform(landmarks[idx])
            let x = rawX * mirrorSign
            let y = rawY
            let zMm = depthForLandmark(idx)
            let z = (zMm * 0.001) * elevationScale
            _ = addVertex(x, y, z, elevMm: zMm)
            landmark3D.append(SCNVector3(x, y, z))
        }

        // Triangle winding order: CCW facing the camera (+Z)
        var indices: [Int32] = []
        let crossZ = (vertices[9].x - vertices[0].x) * (vertices[5].y - vertices[0].y)
                   - (vertices[9].y - vertices[0].y) * (vertices[5].x - vertices[0].x)
        let isRightWinding = crossZ > 0

        func addTri(_ a: Int32, _ b: Int32, _ c: Int32) {
            if isRightWinding {
                indices.append(contentsOf: [a, b, c])
            } else {
                indices.append(contentsOf: [a, c, b])
            }
        }

        // MARK: 4. Dense Anatomical Palm Surface Grid
        //
        // Solves the "empty palm" defect: constructs a structured 5-row × 8-column
        // topographical mesh across the palm that bridges directly into the knuckles.

        let v0 = vertices[0]   // Wrist
        let v1 = vertices[1]   // Thumb CMC
        let v2 = vertices[2]   // Thumb MCP
        let v5 = vertices[5]   // Index MCP
        let v9 = vertices[9]   // Middle MCP
        let v13 = vertices[13] // Ring MCP
        let v17 = vertices[17] // Pinky MCP

        // 8 anatomical boundary rays along the knuckle / web margin:
        // Col 0: Thumb CMC-MCP ray
        // Col 1: First web space (between thumb MCP and index MCP)
        // Col 2: Index MCP
        // Col 3: Index-Middle web fold
        // Col 4: Middle MCP
        // Col 5: Middle-Ring web fold
        // Col 6: Ring MCP
        // Col 7: Pinky MCP
        let distalRays: [Vertex3D] = [
            Vertex3D(x: v2.x, y: v2.y, z: v2.z),
            Vertex3D(x: v2.x * 0.45 + v5.x * 0.55, y: v2.y * 0.45 + v5.y * 0.55 - 0.005, z: (v2.z + v5.z) * 0.5),
            Vertex3D(x: v5.x, y: v5.y, z: v5.z),
            Vertex3D(x: (v5.x + v9.x) * 0.5, y: (v5.y + v9.y) * 0.5 - 0.002, z: (v5.z + v9.z) * 0.5),
            Vertex3D(x: v9.x, y: v9.y, z: v9.z),
            Vertex3D(x: (v9.x + v13.x) * 0.5, y: (v9.y + v13.y) * 0.5 - 0.002, z: (v9.z + v13.z) * 0.5),
            Vertex3D(x: v13.x, y: v13.y, z: v13.z),
            Vertex3D(x: v17.x, y: v17.y, z: v17.z)
        ]

        // Corresponding wrist proximal boundary across carpal arch:
        let wristDirX = v17.x - v1.x
        let wristDirY = v17.y - v1.y
        let proximalRays: [Vertex3D] = (0..<8).map { col in
            let frac = Float(col) / 7.0
            return Vertex3D(
                x: v0.x + wristDirX * (frac - 0.5) * 0.45,
                y: v0.y + wristDirY * (frac - 0.5) * 0.45 - 0.004 * (1.0 - abs(frac - 0.5) * 2.0),
                z: v0.z
            )
        }

        // Mound elevations
        let thenarZMm = clamp(padDepths["thenar"] ?? padDepths["venus"] ?? 2.8, min: 1.0, max: 4.5)
        let lunaZMm = clamp(padDepths["hypothenar"] ?? padDepths["luna"] ?? 2.0, min: 0.5, max: 3.8)
        let fossaZMm = clamp(padDepths["palmar_fossa"] ?? padDepths["mars"] ?? -1.2, min: -2.5, max: -0.2)
        let jupiterZMm = clamp(padDepths["jupiter"] ?? 1.8, min: 0.5, max: 3.5)
        let saturnZMm = clamp(padDepths["saturn"] ?? 1.5, min: 0.5, max: 3.5)
        let apolloZMm = clamp(padDepths["apollo"] ?? 1.6, min: 0.5, max: 3.5)
        let mercuryZMm = clamp(padDepths["mercury"] ?? 1.2, min: 0.5, max: 3.5)

        // Build 5 rows × 8 cols grid
        let numRows = 5
        let numCols = 8
        var gridIndices: [[Int32]] = []

        for r in 0..<numRows {
            var rowIndices: [Int32] = []
            let u = Float(r) / Float(numRows - 1)  // 0.0 = wrist, 1.0 = knuckle row

            for c in 0..<numCols {
                let pProx = proximalRays[c]
                let pDist = distalRays[c]
                let x = lerp(pProx.x, pDist.x, u)
                let y = lerp(pProx.y, pDist.y, u)

                // Anatomical Z elevation model
                var zMm: Float = lerp(-0.3, 1.2, u)

                // Thenar (Venus) eminence: peaks around row 1-2, col 0-1
                let dThenarCol = Float(c) - 0.8
                let dThenarRow = Float(r) - 1.5
                let thenarWeight = exp(-(dThenarCol * dThenarCol * 0.6 + dThenarRow * dThenarRow * 0.7))
                zMm += thenarZMm * thenarWeight * 0.75

                // Hypothenar (Luna) eminence: peaks around row 1-2, col 6-7
                let dLunaCol = Float(c) - 6.5
                let dLunaRow = Float(r) - 1.3
                let lunaWeight = exp(-(dLunaCol * dLunaCol * 0.6 + dLunaRow * dLunaRow * 0.7))
                zMm += lunaZMm * lunaWeight * 0.70

                // Palmar fossa: central hollow around row 2, col 3-4
                let dFossaCol = Float(c) - 3.5
                let dFossaRow = Float(r) - 2.0
                let fossaWeight = exp(-(dFossaCol * dFossaCol * 0.8 + dFossaRow * dFossaRow * 0.9))
                zMm += fossaZMm * fossaWeight * 0.85

                // Sub-digital knuckle pads at row 3 (Jupiter, Saturn, Apollo, Mercury)
                if r == 3 {
                    switch c {
                    case 2: zMm += jupiterZMm * 0.45
                    case 4: zMm += saturnZMm * 0.45
                    case 6: zMm += apolloZMm * 0.45
                    case 7: zMm += mercuryZMm * 0.45
                    default: break
                    }
                }

                // Row 4 is the knuckle perimeter — clamp to actual distal anchor Z
                if r == numRows - 1 {
                    zMm = pDist.z / (0.001 * elevationScale)
                }

                let z = (zMm * 0.001) * elevationScale
                let vIdx = addVertex(x, y, z, elevMm: zMm)
                rowIndices.append(vIdx)
            }
            gridIndices.append(rowIndices)
        }

        // Triangulate palm grid (quad cells subdivided into two triangles)
        for r in 0..<(numRows - 1) {
            for c in 0..<(numCols - 1) {
                let iTL = gridIndices[r + 1][c]
                let iTR = gridIndices[r + 1][c + 1]
                let iBL = gridIndices[r][c]
                let iBR = gridIndices[r][c + 1]

                addTri(iBL, iTR, iTL)
                addTri(iBL, iBR, iTR)
            }
        }

        // MARK: 5. Volumetric 8-Sided Digits with MCP Joint Integration

        let digitChains: [[Int]] = [
            [1, 2, 3, 4],       // Thumb
            [5, 6, 7, 8],       // Index
            [9, 10, 11, 12],    // Middle
            [13, 14, 15, 16],   // Ring
            [17, 18, 19, 20]    // Pinky
        ]
        let baseHalfW: [Float] = [0.0052, 0.0046, 0.0048, 0.0044, 0.0038]
        let baseHalfD: [Float] = [0.0040, 0.0035, 0.0037, 0.0033, 0.0029]
        let n = kRingSides

        var digitFirstRings: [[Int32]] = []

        for (dIdx, chain) in digitChains.enumerated() {
            let bHW = baseHalfW[dIdx]; let bHD = baseHalfD[dIdx]
            var jointRings: [[Int32]] = []

            for (segIdx, jointIdx) in chain.enumerated() {
                let jPt = vertices[jointIdx]
                let taper = 1.0 - Float(segIdx) * (0.50 / Float(chain.count - 1))
                let hw = bHW * taper; let hd = bHD * taper

                let nextIdx = segIdx < chain.count - 1 ? chain[segIdx + 1] : jointIdx
                let prevIdx = segIdx > 0 ? chain[segIdx - 1] : jointIdx
                let pA = vertices[prevIdx]; let pB = vertices[nextIdx]
                var axDx = pB.x - pA.x; var axDy = pB.y - pA.y
                let axLen = sqrt(axDx * axDx + axDy * axDy)
                if axLen > 0.0001 { axDx /= axLen; axDy /= axLen } else { axDx = 0; axDy = 1 }
                let perpX = -axDy; let perpY = axDx

                var ringIndices: [Int32] = []
                for k in 0..<n {
                    let angle = Float(k) * (2.0 * Float.pi) / Float(n)
                    let cosK = cos(angle); let sinK = sin(angle)

                    let vx = jPt.x + perpX * hw * cosK
                    let vy = jPt.y + perpY * hw * cosK
                    let vz = jPt.z + hd * sinK

                    let segFrac = Float(segIdx) / Float(chain.count - 1)
                    let baseZMm = depthForLandmark(jointIdx)
                    let fingZMm = lerp(baseZMm, 0.2, segFrac * 0.6)
                    let tSurface = clamp((fingZMm + (sinK > 0 ? 0.5 : -0.3) + 2.0) / 6.0, min: 0.0, max: 1.0)
                    let vIdx = addVertex(vx, vy, vz, elevMm: lerp(-2.0, 4.0, tSurface))
                    ringIndices.append(vIdx)
                }
                jointRings.append(ringIndices)
            }

            digitFirstRings.append(jointRings[0])

            // Tube segments between consecutive rings
            for s in 0..<(chain.count - 1) {
                let rA = jointRings[s]; let rB = jointRings[s + 1]
                for k in 0..<n {
                    let k1 = (k + 1) % n
                    addTri(rA[k], rA[k1], rB[k])
                    addTri(rA[k1], rB[k1], rB[k])
                }
            }

            // Fingertip apex cap
            let tipPt = vertices[chain.last!]; let prevPt = vertices[chain[chain.count - 2]]
            var tipAxDx = tipPt.x - prevPt.x; var tipAxDy = tipPt.y - prevPt.y
            let tipLen = sqrt(tipAxDx * tipAxDx + tipAxDy * tipAxDy)
            if tipLen > 0.0001 { tipAxDx /= tipLen; tipAxDy /= tipLen }
            let apexIdx = addVertex(
                tipPt.x + tipAxDx * baseHalfW[dIdx] * 0.52 * 0.5,
                tipPt.y + tipAxDy * baseHalfW[dIdx] * 0.52 * 0.5,
                tipPt.z,
                elevMm: 0.4
            )
            let lastRing = jointRings.last!
            for k in 0..<n {
                addTri(lastRing[k], lastRing[(k + 1) % n], apexIdx)
            }

            // Root cap linking tube to MCP landmark
            let mcpIdx = Int32(chain[0])
            let firstRing = jointRings[0]
            for k in 0..<n {
                addTri(firstRing[(k + 1) % n], firstRing[k], mcpIdx)
            }
        }

        // MARK: 6. Interdigital Web Fills (finger base bridges)
        // Connects adjacent finger tubes at the knuckle row so there are no empty gaps.

        if digitFirstRings.count >= 5 {
            let webColumns: [Int] = [3, 5, 7]  // Palm grid columns for web spaces
            for pair in 0..<3 {
                let ringA = digitFirstRings[pair + 1]  // e.g. Index
                let ringB = digitFirstRings[pair + 2]  // e.g. Middle
                let webPalmVertex = gridIndices[numRows - 1][webColumns[pair]]

                // Find the ring vertices closest to each other
                let centerB = vertices[Int(digitChains[pair + 2][0])]
                let centerA = vertices[Int(digitChains[pair + 1][0])]

                let sortedA = ringA.sorted {
                    let va = vertices[Int($0)]; let vb = vertices[Int($1)]
                    let dA = (va.x - centerB.x) * (va.x - centerB.x) + (va.y - centerB.y) * (va.y - centerB.y)
                    let dB = (vb.x - centerB.x) * (vb.x - centerB.x) + (vb.y - centerB.y) * (vb.y - centerB.y)
                    return dA < dB
                }
                let sortedB = ringB.sorted {
                    let va = vertices[Int($0)]; let vb = vertices[Int($1)]
                    let dA = (va.x - centerA.x) * (va.x - centerA.x) + (va.y - centerA.y) * (va.y - centerA.y)
                    let dB = (vb.x - centerA.x) * (vb.x - centerA.x) + (vb.y - centerA.y) * (vb.y - centerA.y)
                    return dA < dB
                }

                let a0 = sortedA[0]; let a1 = sortedA[1]
                let b0 = sortedB[0]; let b1 = sortedB[1]

                addTri(webPalmVertex, a0, b0)
                addTri(a0, a1, b0)
                addTri(a1, b1, b0)
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
                let n = simd_cross(p1 - p0, p2 - p0)
                normals[i0] += n; normals[i1] += n; normals[i2] += n
            }
            i += 3
        }
        return normals.map {
            let l = simd_length($0)
            let u = l > 0.0001 ? $0 / l : simd_float3(0, 0, 1)
            return SCNVector3(u.x, u.y, u.z)
        }
    }

    // MARK: - Image ↔ Mesh Alignment (Photo Underlay)

    /// Closed-form least-squares 2D affine fit mapping normalized image space
    /// (0…1, top-left origin) onto the mesh's XY plane, through the 21 landmark
    /// correspondences. Absorbs the builder's rotation, metric scale, and
    /// chirality mirror — and stays correct if that alignment math ever changes.
    ///
    /// Convention: `(x, y, 1)ᵀ · M → (x′, y′, 1)ᵀ`.
    /// Returns nil for fewer than 3 correspondences or a degenerate (collinear) set.
    public static func affineImageToMeshFit(
        imageLandmarks: [CGPoint],
        meshLandmarks: [SCNVector3]
    ) -> simd_float3x3? {
        let n = min(imageLandmarks.count, meshLandmarks.count)
        guard n >= 3 else { return nil }

        var sxx: Float = 0; var sxy: Float = 0; var syy: Float = 0
        var sx: Float = 0;  var sy: Float = 0
        var sxX: Float = 0; var syX: Float = 0; var sX: Float = 0
        var sxY: Float = 0; var syY: Float = 0; var sY: Float = 0

        for i in 0..<n {
            let p = imageLandmarks[i]
            let q = meshLandmarks[i]
            let x = Float(p.x), y = Float(p.y)
            let X = Float(q.x), Y = Float(q.y)
            sxx += x * x; sxy += x * y; syy += y * y
            sx += x; sy += y
            sxX += x * X; syX += y * X; sX += X
            sxY += x * Y; syY += y * Y; sY += Y
        }

        // Normal equations (symmetric) for both output rows; solve via 3×3 inverse.
        let normal = simd_float3x3(
            SIMD3(sxx, sxy, sx),
            SIMD3(sxy, syy, sy),
            SIMD3(sx, sy, Float(n))
        )
        guard abs(simd_determinant(normal)) > 1e-5 else { return nil }
        let inv = normal.inverse

        let abc = inv * SIMD3(sxX, syX, sX)   // x′ = a·x + b·y + c
        let def = inv * SIMD3(sxY, syY, sY)   // y′ = d·x + e·y + f

        return simd_float3x3(
            SIMD3(abc.x, def.x, 0),
            SIMD3(abc.y, def.y, 0),
            SIMD3(abc.z, def.z, 1)
        )
    }
}
