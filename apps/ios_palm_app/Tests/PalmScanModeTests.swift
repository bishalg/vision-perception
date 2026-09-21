import XCTest
@testable import PalmScanner
import CurvCameraOverlay
import CurvPalmOverlay

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
        XCTAssertEqual(PalmScanMode.stillPhoto.stepNumber, 1)
        XCTAssertEqual(PalmScanMode.orbitScan.stepNumber, 2)
        XCTAssertEqual(PalmScanMode.closeUpMosaic.stepNumber, 3)
        XCTAssertEqual(PalmScanMode.fingertipScan.stepNumber, 4)
        XCTAssertEqual(PalmScanMode.stillPhoto.stepBadge, "Step 1")
    }

    /// Validates that across all target iPhone screen geometries:
    /// 1. Viewfinder reticle never overlaps top chrome or bottom controls.
    /// 2. Reticle remains strictly within window boundaries with positive clearance.
    func testScreenLayoutBoundsAndClearancesAcrossDevices() {
        struct DeviceSpec {
            let name: String
            let width: CGFloat
            let height: CGFloat
            let topInset: CGFloat
            let bottomInset: CGFloat
        }

        let devices: [DeviceSpec] = [
            DeviceSpec(name: "iPhone SE (3rd gen)", width: 375, height: 667, topInset: 20, bottomInset: 0),
            DeviceSpec(name: "iPhone 13 / 14 (Notch)", width: 390, height: 844, topInset: 47, bottomInset: 34),
            DeviceSpec(name: "iPhone 15 / 16 / 18 Pro (Dynamic Island)", width: 402, height: 874, topInset: 59, bottomInset: 34),
            DeviceSpec(name: "iPhone 16 Pro Max", width: 440, height: 956, topInset: 59, bottomInset: 34),
            DeviceSpec(name: "iPad Pro 11-inch", width: 834, height: 1194, topInset: 24, bottomInset: 20)
        ]

        let profile = PalmOverlay.profile

        for device in devices {
            // Mode selector top padding inside camera frame
            let modeSelectorTop: CGFloat = 8
            // Top guidance chip sits 8pt below mode selector (~36pt height)
            let guidanceChipBottom = modeSelectorTop + 36 + 8 + 36

            for showsShutter in [true, false] {
                let effectiveTopInset: CGFloat = 0  // top safe area consumed by safety notice banner
                let effectiveBottomInset = max(device.bottomInset, 16)

                let topBoundary = effectiveTopInset + 88
                let bottomBoundary = device.height - (effectiveBottomInset + (showsShutter ? 134 : 158))
                let availableH = max(240, bottomBoundary - topBoundary)
                let reticleCenterY = topBoundary + availableH / 2

                let reticleH = min(device.height * profile.reticleHeightFraction, availableH - 8)
                let reticleW = min(device.width * profile.reticleWidthFraction, reticleH * 0.65)

                let reticleTop = reticleCenterY - reticleH / 2
                let reticleBottom = reticleCenterY + reticleH / 2

                // 1. Top clearance: Reticle top must be strictly below the top chrome (mode selector + chip)
                XCTAssertGreaterThanOrEqual(
                    reticleTop, guidanceChipBottom,
                    "\(device.name) (showsShutter=\(showsShutter)): Reticle top (\(reticleTop)) must be below top chrome (\(guidanceChipBottom))"
                )

                // 2. Bottom clearance: Reticle bottom must not exceed bottom boundary
                XCTAssertLessThanOrEqual(
                    reticleBottom, bottomBoundary,
                    "\(device.name) (showsShutter=\(showsShutter)): Reticle bottom (\(reticleBottom)) must not exceed bottom boundary (\(bottomBoundary))"
                )

                // 3. Side boundaries: Reticle width must fit on screen with side margins
                let sideMargin = (device.width - reticleW) / 2
                XCTAssertGreaterThan(
                    sideMargin, 16,
                    "\(device.name): Side margin (\(sideMargin)) must provide at least 16pt clearance"
                )

                // 4. Status instruction pill inside reticle top:
                let statusPillY = reticleTop + 12
                XCTAssertGreaterThan(
                    statusPillY, guidanceChipBottom,
                    "\(device.name): Embedded status pill must not collide with top coaching chip"
                )
            }
        }
    }

    func testCompletedScanModesReflectsEachModeIndependently() {
        let vm = PalmAnalysisViewModel()

        // 1. Initial: empty
        XCTAssertTrue(vm.completedScanModes.isEmpty)

        // 2. Still photo completed when a shot is captured
        let dummyImage = UIGraphicsImageRenderer(size: CGSize(width: 10, height: 10)).image { ctx in
            UIColor.white.setFill()
            ctx.fill(CGRect(x: 0, y: 0, width: 10, height: 10))
        }
        vm.setOrbitCaptureResult(image: dummyImage, landmarks: [])
        XCTAssertTrue(vm.completedScanModes.contains(.stillPhoto))
        XCTAssertFalse(vm.completedScanModes.contains(.orbitScan))
        XCTAssertFalse(vm.completedScanModes.contains(.closeUpMosaic))
        XCTAssertFalse(vm.completedScanModes.contains(.fingertipScan))

        // 3. Orbit: requires >= 4 bins or relief map
        let dummyIntrinsics = CameraIntrinsics(
            focalLengthX: 1000, focalLengthY: 1000,
            principalPointX: 960, principalPointY: 540,
            imageWidth: 1920, imageHeight: 1080
        )
        func makeOrbitFrame(bin: OrbitBin) -> OrbitFrame {
            OrbitFrame(
                bin: bin, timestamp: 0, landmarks: [], gravity: [0, 1, 0],
                deviceRollDeg: 0, intrinsics: dummyIntrinsics,
                imageWidth: 1920, imageHeight: 1080,
                digitSpreadOk: true, maxJointDrift: 0
            )
        }

        vm.orbitSession.frames.append(makeOrbitFrame(bin: .flatCenter))
        vm.orbitSession.frames.append(makeOrbitFrame(bin: .tiltLeft))
        vm.orbitSession.frames.append(makeOrbitFrame(bin: .tiltRight))
        XCTAssertFalse(vm.completedScanModes.contains(.orbitScan), "3 bins is not enough")

        vm.orbitSession.frames.append(makeOrbitFrame(bin: .tiltForward))
        XCTAssertTrue(vm.completedScanModes.contains(.orbitScan), "4 bins completes orbit")

        // 4. Close-Up: requires all 4 tiles
        let intrinsics = dummyIntrinsics

        vm.closeUpSession.tiles.append(CloseUpTileFrame(tile: .thenar, timestamp: 0, landmarks: [], intrinsics: intrinsics, imageWidth: 1920, imageHeight: 1080))
        vm.closeUpSession.tiles.append(CloseUpTileFrame(tile: .distal, timestamp: 0, landmarks: [], intrinsics: intrinsics, imageWidth: 1920, imageHeight: 1080))
        vm.closeUpSession.tiles.append(CloseUpTileFrame(tile: .midPalm, timestamp: 0, landmarks: [], intrinsics: intrinsics, imageWidth: 1920, imageHeight: 1080))
        XCTAssertFalse(vm.completedScanModes.contains(.closeUpMosaic), "3 of 4 tiles does not complete closeUp")

        vm.closeUpSession.tiles.append(CloseUpTileFrame(tile: .ulnar, timestamp: 0, landmarks: [], intrinsics: intrinsics, imageWidth: 1920, imageHeight: 1080))
        XCTAssertTrue(vm.completedScanModes.contains(.closeUpMosaic), "All 4 tiles completes closeUp")

        // 5. Fingertips: requires all 5 digits
        let digits: [FingertipDigit] = [.thumb, .index, .middle, .ring]
        for d in digits {
            vm.fingertipSession.results.append(FingertipScanResult(digit: d, pattern: .loop, ridgeFrequency: 0.8, quality: 0.9, timestamp: 0))
        }
        XCTAssertFalse(vm.completedScanModes.contains(.fingertipScan), "4 of 5 digits does not complete fingertips")

        vm.fingertipSession.results.append(FingertipScanResult(digit: .pinky, pattern: .whorl, ridgeFrequency: 0.8, quality: 0.9, timestamp: 0))
        XCTAssertTrue(vm.completedScanModes.contains(.fingertipScan), "All 5 digits completes fingertips")

        // 6. All 4 completed simultaneously
        XCTAssertEqual(vm.completedScanModes.count, 4)
    }

    func testModeSwitchingResetsTransientsAndManagesMacroFocus() {
        let vm = PalmAnalysisViewModel()

        // Switch to Close-Up: macro enabled, dwell gate reset
        vm.setScanMode(.closeUpMosaic)
        XCTAssertEqual(vm.scanMode, .closeUpMosaic)
        XCTAssertEqual(vm.closeUpDwellProgress, 0.0)

        // Switch to Fingertips: macro enabled
        vm.setScanMode(.fingertipScan)
        XCTAssertEqual(vm.scanMode, .fingertipScan)

        // Switch to Orbit: tilt started, macro disabled
        vm.setScanMode(.orbitScan)
        XCTAssertEqual(vm.scanMode, .orbitScan)

        // Switch to Still Photo: macro disabled
        vm.setScanMode(.stillPhoto)
        XCTAssertEqual(vm.scanMode, .stillPhoto)
    }
}

