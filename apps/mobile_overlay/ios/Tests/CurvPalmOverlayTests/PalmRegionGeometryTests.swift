import CoreGraphics
@testable import CurvPalmOverlay
import XCTest

final class PalmRegionGeometryTests: XCTestCase {
    // MARK: - Incomplete Inputs

    func testInsufficientLandmarksReturnsNil() {
        XCTAssertNil(PalmRegionGeometry.regions(from: []))
        let shortList = (0..<20).map { _ in CGPoint(x: 0.5, y: 0.5) }
        XCTAssertNil(PalmRegionGeometry.regions(from: shortList))
    }

    // MARK: - Scaffold Verification

    func testScaffoldLandmarksWellFormed() {
        let scaffold = PalmRegionGeometry.scaffoldLandmarks()
        XCTAssertEqual(scaffold.count, 21, "Scaffold must produce exactly 21 landmarks")

        for (index, pt) in scaffold.enumerated() {
            XCTAssertFalse(pt.x.isNaN, "Point \(index) x is NaN")
            XCTAssertFalse(pt.y.isNaN, "Point \(index) y is NaN")
            XCTAssertGreaterThanOrEqual(pt.x, 0.0, "Point \(index) x out of bounds")
            XCTAssertLessThanOrEqual(pt.x, 1.0, "Point \(index) x out of bounds")
            XCTAssertGreaterThanOrEqual(pt.y, 0.0, "Point \(index) y out of bounds")
            XCTAssertLessThanOrEqual(pt.y, 1.0, "Point \(index) y out of bounds")
        }

        let regions = PalmRegionGeometry.scaffoldRegions()
        XCTAssertEqual(regions.mounts.count, 7)
        XCTAssertEqual(regions.corridors.count, 3)
    }

    // MARK: - Mount Topologies & Areas

    func testMountTopologyAndNonZeroArea() {
        let landmarks = PalmRegionGeometry.scaffoldLandmarks()
        guard let regions = PalmRegionGeometry.regions(from: landmarks) else {
            XCTFail("Failed to compute regions from scaffold landmarks")
            return
        }

        let expectedMounts: [PalmMount] = [.jupiter, .saturn, .apollo, .mercury, .venus, .luna, .mars]
        XCTAssertEqual(regions.mounts.count, expectedMounts.count)

        for mount in expectedMounts {
            let poly = regions.mountPolygon(mount)
            XCTAssertGreaterThanOrEqual(poly.count, 3, "Mount \(mount) must have >= 3 vertices")

            // Shoelace formula for polygon area
            var area: CGFloat = 0.0
            let n = poly.count
            for i in 0..<n {
                let p1 = poly[i]
                let p2 = poly[(i + 1) % n]
                area += (p1.x * p2.y - p2.x * p1.y)
            }
            area = abs(area) * 0.5

            XCTAssertGreaterThan(area, 1e-4, "Mount \(mount) must have non-zero positive area")
            XCTAssertFalse(hasSelfIntersection(poly), "Mount \(mount) polygon must not self-intersect")
        }
    }

    func testMercuryApolloAdjacentBoundary() {
        let landmarks = PalmRegionGeometry.scaffoldLandmarks()
        guard let regions = PalmRegionGeometry.regions(from: landmarks) else {
            XCTFail("Failed to compute regions")
            return
        }
        let apollo = regions.mountPolygon(.apollo)
        let mercury = regions.mountPolygon(.mercury)
        // Apollo topB is ringMid (vertex 1), Mercury topA is ringMid (vertex 0)
        XCTAssertEqual(apollo[1].x, mercury[0].x, accuracy: 1e-4, "Mercury must share boundary with Apollo")
        XCTAssertEqual(apollo[1].y, mercury[0].y, accuracy: 1e-4, "Mercury must share boundary with Apollo")
    }

