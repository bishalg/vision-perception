import CoreGraphics
@testable import CurvPalmOverlay
import XCTest

final class HandBiometricsTests: XCTestCase {
    func testInsufficientLandmarksReturnsNil() {
        XCTAssertNil(HandBiometricsCalculator.compute(landmarks: []))
        let shortList: [[Float]] = (0..<20).map { _ in [0.5, 0.5] }
        XCTAssertNil(HandBiometricsCalculator.compute(landmarks: shortList))
    }

    func testAnatomicalPadMapping() {
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("venus"), .thenar)
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("luna"), .hypothenar)
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("jupiter"), .radialInterdigital)
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("saturn"), .centralInterdigital)
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("apollo"), .ringInterdigital)
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("mercury"), .ulnarInterdigital)
        XCTAssertEqual(AnatomicalPad.fromAstrologicalMount("mars"), .palmarFossa)

        for pad in AnatomicalPad.allCases {
            XCTAssertFalse(pad.landmarkAnchors.isEmpty)
            XCTAssertFalse(pad.astrologicalMountName.isEmpty)
            XCTAssertFalse(pad.scientificDisplayName.isEmpty)
            XCTAssertEqual(AnatomicalPad.fromAstrologicalMount(pad.astrologicalMountName), pad)
        }
    }

    func testSyntheticHandBiometrics() {
        let scaffold = PalmRegionGeometry.scaffoldLandmarks()
        let lms: [[Float]] = scaffold.map { [Float($0.x), Float($0.y)] }

        guard let report = HandBiometricsCalculator.compute(
            landmarks: lms,
            imageWidth: 1000.0,
            imageHeight: 1000.0,
            referencePalmWidthMm: 80.0,
            padDepthMm: ["thenar": 2.5, "palmar_fossa": -1.0],
            handSide: "right"
        ) else {
            XCTFail("Failed to compute hand biometrics from scaffold")
            return
        }

        // Palm dimensions
        XCTAssertEqual(report.palm.metacarpalBreadthMm, 80.0, accuracy: 0.01)
        XCTAssertGreaterThan(report.palm.palmLengthMm, 40.0)
        XCTAssertGreaterThan(report.palm.totalHandLengthMm, report.palm.palmLengthMm)
        XCTAssertGreaterThan(report.palm.aspectRatio, 0.4)
        XCTAssertLessThan(report.palm.aspectRatio, 1.8)

        // Digits
        XCTAssertEqual(report.digits.count, 5)
        for d in report.digits {
            XCTAssertGreaterThan(d.totalLengthMm, 10.0, "Digit \(d.name) length too small")
            XCTAssertGreaterThan(d.proximalPhalanxMm, 0.0)
            XCTAssertGreaterThan(d.distalPhalanxMm, 0.0)
        }

        // 2D:4D ratio
        XCTAssertGreaterThan(report.ratios.ratio2D4D, 0.70)
        XCTAssertLessThan(report.ratios.ratio2D4D, 1.30)
        XCTAssertGreaterThan(report.ratios.ratio2D3D, 0.60)
        XCTAssertLessThan(report.ratios.ratio2D3D, 1.10)

        // Spans
        XCTAssertGreaterThan(report.spans.thumbIndexAngleDeg, 5.0)
        XCTAssertGreaterThan(report.spans.indexMiddleAngleDeg, 1.0)

        // Anatomical pads
        XCTAssertEqual(report.pads.count, 7)
        let thenar = report.pads.first { $0.padName == "thenar" }
        XCTAssertNotNil(thenar)
        XCTAssertEqual(thenar?.heightMm ?? 0, 2.5, accuracy: 0.01)

        let fossa = report.pads.first { $0.padName == "palmar_fossa" }
        XCTAssertNotNil(fossa)
        XCTAssertEqual(fossa?.heightMm ?? 0, -1.0, accuracy: 0.01)

        // Anatomical flexion creases
        XCTAssertEqual(report.creases.count, 4)
        let dtc = report.creases.first { $0.anatomicalName == "distal_transverse_crease" }
        XCTAssertNotNil(dtc)
        XCTAssertEqual(dtc?.traditionalAlias, "heart")
        XCTAssertEqual(dtc?.scientificDisplayName, "Distal Transverse Crease")
        XCTAssertGreaterThan(dtc?.arcLengthMm ?? 0, 20.0)

        let ptc = report.creases.first { $0.anatomicalName == "proximal_transverse_crease" }
        XCTAssertNotNil(ptc)
        XCTAssertEqual(ptc?.traditionalAlias, "head")

        let tlc = report.creases.first { $0.anatomicalName == "thenar_longitudinal_crease" }
        XCTAssertNotNil(tlc)
        XCTAssertEqual(tlc?.traditionalAlias, "life")

        let mlc = report.creases.first { $0.anatomicalName == "median_longitudinal_crease" }
        XCTAssertNotNil(mlc)
        XCTAssertEqual(mlc?.traditionalAlias, "fate")
    }
}
