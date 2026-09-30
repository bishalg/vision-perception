import XCTest
import CoreGraphics
import SceneKit
@testable import PalmScanner

final class HandMeshGeometryBuilderTests: XCTestCase {

    /// Standard synthetic upright hand landmarks (21 points)
    private var sampleLandmarks: [CGPoint] {
        var pts = [CGPoint](repeating: CGPoint(x: 0.5, y: 0.5), count: 21)
        // Wrist
        pts[0] = CGPoint(x: 0.50, y: 0.85)
        // Thumb (1, 2, 3, 4)
        pts[1] = CGPoint(x: 0.40, y: 0.72)
        pts[2] = CGPoint(x: 0.32, y: 0.62)
        pts[3] = CGPoint(x: 0.28, y: 0.54)
        pts[4] = CGPoint(x: 0.24, y: 0.48)
        // Index (5, 6, 7, 8)
        pts[5] = CGPoint(x: 0.38, y: 0.50)
        pts[6] = CGPoint(x: 0.36, y: 0.38)
        pts[7] = CGPoint(x: 0.35, y: 0.28)
        pts[8] = CGPoint(x: 0.34, y: 0.20)
        // Middle (9, 10, 11, 12)
        pts[9] = CGPoint(x: 0.50, y: 0.48)
        pts[10] = CGPoint(x: 0.50, y: 0.34)
        pts[11] = CGPoint(x: 0.50, y: 0.24)
        pts[12] = CGPoint(x: 0.50, y: 0.15)
        // Ring (13, 14, 15, 16)
        pts[13] = CGPoint(x: 0.62, y: 0.51)
        pts[14] = CGPoint(x: 0.63, y: 0.37)
        pts[15] = CGPoint(x: 0.64, y: 0.27)
        pts[16] = CGPoint(x: 0.65, y: 0.19)
        // Pinky (17, 18, 19, 20)
        pts[17] = CGPoint(x: 0.72, y: 0.55)
        pts[18] = CGPoint(x: 0.74, y: 0.44)
        pts[19] = CGPoint(x: 0.75, y: 0.36)
        pts[20] = CGPoint(x: 0.76, y: 0.29)
        return pts
    }

