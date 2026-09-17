import XCTest
import CoreGraphics
@testable import PalmScanner

final class CreaseSplineGeometryTests: XCTestCase {
    private let diagonal = (0..<9).map { CGPoint(x: CGFloat($0) * 10, y: CGFloat($0) * 10) }

    func testStraightLineResamplesToEvenlySpacedEndpointsAndMidpoints() {
        let points = CreaseSpline.resampleToControlPoints(diagonal, targetCount: 4)
        XCTAssertEqual(points.count, 4)
        XCTAssertEqual(points.first, diagonal.first)
        XCTAssertEqual(points.last, diagonal.last)
        for (index, point) in points.enumerated() {
            XCTAssertEqual(point.x, CGFloat(index) * 80 / 3, accuracy: 1e-9)
            XCTAssertEqual(point.y, CGFloat(index) * 80 / 3, accuracy: 1e-9)
        }
    }

    func testResamplingKeepsShortCurvesAndDegenerateInputVerbatim() {
        XCTAssertEqual(CreaseSpline.resampleToControlPoints(Array(diagonal.prefix(3))), Array(diagonal.prefix(3)))
        let dupes = Array(repeating: CGPoint(x: 2, y: 3), count: 12)
        XCTAssertEqual(CreaseSpline.resampleToControlPoints(dupes), [CGPoint(x: 2, y: 3), CGPoint(x: 2, y: 3)])
    }

    func testCreaseSplineInitResamplesDensePolylinesToFourControlPoints() {
        let spline = CreaseSpline(lineType: .heart, points: diagonal, confidence: 0.9, tortuosity: 1.1, arcLengthPx: 100)
        XCTAssertEqual(spline.points.count, 4)
        XCTAssertEqual(spline.points.first, diagonal.first)
        XCTAssertEqual(spline.points.last, diagonal.last)
        XCTAssertFalse(spline.isUserAdjusted)
        XCTAssertEqual(spline.engineKey, "heart_line")
    }

    func testReplacingKeepsIdentityAndFlagsUserAdjustment() {
        var spline = CreaseSpline(id: "fixture", lineType: .heart, points: [CGPoint.zero, CGPoint(x: 1, y: 0)],
                                  confidence: 0.5, tortuosity: 1.0, arcLengthPx: 50)
        let relabeled = spline.replacing(lineType: .fate)
        XCTAssertEqual(relabeled.id, "fixture")
        XCTAssertEqual(relabeled.lineType, .fate)
        XCTAssertEqual(relabeled.engineKey, "fate_line")
        XCTAssertTrue(relabeled.isUserAdjusted)
        XCTAssertFalse(spline.isUserAdjusted)
        let moved = spline.replacing(points: diagonal)
        XCTAssertEqual(moved.points.count, 4)
        XCTAssertEqual(moved.lineType, .heart)
        XCTAssertTrue(moved.isUserAdjusted)
        let untouched = spline.replacing(markAdjusted: false)
        XCTAssertFalse(untouched.isUserAdjusted)
        XCTAssertEqual(untouched.points, spline.points)
    }

    func testDenseEvaluationStartsAndEndsAtControlPoints() {
        let spline = CreaseSpline(lineType: .head, points: [CGPoint.zero, CGPoint(x: 1, y: 1)],
                                  confidence: 0.5, tortuosity: 1, arcLengthPx: 10)
        XCTAssertEqual(spline.evaluateDensePoints(), [CGPoint.zero, CGPoint(x: 1, y: 1)])
        XCTAssertEqual(CreaseSpline(lineType: .head, points: [], confidence: 0.5, tortuosity: 1, arcLengthPx: 10).evaluateDensePoints(), [])
        let curve = CreaseSpline(lineType: .head, points: [CGPoint.zero, CGPoint(x: 0, y: 1), CGPoint(x: 1, y: 1)],
                                 confidence: 0.5, tortuosity: 1, arcLengthPx: 10).evaluateDensePoints(samplesPerSegment: 4)
        XCTAssertEqual(curve.first, CGPoint.zero)
        XCTAssertEqual(curve.last, CGPoint(x: 1, y: 1))
        XCTAssertEqual(curve.count, 9)
    }

    func testPredictionPointsAreFiniteNormalizedPairs() {
        let normalized = diagonal.map { CGPoint(x: $0.x / 80, y: $0.y / 80) }
        let spline = CreaseSpline(lineType: .life, points: normalized, confidence: 0.8, tortuosity: 1.2, arcLengthPx: 90)
        let payload = spline.predictionPointsImageNorm
        XCTAssertFalse(payload.isEmpty)
        for pair in payload {
            XCTAssertEqual(pair.count, 2)
            XCTAssertTrue(pair[0].isFinite && pair[1].isFinite)
            XCTAssertTrue((0...1.2).contains(pair[0]) && (0...1.2).contains(pair[1]))
        }
    }

    func testCreaseLineTypeMetadataAndAnatomicalAliases() {
        XCTAssertEqual(CreaseLineType.allCases.count, 4)
        let expected: [(CreaseLineType, String, String)] = [
            (.heart, "#FF2D55", "distal_transverse_crease"),
            (.head, "#007AFF", "proximal_transverse_crease"),
            (.life, "#34C759", "thenar_longitudinal_crease"),
            (.fate, "#AF52DE", "median_longitudinal_crease")
        ]
        for (lineType, hex, anatomical) in expected {
            XCTAssertEqual(lineType.hexColor, hex)
            XCTAssertEqual(lineType.anatomicalName, anatomical)
            XCTAssertNotNil(CreaseLineType.fromAnatomicalName(anatomical))
            XCTAssertEqual(CreaseLineType.fromAnatomicalName(anatomical.uppercased()), lineType)
        }
        let aliases = ["dtc": CreaseLineType.heart, "PTC": .head, "tlc": .life, "median_longitudinal": .fate]
        for (alias, lineType) in aliases {
            XCTAssertEqual(CreaseLineType.fromAnatomicalName(alias), lineType)
        }
        XCTAssertNil(CreaseLineType.fromAnatomicalName("mystery_crease"))
        for lineType in CreaseLineType.allCases {
            XCTAssertFalse(lineType.displayNameKey.isEmpty)
        }
    }
}
