import XCTest
@testable import PalmScanner

final class PalmScanModeTests: XCTestCase {
    func testAllModesExposeUniqueDistinctPresentationMetadata() {
        XCTAssertEqual(PalmScanMode.allCases.count, 4)
        var names = Set<String>()
        var subtitles = Set<String>()
        var icons = Set<String>()
        var shortTitles = Set<String>()
        for mode in PalmScanMode.allCases {
            XCTAssertEqual(mode.id, mode.rawValue)
            XCTAssertFalse(mode.displayName.isEmpty)
            XCTAssertFalse(mode.subtitle.isEmpty)
            XCTAssertFalse(mode.iconName.isEmpty)
            XCTAssertFalse(mode.shortTitle.isEmpty)
            names.insert(mode.displayName)
            subtitles.insert(mode.subtitle)
            icons.insert(mode.iconName)
            shortTitles.insert(mode.shortTitle)
        }
        XCTAssertEqual(names.count, 4)
        XCTAssertEqual(subtitles.count, 4)
        XCTAssertEqual(icons.count, 4)
        XCTAssertEqual(shortTitles.count, 4)
    }

    func testKnownModesMapToExpectedValues() {
        XCTAssertEqual(PalmScanMode(rawValue: "still"), .stillPhoto)
        XCTAssertEqual(PalmScanMode(rawValue: "orbit"), .orbitScan)
        XCTAssertEqual(PalmScanMode(rawValue: "closeup"), .closeUpMosaic)
        XCTAssertEqual(PalmScanMode(rawValue: "fingertip"), .fingertipScan)
        XCTAssertNil(PalmScanMode(rawValue: "video"))
        XCTAssertEqual(PalmScanMode.stillPhoto.displayName, "Still Photo")
        XCTAssertEqual(PalmScanMode.orbitScan.shortTitle, "3D Orbit")
        XCTAssertEqual(PalmScanMode.closeUpMosaic.subtitle, "Fine crease detail tiles")
        XCTAssertEqual(PalmScanMode.fingertipScan.subtitle, "Dermatoglyphic pattern class")
        XCTAssertEqual(PalmScanMode.stillPhoto.iconName, "camera.fill")
    }
}
