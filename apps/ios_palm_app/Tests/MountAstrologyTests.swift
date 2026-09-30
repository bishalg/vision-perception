import XCTest
import CurvPalmOverlay
@testable import PalmScanner

final class MountAstrologyTests: XCTestCase {

    // MARK: - Development derivation

    func testDevelopmentFromMillimetreRelief() {
        // Mesh-builder clamp domain is [-2.5, 4.0] mm.
        XCTAssertEqual(MountAstrology.development(heightMm: -1.0, prominence: nil), .deficient)
        XCTAssertEqual(MountAstrology.development(heightMm: 0.1, prominence: nil), .deficient)
        XCTAssertEqual(MountAstrology.development(heightMm: 0.3, prominence: nil), .balanced)
        XCTAssertEqual(MountAstrology.development(heightMm: 1.0, prominence: nil), .balanced)
        XCTAssertEqual(MountAstrology.development(heightMm: 1.4, prominence: nil), .developed)
        XCTAssertEqual(MountAstrology.development(heightMm: 2.5, prominence: nil), .developed)
        XCTAssertEqual(MountAstrology.development(heightMm: 2.6, prominence: nil), .prominent)
        XCTAssertEqual(MountAstrology.development(heightMm: 4.0, prominence: nil), .prominent)
    }

    func testDevelopmentFallsBackToProminenceWhenNoMm() {
        XCTAssertEqual(MountAstrology.development(heightMm: nil, prominence: .deficient), .deficient)
        XCTAssertEqual(MountAstrology.development(heightMm: nil, prominence: .average), .balanced)
        XCTAssertEqual(MountAstrology.development(heightMm: nil, prominence: .elevated), .developed)
        XCTAssertNil(MountAstrology.development(heightMm: nil, prominence: nil))
    }

    func testMmTakesPrecedenceOverProminence() {
        // Contradictory inputs: the measured mm value wins.
        XCTAssertEqual(
            MountAstrology.development(heightMm: 3.0, prominence: .deficient),
            .prominent
        )
    }

    // MARK: - Catalog completeness

    func testAllSevenMountsHaveProfiles() {
        let expected: Set<String> = ["jupiter", "saturn", "apollo", "mercury", "venus", "luna", "mars"]
        let provided = Set(MountAstrology.allProfiles.map(\.mount))
        XCTAssertEqual(provided, expected)

        for profile in MountAstrology.allProfiles {
            XCTAssertFalse(profile.displayName.isEmpty)
            XCTAssertFalse(profile.planetGlyph.isEmpty)
            XCTAssertFalse(profile.anatomicalLocation.isEmpty)
            XCTAssertFalse(profile.governs.isEmpty)
        }
    }

    func testEveryMountHasReadingAtEveryDevelopmentLevel() {
        for profile in MountAstrology.allProfiles {
            for level in MountDevelopment.allCases {
                let text = profile.reading(for: level)
                XCTAssertFalse(text.isEmpty, "\(profile.mount) missing reading for \(level.rawValue)")
                XCTAssertGreaterThan(text.count, 40, "\(profile.mount) reading for \(level.rawValue) is too thin")
            }
        }
    }

    func testProfileLookupIsCaseExact() {
        XCTAssertNotNil(MountAstrology.profile(for: "jupiter"))
        XCTAssertNil(MountAstrology.profile(for: "Jupiter"))
        XCTAssertNil(MountAstrology.profile(for: "nonexistent"))
    }

    // MARK: - Region identification integration

    func testRegionGeometryCoversAllSevenMountsFromLandmarks() {
        // PalmMount cases drive the detail sheet; the overlay must be able to
        // draw a polygon for each one from an ordinary upright landmark set.
        let landmarks = UprightPalm.landmarks
        guard let regions = PalmRegionGeometry.regions(from: landmarks) else {
            return XCTFail("PalmRegionGeometry.regions should succeed for 21 landmarks")
        }
        XCTAssertEqual(regions.mounts.count, 7)
        for mount in PalmMount.allCases {
            let polygon = regions.mountPolygon(mount)
            XCTAssertGreaterThanOrEqual(polygon.count, 4, "\(mount.rawValue) polygon is degenerate")
        }
    }

    func testJupiterPolygonSitsUnderTheIndexFinger() {
        // Region sanity: the Jupiter mount polygon's mean point must fall below
        // the index MCP (image space, y-down) and inside the palm region.
        let landmarks = UprightPalm.landmarks
        guard let regions = PalmRegionGeometry.regions(from: landmarks) else {
            return XCTFail("regions should build")
        }
        let polygon = regions.mountPolygon(.jupiter)
        let meanY = polygon.map(\.y).reduce(0, +) / CGFloat(polygon.count)
        XCTAssertLessThan(meanY, landmarks[5].y + 0.15,
                          "Jupiter region should sit at/below the index MCP row")
    }
}

/// Upright right-palm normalized landmarks (top-left origin), same fixture the
/// mesh builder tests use.
private enum UprightPalm {
    static let landmarks: [CGPoint] = [
        CGPoint(x: 0.50, y: 0.85), // 0 wrist
        CGPoint(x: 0.40, y: 0.72), CGPoint(x: 0.32, y: 0.62),
        CGPoint(x: 0.28, y: 0.54), CGPoint(x: 0.24, y: 0.48), // thumb
        CGPoint(x: 0.38, y: 0.50), CGPoint(x: 0.36, y: 0.38),
        CGPoint(x: 0.35, y: 0.28), CGPoint(x: 0.34, y: 0.20), // index
        CGPoint(x: 0.50, y: 0.48), CGPoint(x: 0.50, y: 0.34),
        CGPoint(x: 0.50, y: 0.24), CGPoint(x: 0.50, y: 0.15), // middle
        CGPoint(x: 0.62, y: 0.51), CGPoint(x: 0.63, y: 0.37),
        CGPoint(x: 0.64, y: 0.27), CGPoint(x: 0.65, y: 0.19), // ring
        CGPoint(x: 0.72, y: 0.55), CGPoint(x: 0.74, y: 0.44),
        CGPoint(x: 0.75, y: 0.36), CGPoint(x: 0.76, y: 0.29), // pinky
    ]
}
