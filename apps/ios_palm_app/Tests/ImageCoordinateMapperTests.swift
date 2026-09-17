import XCTest
import CoreGraphics
@testable import PalmScanner

final class ImageCoordinateMapperTests: XCTestCase {
    func testAspectFitLetterboxesTheLongerViewAxis() {
        let wide = ImageCoordinateMapper.imageRect(imageSize: CGSize(width: 100, height: 50),
                                                   viewSize: CGSize(width: 100, height: 100), mode: .fit)
        XCTAssertEqual(wide, CGRect(x: 0, y: 25, width: 100, height: 50))
        let tall = ImageCoordinateMapper.imageRect(imageSize: CGSize(width: 50, height: 100),
                                                   viewSize: CGSize(width: 100, height: 100), mode: .fit)
        XCTAssertEqual(tall, CGRect(x: 25, y: 0, width: 50, height: 100))
    }

    func testAspectFillOverflowsTheLongerImageAxis() {
        let rect = ImageCoordinateMapper.imageRect(imageSize: CGSize(width: 100, height: 50),
                                                   viewSize: CGSize(width: 50, height: 50), mode: .fill)
        XCTAssertEqual(rect, CGRect(x: -25, y: 0, width: 100, height: 50))
    }

    func testDegenerateSizesFallBackToFullViewRect() {
        for imageSize in [CGSize.zero, CGSize(width: 1, height: 1)] {
            for viewSize in [CGSize.zero, CGSize(width: 1, height: 400)] {
                XCTAssertEqual(ImageCoordinateMapper.imageRect(imageSize: imageSize, viewSize: viewSize, mode: .fit),
                               CGRect(origin: .zero, size: viewSize))
            }
        }
    }

    func testMapAndUnmapAreInversesInsideTheRect() {
        let rect = CGRect(x: 10, y: 20, width: 200, height: 100)
        let normalized = CGPoint(x: 0.25, y: 0.75)
        let viewPoint = ImageCoordinateMapper.map(normalized, into: rect)
        XCTAssertEqual(viewPoint, CGPoint(x: 60, y: 95))
        XCTAssertEqual(ImageCoordinateMapper.unmap(viewPoint, from: rect), normalized)
    }

    func testUnmapClampsOutsidePointsToUnitSquare() {
        let rect = CGRect(x: 0, y: 0, width: 100, height: 50)
        XCTAssertEqual(ImageCoordinateMapper.unmap(CGPoint(x: -30, y: 90), from: rect), CGPoint(x: 0, y: 1))
        XCTAssertEqual(ImageCoordinateMapper.unmap(CGPoint(x: 500, y: 900), from: rect), CGPoint(x: 1, y: 1))
    }

    func testUnmapOnDegenerateRectReturnsZero() {
        XCTAssertEqual(ImageCoordinateMapper.unmap(CGPoint(x: 5, y: 5), from: CGRect(x: 0, y: 0, width: 1, height: 0)), .zero)
    }

    func testTransformedImageRectZoomsAboutTheViewCenter() {
        let base = ImageCoordinateMapper.transformedImageRect(imageSize: CGSize(width: 100, height: 100),
                                                              viewSize: CGSize(width: 100, height: 100),
                                                              mode: .fit, zoomScale: 2, panOffset: .zero)
        XCTAssertEqual(base, CGRect(x: -50, y: -50, width: 200, height: 200))
        let panned = ImageCoordinateMapper.transformedImageRect(imageSize: CGSize(width: 100, height: 100),
                                                                viewSize: CGSize(width: 100, height: 100),
                                                                mode: .fit, zoomScale: 1, panOffset: CGSize(width: 7, height: -3))
        XCTAssertEqual(panned, CGRect(x: 7, y: -3, width: 100, height: 100))
    }

    func testSmoothPointsPreservesEndsAndAveragesFlatRunsToTheSameValue() {
        XCTAssertEqual(ImageCoordinateMapper.smoothPoints([]), [])
        let short = [CGPoint(x: 0, y: 0), CGPoint(x: 1, y: 1)]
        XCTAssertEqual(ImageCoordinateMapper.smoothPoints(short), short)
        let line = (0..<9).map { CGPoint(x: CGFloat($0), y: 2) }
        XCTAssertEqual(ImageCoordinateMapper.smoothPoints(line).first, CGPoint(x: 0, y: 2))
        XCTAssertEqual(ImageCoordinateMapper.smoothPoints(line).last, CGPoint(x: 8, y: 2))
        for point in ImageCoordinateMapper.smoothPoints(line) {
            XCTAssertEqual(point.y, 2, accuracy: 1e-9)
        }
    }

    func testDistanceToSegmentHandlesEndpointsProjectionAndDegenerateSpan() {
        XCTAssertEqual(ImageCoordinateMapper.distanceToSegment(point: CGPoint(x: -3, y: 4), a: .zero, b: CGPoint(x: 4, y: 0)), 5)
        XCTAssertEqual(ImageCoordinateMapper.distanceToSegment(point: CGPoint(x: 9, y: 0), a: .zero, b: CGPoint(x: 4, y: 0)), 5)
        XCTAssertEqual(ImageCoordinateMapper.distanceToSegment(point: CGPoint(x: 3, y: 7), a: .zero, b: .zero), sqrt(58), accuracy: 1e-9)
    }

    func testDistanceToSplineHitsCurveAndReportsSentinelWithoutUsablePoints() {
        let rect = CGRect(x: 0, y: 0, width: 100, height: 100)
        XCTAssertEqual(ImageCoordinateMapper.distanceToSpline(point: CGPoint(x: 50, y: 60),
                                                              normPoints: [CGPoint(x: 0, y: 0.5), CGPoint(x: 1, y: 0.5)],
                                                              in: rect),
                       10, accuracy: 1e-9)
        XCTAssertEqual(ImageCoordinateMapper.distanceToSpline(point: .zero, normPoints: [CGPoint(x: 0.5, y: 0.5)], in: rect), 9999)
        XCTAssertEqual(ImageCoordinateMapper.distanceToSpline(point: .zero, normPoints: [], in: rect), 9999)
    }

    func testDenseSplineEvaluationPassesThroughControlPoints() {
        let square = [CGPoint.zero, CGPoint(x: 0, y: 1), CGPoint(x: 1, y: 1), CGPoint(x: 1, y: 0)]
        let dense = ImageCoordinateMapper.evaluateSplinePoints(points: square, samplesPerSegment: 10)
        XCTAssertEqual(dense.count, 31)
        for point in square {
            XCTAssertTrue(dense.contains(where: { hypot($0.x - point.x, $0.y - point.y) < 1e-9 }),
                          "dense spline should pass through \(point)")
        }
        XCTAssertEqual(ImageCoordinateMapper.evaluateSplinePoints(points: square.dropLast(2)), Array(square.dropLast(2)))
    }
}
