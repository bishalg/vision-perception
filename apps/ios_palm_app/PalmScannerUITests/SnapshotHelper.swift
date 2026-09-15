//
//  SnapshotHelper.swift
//  PalmScannerUITests
//
//  Helper for Fastlane snapshot automated screenshots.
//  Usage: fastlane screenshots
//

import XCTest

class SnapshotHelper {
    static let app = XCUIApplication()
    
    // MARK: - Setup
    
    static func setupSnapshot(app: XCUIApplication) {
        app.launchArguments = ["-snapshot-mode", "true"]
        app.launch()
        
        let snapshotReady = app.otherElements["SnapshotReady"].waitForExistence(timeout: 10)
        XCTAssertTrue(snapshotReady, "App did not enter snapshot mode in time")
    }
    
    // MARK: - Screen Navigation
    
    static func navigateToScreen(_ screenName: String) {
        switch screenName {
        case "scanner":
            navigateToScanner()
        case "result":
            navigateToResult()
        case "reading":
            navigateToReading()
        case "settings":
            navigateToSettings()
        default:
            XCTFail("Unknown screen: \(screenName)")
        }
    }
    
    private static func navigateToScanner() {
        // Scanner is the main screen
        sleep(1)
    }
    
    private static func navigateToResult() {
        // Perform a scan to get result
        let captureButton = app.buttons["Capture"]
        if captureButton.waitForExistence(timeout: 5) {
            captureButton.tap()
        }
        sleep(3)
    }
    
    private static func navigateToReading() {
        navigateToResult()
        let viewReading = app.buttons["View Reading"]
        if viewReading.waitForExistence(timeout: 5) {
            viewReading.tap()
        }
        sleep(2)
    }
    
    private static func navigateToSettings() {
        let settingsButton = app.buttons["Settings"]
        if settingsButton.waitForExistence(timeout: 5) {
            settingsButton.tap()
        }
        sleep(1)
    }
    
    // MARK: - Snapshot Capture
    
    static func snapshot(_ name: String, waitFor: TimeInterval = 1) {
        sleep(UInt32(waitFor))
        snapshot(name)
    }
    
    // MARK: - Common Screenshots
    
    static func captureAllScreens() {
        // 1. Scanner - Ready State
        navigateToScanner()
        snapshot("01_scanner_ready")
        
        // 2. Scanner - Hand Detected
        sleep(2)
        snapshot("02_scanner_hand_detected")
        
        // 3. Scanner - Capture
        let captureButton = app.buttons["Capture"]
        if captureButton.exists {
            captureButton.tap()
        }
        sleep(3)
        snapshot("03_scanner_capturing")
        
        // 4. Result - Crease Overlay
        navigateToResult()
        snapshot("04_result_creases")
        
        // 5. Result - Magnifying Loupe
        app.swipeLeft()
        sleep(1)
        snapshot("05_result_loupe")
        
        // 6. Reading - Samudrika
        navigateToReading()
        snapshot("06_reading_samudrika")
        
        // 7. Reading - Scroll
        app.swipeUp()
        sleep(1)
        snapshot("07_reading_scroll")
        
        // 8. 3D Orbit Scan
        let orbitButton = app.segmentedControls.buttons["3D Orbit"]
        if orbitButton.exists {
            orbitButton.tap()
        }
        sleep(2)
        snapshot("08_orbit_scan")
        
        // 9. Settings
        navigateToSettings()
        snapshot("09_settings")
    }
}

extension XCUIApplication {
    func snapshot(_ name: String) {
        // Fastlane snapshot intercepts this
    }
}