    private func hasSelfIntersection(_ poly: [CGPoint]) -> Bool {
        func ccw(_ a: CGPoint, _ b: CGPoint, _ c: CGPoint) -> Bool {
            (c.y - a.y) * (b.x - a.x) > (b.y - a.y) * (c.x - a.x)
        }
        func intersect(_ a: CGPoint, _ b: CGPoint, _ c: CGPoint, _ d: CGPoint) -> Bool {
            ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d)
        }
        let n = poly.count
        guard n >= 4 else { return false }
        for i in 0..<(n - 1) {
            let startJ = i + 2
            guard startJ < n else { continue }
            for j in startJ..<n {
                if i == 0 && j == n - 1 { continue }
                if intersect(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n]) {
                    return true
                }
            }
        }
        return false
    }

    // MARK: - Corridor Structures

    func testCorridorTopologyAndEchoes() {
        let landmarks = PalmRegionGeometry.scaffoldLandmarks()
        guard let regions = PalmRegionGeometry.regions(from: landmarks) else {
            XCTFail("Failed to compute regions")
            return
        }

        let expectedCorridors: [PalmCorridor] = [.heart, .head, .life]
        let corridorMap = Dictionary(uniqueKeysWithValues: regions.corridors.map { ($0.corridor, ($0.center, $0.echoes)) })

        for corridor in expectedCorridors {
            guard let (center, echoes) = corridorMap[corridor] else {
                XCTFail("Missing corridor \(corridor)")
                continue
            }
            XCTAssertGreaterThanOrEqual(center.count, 4, "Corridor \(corridor) center polyline must have >= 4 points")
            XCTAssertEqual(echoes.count, 2, "Corridor \(corridor) must produce exactly 2 parallel echoes")

            for echo in echoes {
                XCTAssertEqual(echo.count, center.count, "Echo must have same point count as center polyline")
                for pt in echo {
                    XCTAssertFalse(pt.x.isNaN)
                    XCTAssertFalse(pt.y.isNaN)
                }
            }
        }
    }

    // MARK: - Handedness & Anatomical Invariance

    func testLeftHandAnatomicalRegions() {
        // Simulate a left hand: horizontally flipped scaffold (x' = 1 - x)
        let rightLandmarks = PalmRegionGeometry.scaffoldLandmarks()
        let leftLandmarks = rightLandmarks.map { CGPoint(x: 1.0 - $0.x, y: $0.y) }

        guard let leftRegions = PalmRegionGeometry.regions(from: leftLandmarks) else {
            XCTFail("Failed to compute left hand regions")
            return
        }

        // On left hand:
        // pinky (pt 17) is on the left (smaller x)
        // index (pt 5) is on the right (larger x)
        let leftPinky = leftLandmarks[17]
        let leftIndex = leftLandmarks[5]
        XCTAssertLessThan(leftPinky.x, leftIndex.x, "Left hand pinky should have smaller x than index")

        // Mercury mount is under the pinky, Jupiter mount is under the index
        let mercuryPoly = leftRegions.mountPolygon(.mercury)
        let jupiterPoly = leftRegions.mountPolygon(.jupiter)
        let mercuryCentroidX = mercuryPoly.map(\.x).reduce(0, +) / CGFloat(mercuryPoly.count)
        let jupiterCentroidX = jupiterPoly.map(\.x).reduce(0, +) / CGFloat(jupiterPoly.count)

        XCTAssertLessThan(mercuryCentroidX, jupiterCentroidX, "Mercury mount must remain on pinky (ulnar) side")

        // Venus mount is under the thumb (larger x on left hand)
        let venusPoly = leftRegions.mountPolygon(.venus)
        let venusCentroidX = venusPoly.map(\.x).reduce(0, +) / CGFloat(venusPoly.count)
        let lunaPoly = leftRegions.mountPolygon(.luna)
        let lunaCentroidX = lunaPoly.map(\.x).reduce(0, +) / CGFloat(lunaPoly.count)

        XCTAssertGreaterThan(venusCentroidX, lunaCentroidX, "Venus mount must remain on radial/thumb side")
    }

    // MARK: - Degenerate Input Safety

    func testCoincidentPointsDoNotCrash() {
        // All 21 landmarks at exact same point (0.5, 0.5)
        let collapsed = (0..<21).map { _ in CGPoint(x: 0.5, y: 0.5) }
        let regions = PalmRegionGeometry.regions(from: collapsed)
        XCTAssertNotNil(regions, "Degenerate points should still safely produce regions without fatalError")

        if let regions {
            for (_, poly) in regions.mounts {
                for pt in poly {
                    XCTAssertFalse(pt.x.isNaN)
                    XCTAssertFalse(pt.y.isNaN)
                }
            }
        }
    }
}
