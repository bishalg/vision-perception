import CoreGraphics
@testable import CurvPalmOverlay
import CurvCameraOverlay
import SwiftUI
import XCTest

final class GuideSpecTests: XCTestCase {
    private func docJSON(_ segmentsJSON: String) -> Data {
        Data("{\"segments\": \(segmentsJSON)}".utf8)
    }

    // MARK: - Segment loading

    func testLoadSegmentsFlattensDottedKeys() throws {
        let data = docJSON("""
        {"fingers": {"pinky": {"move": [0.78, 0.62],
          "curves": [[0.84, 0.50, 0.90, 0.32, 0.86, 0.18]]}}}
        """)
        let segments = try GuideSpec.loadSegments(from: data)
        XCTAssertEqual(segments.count, 1)
        XCTAssertNotNil(segments["fingers.pinky"])
    }

    func testMalformedSegmentThrows() {
        let data = docJSON("""
        {"fingers": {"pinky": {"move": [0.78, 0.62],
          "curves": [[0.84, 0.50, 0.90, 0.32]]}}}
        """)
        XCTAssertThrowsError(try GuideSpec.loadSegments(from: data))
    }

    func testEmptyDocumentThrows() {
        XCTAssertThrowsError(try GuideSpec.loadSegments(from: Data("{\"segments\": {}}".utf8)))
        XCTAssertThrowsError(try GuideSpec.loadSegments(from: Data("not json".utf8)))
    }

    // MARK: - Chaining (fill-once invariant)

    func testLoopChainingEndToStart() throws {
        let data = docJSON("""
        {"fingers": {
          "pinky": {"move": [0.78, 0.62],
            "curves": [[0.84, 0.50, 0.90, 0.32, 0.86, 0.18],
                       [0.85, 0.12, 0.80, 0.12, 0.78, 0.16],
                       [0.76, 0.24, 0.74, 0.34, 0.74, 0.42]]},
          "ring": {"move": [0.74, 0.42],
            "curves": [[0.74, 0.28, 0.74, 0.16, 0.70, 0.10]]}}}
        """)
        let segments = try GuideSpec.loadSegments(from: data)
        let pinkyEnd = try XCTUnwrap(segments["fingers.pinky"]?.endPoint)
        let ringStart = try XCTUnwrap(segments["fingers.ring"]?.startPoint)
        XCTAssertEqual(pinkyEnd.x, ringStart.x, accuracy: 1e-9)
        XCTAssertEqual(pinkyEnd.y, ringStart.y, accuracy: 1e-9)
    }

    // MARK: - Path building + mirror

    func testPathMapsNormalizedToRect() throws {
        let segment = GuideSegment(move: [0.5, 0.25], curves: [[0.6, 0.3, 0.7, 0.3, 0.8, 0.4]])
        let rect = CGRect(x: 10, y: 20, width: 200, height: 100)
        let path = GuideSpec.path(for: segment, in: rect)
        let bounds = path.boundingRect
        XCTAssertEqual(bounds.minX, 110, accuracy: 0.5)
        XCTAssertEqual(bounds.maxX, 170, accuracy: 0.5)
        XCTAssertEqual(bounds.minY, 45, accuracy: 0.5)
    }

    func testMirrorFlipsXAroundRectCenter() throws {
        let segment = GuideSegment(move: [0.2, 0.5], curves: [])
        let rect = CGRect(x: 0, y: 0, width: 200, height: 100)
        let normal = GuideSpec.path(for: segment, in: rect).boundingRect
        let mirrored = GuideSpec.path(for: segment, in: rect, mirror: true).boundingRect
        XCTAssertEqual(normal.minX + mirrored.minX, 200, accuracy: 0.5)
    }

    // MARK: - Theme

    func testThemeHexParses() throws {
        let data = Data("{\"colors\": {\"emerald\": \"#33D64A\"}}".utf8)
        let theme = try JSONDecoder().decode(GuideTheme.self, from: data)
        XCTAssertNotNil(theme.color("emerald"))
        XCTAssertNil(theme.color("missing"))
        XCTAssertNil(Color(hex: "not-a-hex"))
    }
}
