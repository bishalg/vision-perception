@testable import AsdGazeCollector
import XCTest

/// SIM-7: Release builds declare the hardware the app cannot work without.
/// Apple does not allow tightening `UIRequiredDeviceCapabilities` after the first
/// release, so the set is pinned here.
final class BuildConfigurationTests: XCTestCase {
    func testRequiredDeviceCapabilitiesDeclareArkitAndFrontCamera() throws {
        let info = try XCTUnwrap(Bundle.main.infoDictionary)
        let caps = try XCTUnwrap(info["UIRequiredDeviceCapabilities"] as? [String])
        XCTAssertTrue(caps.contains("arkit"))
        XCTAssertTrue(caps.contains("front-facing-camera"))
    }
}
