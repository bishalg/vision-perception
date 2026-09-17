import XCTest

final class ScannerSmokeTests: XCTestCase {
    func testScannerDisclosureHandSelectionAndForegroundRecovery() {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()

        let disclosure = app.staticTexts["For entertainment and cultural exploration"]
        XCTAssertTrue(disclosure.waitForExistence(timeout: 20))
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "not scientific predictions")).firstMatch.exists)
        let privacy = app.descendants(matching: .any).matching(identifier: "Privacy Policy").firstMatch
        XCTAssertTrue(privacy.exists)
        XCTAssertTrue(privacy.isHittable)

        let left = app.buttons["Left Hand"]
        let right = app.buttons["Right Hand"]
        XCTAssertTrue(left.waitForExistence(timeout: 5))
        XCTAssertTrue(left.isHittable)
        left.tap()
        XCTAssertEqual(left.value as? String, "Active")
        XCTAssertEqual(right.value as? String, "Inactive")
        right.tap()
        XCTAssertEqual(right.value as? String, "Active")
        XCTAssertEqual(left.value as? String, "Inactive")

        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(disclosure.waitForExistence(timeout: 10))
        XCTAssertEqual(right.value as? String, "Active")
        XCTAssertTrue(left.isHittable)
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = "Scanner after foreground recovery"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