    func testInsufficientLandmarksReturnsEmptyMesh() {
        let result = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: [CGPoint(x: 0.5, y: 0.5)],
            padDepths: [:]
        )
        XCTAssertTrue(result.vertices.isEmpty)
        XCTAssertTrue(result.indices.isEmpty)
        XCTAssertTrue(result.colors.isEmpty)
        XCTAssertTrue(result.landmark3DPositions.isEmpty)
    }

    func testStandardHandMeshReconstruction() {
        let depths: [String: Float] = [
            "venus": 2.5,
            "jupiter": 1.8,
            "saturn": 1.5,
            "apollo": 1.6,
            "mercury": 1.2,
            "luna": 2.0,
            "palmar_fossa": -1.0
        ]

        let mesh = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: sampleLandmarks,
            padDepths: depths,
            referencePalmWidthMm: 80.0,
            elevationScale: 1.2
        )

        // 1. Mesh contains expected minimum vertices
        // (21 landmarks + palm center + 2 mounts + 5 digits * 4 joints * 4 verts + 5 apex caps)
        XCTAssertGreaterThan(mesh.vertices.count, 80)
        XCTAssertEqual(mesh.landmark3DPositions.count, 21)

        // 2. Color channel count matches 4 floats (RGBA) per vertex
        XCTAssertEqual(mesh.colors.count, mesh.vertices.count * 4)

        // 3. Triangle indices form complete triplets
        XCTAssertEqual(mesh.indices.count % 3, 0)
        XCTAssertGreaterThan(mesh.indices.count, 100)

        // 4. All triangle indices are strictly within vertex array bounds
        for idx in mesh.indices {
            XCTAssertGreaterThanOrEqual(idx, 0)
            XCTAssertLessThan(Int(idx), mesh.vertices.count)
        }

        // 5. Upright alignment verification:
        // Fingertip of middle finger (landmark 12) must have higher Y than wrist (landmark 0)
        let wristPos = mesh.landmark3DPositions[0]
        let middleTipPos = mesh.landmark3DPositions[12]
        XCTAssertGreaterThan(middleTipPos.y, wristPos.y, "Fingertip must point upward relative to wrist")

        // 6. Normal computation produces valid unit vectors for all vertices
        let normals = HandMeshGeometryBuilder.computeNormals(vertices: mesh.vertices, indices: mesh.indices)
        XCTAssertEqual(normals.count, mesh.vertices.count)
        for normal in normals {
            let len = sqrt(normal.x * normal.x + normal.y * normal.y + normal.z * normal.z)
            XCTAssertEqual(len, 1.0, accuracy: 0.01, "All vertex normals must be unit vectors")
        }
    }

    func testElevationClampingPreventsSpikeDistortion() {
        // Extreme elevation values that would previously distort the hand into a cone/spike
        let extremeDepths: [String: Float] = [
            "saturn": 61.5,
            "venus": -21.4,
            "jupiter": 85.0,
            "palmar_fossa": -50.0
        ]

        let mesh = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: sampleLandmarks,
            padDepths: extremeDepths,
            referencePalmWidthMm: 80.0,
            elevationScale: 1.0
        )

        // Maximum allowable Z elevation in meters: 4.5mm * 0.001 = 0.0045m
        let maxAllowedZ: Float = 0.008
        let minAllowedZ: Float = -0.008

        for v in mesh.vertices {
            XCTAssertLessThanOrEqual(v.z, maxAllowedZ, "Vertex Z must not spike beyond physical limits")
            XCTAssertGreaterThanOrEqual(v.z, minAllowedZ, "Vertex Z must not invert beyond physical limits")
        }
    }

    func testElevationColorMapsFullDomain() {
        let blue = HandMeshGeometryBuilder.elevationColor(t: 0.0)
        let cyan = HandMeshGeometryBuilder.elevationColor(t: 0.25)
        let green = HandMeshGeometryBuilder.elevationColor(t: 0.50)
        let amber = HandMeshGeometryBuilder.elevationColor(t: 0.75)
        let coral = HandMeshGeometryBuilder.elevationColor(t: 1.0)

        // Blue has high blue component
        XCTAssertGreaterThan(blue.2, blue.0)
        // Cyan has high green and blue
        XCTAssertGreaterThan(cyan.1, 0.5)
        XCTAssertGreaterThan(cyan.2, 0.5)
        // Green has high green component
        XCTAssertGreaterThan(green.1, green.0)
        // Amber has high red and green
        XCTAssertGreaterThan(amber.0, 0.5)
        // Coral has high red component
        XCTAssertGreaterThan(coral.0, coral.2)
    }

    func testLeftHandChiralityPutsThumbOnAnatomicalRight() {
        // Build mesh for Left Hand
        let leftMesh = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: sampleLandmarks,
            padDepths: [:],
            handSide: .left
        )
        // Landmark 4 is thumb tip, Landmark 20 is pinky tip
        let thumbTip = leftMesh.landmark3DPositions[4]
        let pinkyTip = leftMesh.landmark3DPositions[20]
        XCTAssertGreaterThan(thumbTip.x, pinkyTip.x, "In left hand palm view, thumb must be on the anatomical right (+X)")

        // Build mesh for Right Hand
        let rightMesh = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: sampleLandmarks,
            padDepths: [:],
            handSide: .right
        )
        let rightThumbTip = rightMesh.landmark3DPositions[4]
        let rightPinkyTip = rightMesh.landmark3DPositions[20]
        XCTAssertLessThan(rightThumbTip.x, rightPinkyTip.x, "In right hand palm view, thumb must be on the anatomical left (-X)")
    }

    // MARK: - affineImageToMeshFit (Photo Underlay Alignment)

    private func apply(_ m: simd_float3x3, _ p: CGPoint) -> CGPoint {
        let q = m * SIMD3(Float(p.x), Float(p.y), 1)
        return CGPoint(x: CGFloat(q.x), y: CGFloat(q.y))
    }

    private func assertAffine(
        _ m: simd_float3x3,
        columns c0: (Float, Float), c1: (Float, Float), c2: (Float, Float),
        accuracy: Float,
        _ message: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertEqual(m.columns.0.x, c0.0, accuracy: accuracy, message, file: file, line: line)
        XCTAssertEqual(m.columns.0.y, c0.1, accuracy: accuracy, message, file: file, line: line)
        XCTAssertEqual(m.columns.1.x, c1.0, accuracy: accuracy, message, file: file, line: line)
        XCTAssertEqual(m.columns.1.y, c1.1, accuracy: accuracy, message, file: file, line: line)
        XCTAssertEqual(m.columns.2.x, c2.0, accuracy: accuracy, message, file: file, line: line)
        XCTAssertEqual(m.columns.2.y, c2.1, accuracy: accuracy, message, file: file, line: line)
    }

    func testAffineFitRecoversKnownRotationScaleTranslation() throws {
        let theta: Float = 20.0 * Float.pi / 180.0
        let s: Float = 0.09
        let tx: Float = 0.02, ty: Float = -0.05

        let targets = sampleLandmarks.map { p -> CGPoint in
            let x = Float(p.x), y = Float(p.y)
            return CGPoint(
                x: CGFloat(s * (cos(theta) * x - sin(theta) * y) + tx),
                y: CGFloat(s * (sin(theta) * x + cos(theta) * y) + ty)
            )
        }
        let meshPts = targets.map { SCNVector3($0.x, $0.y, 0) }

        let fit = HandMeshGeometryBuilder.affineImageToMeshFit(
            imageLandmarks: sampleLandmarks,
            meshLandmarks: meshPts
        )
        let m = try XCTUnwrap(fit, "Fit must succeed for a full landmark set")
        assertAffine(
            m,
            columns: (s * cos(theta), s * sin(theta)),
            c1: (-s * sin(theta), s * cos(theta)),
            c2: (tx, ty),
            accuracy: 1e-4,
            "Fit must recover the known rotation/scale/translation"
        )
        // Spot-check an off-correspondence point maps consistently too.
        let probe = CGPoint(x: 0.13, y: 0.77)
        let mapped = apply(m, probe)
        let expected = CGPoint(
            x: CGFloat(s * (cos(theta) * 0.13 - sin(theta) * 0.77) + tx),
            y: CGFloat(s * (sin(theta) * 0.13 + cos(theta) * 0.77) + ty)
        )
        XCTAssertEqual(mapped.x, expected.x, accuracy: 1e-4, "Probe X mapping")
        XCTAssertEqual(mapped.y, expected.y, accuracy: 1e-4, "Probe Y mapping")
    }

    func testAffineFitRecoversReflectedTransform() throws {
        // Chirality mirror (x -> -x) composed with rotation/scale/translation.
        let theta: Float = -15.0 * Float.pi / 180.0
        let s: Float = 0.075
        let tx: Float = -0.03, ty: Float = 0.04

        let targets = sampleLandmarks.map { p -> CGPoint in
            let x = Float(p.x), y = Float(p.y)
            let mx = -x
            return CGPoint(
                x: CGFloat(s * (cos(theta) * mx - sin(theta) * y) + tx),
                y: CGFloat(s * (sin(theta) * mx + cos(theta) * y) + ty)
            )
        }
        let meshPts = targets.map { SCNVector3($0.x, $0.y, 0) }

        let fit = HandMeshGeometryBuilder.affineImageToMeshFit(
            imageLandmarks: sampleLandmarks,
            meshLandmarks: meshPts
        )
        let m = try XCTUnwrap(fit, "Fit must succeed for a full landmark set")
        // Mirror: 2x2 linear part has negative determinant.
        let det = m.columns.0.x * m.columns.1.y - m.columns.1.x * m.columns.0.y
        XCTAssertLessThan(det, 0, "Reflected transform must be recovered (negative determinant)")
        assertAffine(
            m,
            columns: (-s * cos(theta), -s * sin(theta)),
            c1: (-s * sin(theta), s * cos(theta)),
            c2: (tx, ty),
            accuracy: 1e-4,
            "Fit must recover the known reflection + rotation/scale/translation"
        )
    }

    func testAffineFitMapsBuiltMeshLandmarksEndToEnd() throws {
        let mesh = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: sampleLandmarks,
            padDepths: ["venus": 2.5, "luna": 2.0, "palmar_fossa": -1.0],
            referencePalmWidthMm: 80.0,
            elevationScale: 1.2,
            handSide: .right
        )

        let fit = HandMeshGeometryBuilder.affineImageToMeshFit(
            imageLandmarks: sampleLandmarks,
            meshLandmarks: mesh.landmark3DPositions
        )
        let m = try XCTUnwrap(fit, "Fit must succeed for a built mesh")

        // Every landmark must map onto its own mesh XY position — the photo
        // underlay alignment is only valid if this holds for all 21 points.
        for (i, p) in sampleLandmarks.enumerated() {
            let mapped = apply(m, p)
            let target = mesh.landmark3DPositions[i]
            XCTAssertEqual(mapped.x, CGFloat(target.x), accuracy: 1e-4, "Landmark \(i) X mapping")
            XCTAssertEqual(mapped.y, CGFloat(target.y), accuracy: 1e-4, "Landmark \(i) Y mapping")
        }
    }

    func testAffineFitDegenerateInputsReturnNil() {
        let meshPts = sampleLandmarks.map { SCNVector3($0.x, $0.y, 0) }

        // Too few correspondences
        XCTAssertNil(
            HandMeshGeometryBuilder.affineImageToMeshFit(
                imageLandmarks: Array(sampleLandmarks.prefix(2)),
                meshLandmarks: Array(meshPts.prefix(2))
            ),
            "Fewer than 3 correspondences must return nil"
        )

        // Collinear correspondences are degenerate
        let collinear = [CGPoint(x: 0.0, y: 0.0), CGPoint(x: 0.5, y: 0.5), CGPoint(x: 1.0, y: 1.0)]
        let collinearTargets: [SCNVector3] = [
            SCNVector3(0.0, 0.0, 0), SCNVector3(0.1, 0.1, 0), SCNVector3(0.2, 0.2, 0)
        ]
        XCTAssertNil(
            HandMeshGeometryBuilder.affineImageToMeshFit(
                imageLandmarks: collinear,
                meshLandmarks: collinearTargets
            ),
            "Collinear correspondences must return nil"
        )
    }
}
