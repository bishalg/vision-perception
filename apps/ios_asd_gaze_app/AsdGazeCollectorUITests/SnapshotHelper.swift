//
//  SnapshotHelper.swift
//  AsdGazeCollectorUITests
//
//  Helper for Fastlane snapshot automated screenshots.
//  Usage: fastlane screenshots (or fastlane screenshots_with_mode)
//

import XCTest

class SnapshotHelper {
    static let app = XCUIApplication()
    
    // MARK: - Setup
    
    static func setupSnapshot(app: XCUIApplication) {
        // Configure for snapshot mode
        app.launchArguments = ["-snapshot-mode", "true", "-skip-onboarding", "true"]
        app.launch()
        
        // Wait for app to be ready
        let snapshotReady = app.otherElements["SnapshotReady"].waitForExistence(timeout: 10)
        XCTAssertTrue(snapshotReady, "App did not enter snapshot mode in time")
    }
    
    // MARK: - Screen Navigation
    
    static func navigateToScreen(_ screenName: String) {
        switch screenName {
        case "onboarding":
            navigateToOnboarding()
        case "calibration":
            navigateToCalibration()
        case "screening":
            navigateToScreening()
        case "results":
            navigateToResults()
        case "dashboard":
            navigateToDashboard()
        case "settings":
            navigateToSettings()
        default:
            XCTFail("Unknown screen: \(screenName)")
        }
    }
    
    private static func navigateToOnboarding() {
        // Onboarding is the first screen in snapshot mode
        sleep(1)
    }
    
    private static func navigateToCalibration() {
        // Tap through onboarding to reach calibration
        let continueButton = app.buttons["Continue"]
        if continueButton.waitForExistence(timeout: 5) {
            continueButton.tap()
        }
        sleep(2)
    }
    
    private static func navigateToScreening() {
        // Navigate through calibration to screening
        navigateToCalibration()
        
        let startButton = app.buttons["Start Screening"]
        if startButton.waitForExistence(timeout: 10) {
            startButton.tap()
        }
        sleep(3)
    }
    
    private static func navigateToResults() {
        // Complete a screening session to reach results
        navigateToScreening()
        
        // Wait for screening to complete (mocked in snapshot mode)
        let doneButton = app.buttons["Done"]
        if doneButton.waitForExistence(timeout: 15) {
            doneButton.tap()
        }
        sleep(2)
    }
    
    private static func navigateToDashboard() {
        // Dashboard is accessible from home
        let dashboardTab = app.tabBars.buttons["Dashboard"]
        if dashboardTab.waitForExistence(timeout: 5) {
            dashboardTab.tap()
        }
        sleep(1)
    }
    
    private static func navigateToSettings() {
        let settingsTab = app.tabBars.buttons["Settings"]
        if settingsTab.waitForExistence(timeout: 5) {
            settingsTab.tap()
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
        // 1. Onboarding - Welcome
        navigateToOnboarding()
        snapshot("01_onboarding_welcome")
        
        // 2. Onboarding - Child Profile
        app.buttons["Continue"].tap()
        sleep(1)
        snapshot("02_onboarding_child_profile")
        
        // 3. Onboarding - Language Selection
        app.buttons["Continue"].tap()
        sleep(1)
        snapshot("03_onboarding_language")
        
        // 4. Onboarding - Consent
        app.buttons["Continue"].tap()
        sleep(1)
        snapshot("04_onboarding_consent")
        
        // 5. Onboarding - Environment Setup
        app.buttons["Continue"].tap()
        sleep(1)
        snapshot("05_onboarding_environment")
        
        // 6. Calibration - Bubble Pop
        navigateToCalibration()
        snapshot("06_calibration_bubble")
        
        // 7. Screening - GeoPref Split Screen
        navigateToScreening()
        snapshot("07_screening_geopref")
        
        // 8. Screening - Face Scanning
        sleep(5) // Let stimulus play
        snapshot("08_screening_face")
        
        // 9. Screening - Joint Attention
        sleep(5)
        snapshot("09_screening_joint_attention")
        
        // 10. M-CHAT Survey
        sleep(5)
        snapshot("10_mchat_survey")
        
        // 11. Results - Summary
        navigateToResults()
        snapshot("11_results_summary")
        
        // 12. Results - Biomarkers
        app.swipeLeft()
        sleep(1)
        snapshot("12_results_biomarkers")
        
        // 13. Dashboard - Overview
        navigateToDashboard()
        snapshot("13_dashboard_overview")
        
        // 14. Dashboard - Trends
        app.swipeUp()
        sleep(1)
        snapshot("14_dashboard_trends")
        
        // 15. Settings
        navigateToSettings()
        snapshot("15_settings")
    }
}

// MARK: - XCUIApplication Extension for Fastlane

extension XCUIApplication {
    func snapshot(_ name: String) {
        // This is a no-op in tests; Fastlane's snapshot tool intercepts this
        // The actual screenshot is taken by the snapshot runner
    }
}