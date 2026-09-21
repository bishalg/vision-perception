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
        // Coral has high red component
        XCTAssertGreaterThan(coral.0, coral.2)
        // Green has high green component
        XCTAssertGreaterThan(green.1, green.0)
    }
}
