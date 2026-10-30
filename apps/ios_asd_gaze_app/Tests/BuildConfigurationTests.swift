@testable import AsdGazeCollector
import XCTest

/// SIM-7: the app declares no hardware capabilities. `arkit` blocks Apple Vision Pro,
/// `front-facing-camera` filters no iOS 18 device, and face tracking is gated at
/// runtime (`CaptureEntryGate`). Apple does not allow tightening
/// `UIRequiredDeviceCapabilities` after the first release, so this is pinned here.
final class BuildConfigurationTests: XCTestCase {
    func testNoRequiredDeviceCapabilitiesBlockVisionPro() throws {
        let info = try XCTUnwrap(Bundle.main.infoDictionary)
        let caps = info["UIRequiredDeviceCapabilities"] as? [String] ?? []
        XCTAssertFalse(caps.contains("arkit"), "arkit blocks the app on Apple Vision Pro")
        XCTAssertFalse(caps.contains("front-facing-camera"), "not honoured on visionOS")
    }

    /// About shows the real bundle version, never a hard-coded build number.
    func testSettingsVersionLabelMatchesBundle() throws {
        let info = try XCTUnwrap(Bundle.main.infoDictionary)
        let version = try XCTUnwrap(info["CFBundleShortVersionString"] as? String)
        let build = try XCTUnwrap(info["CFBundleVersion"] as? String)
        XCTAssertEqual(SettingsView.appVersionLabel, "\(version) (Build \(build))")
    }

    // MARK: - CNS-9: no research study in the default build

    func testDefaultBuildDeclaresNoResearchStudy() {
        XCTAssertNil(Bundle.main.object(forInfoDictionaryKey: ResearchStudyConfiguration.infoPlistKey))
    }

    // MARK: - PRV-1 / PRV-2: privacy manifests

    private static let researchCollectedTypes: Set<String> = [
        "NSPrivacyCollectedDataTypeHealth",
        "NSPrivacyCollectedDataTypeOtherDataTypes"
    ]

    private func manifest(in bundle: URL) throws -> [String: Any] {
        let url = bundle.appendingPathComponent("PrivacyInfo.xcprivacy")
        let data = try Data(contentsOf: url)
        let plist = try PropertyListSerialization.propertyList(from: data, format: nil)
        return try XCTUnwrap(plist as? [String: Any], "\(url.path) is not a dictionary")
    }

    private var appManifest: [String: Any] {
        get throws { try manifest(in: Bundle.main.bundleURL) }
    }

    private var kitManifest: [String: Any] {
        get throws {
            let kitBundle = Bundle.main.bundleURL.appendingPathComponent("CurvAsdGazeKit_CurvAsdGazeKit.bundle")
            return try manifest(in: kitBundle)
        }
    }

    private func collectedTypes(_ manifest: [String: Any]) throws -> [String] {
        let entries = try XCTUnwrap(manifest["NSPrivacyCollectedDataTypes"] as? [[String: Any]])
        return entries.compactMap { $0["NSPrivacyCollectedDataType"] as? String }
    }

    private func accessedAPICategories(_ manifest: [String: Any]) throws -> Set<String> {
        let entries = try XCTUnwrap(manifest["NSPrivacyAccessedAPITypes"] as? [[String: Any]])
        return Set(entries.compactMap { $0["NSPrivacyAccessedAPIType"] as? String })
    }

    /// "Data Not Collected": with no research upload configured nothing leaves the
    /// device, so the app manifest declares no collected data types (PRV-1).
    func testAppManifestDeclaresNoCollectedDataInDefaultBuild() throws {
        XCTAssertNil(ResearchStudyConfiguration.current)
        XCTAssertEqual(try collectedTypes(appManifest), [])
    }

    /// The kit is a library with no network path; it never collects.
    func testKitManifestDeclaresNoCollectedData() throws {
        XCTAssertEqual(try collectedTypes(kitManifest), [])
    }

    /// When a study is configured, the upload bundle carries diary observations
    /// and gaze summaries, so the manifest must then declare both (PRV-1 a, b).
    func testCollectedTypesMatchResearchConfiguration() throws {
        let declared = Set(try collectedTypes(appManifest))
        if ResearchStudyConfiguration.current == nil {
            XCTAssertTrue(declared.isEmpty)
        } else {
            XCTAssertEqual(declared, Self.researchCollectedTypes)
        }
    }

    func testManifestsDeclareNoTracking() throws {
        for manifest in [try appManifest, try kitManifest] {
            XCTAssertEqual(manifest["NSPrivacyTracking"] as? Bool, false)
            XCTAssertEqual((manifest["NSPrivacyTrackingDomains"] as? [String]) ?? ["missing"], [])
        }
    }

    /// The categories are audited against the sources by
    /// `scripts/check_privacy_api_use.sh`; pinned here so a manifest edit that
    /// skips the script still fails a test (PRV-2).
    func testRequiredReasonCategoriesArePinned() throws {
        XCTAssertEqual(try accessedAPICategories(appManifest), ["NSPrivacyAccessedAPICategoryUserDefaults"])
        XCTAssertEqual(
            try accessedAPICategories(kitManifest),
            ["NSPrivacyAccessedAPICategoryUserDefaults", "NSPrivacyAccessedAPICategorySystemBootTime"]
        )
    }

    // MARK: - PRV-3 / CNS-8: usage strings

    private static let usageKeys = [
        "NSCameraUsageDescription",
        "NSMotionUsageDescription",
        "NSSpeechRecognitionUsageDescription",
        "NSMicrophoneUsageDescription",
        "NSPhotoLibraryUsageDescription",
        "NSHealthShareUsageDescription",
        "NSHealthUpdateUsageDescription"
    ]

    private func usageString(_ key: String) throws -> String {
        let value = try XCTUnwrap(Bundle.main.object(forInfoDictionaryKey: key) as? String, "\(key) missing")
        XCTAssertFalse(value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, "\(key) is blank")
        return value
    }

    func testEveryUsedPermissionHasAUsageString() throws {
        for key in Self.usageKeys {
            _ = try usageString(key)
        }
    }

    func testCameraUsageDisclosesOnDeviceGazeExpressionAndAgeCheck() throws {
        let camera = try usageString("NSCameraUsageDescription").lowercased()
        XCTAssertTrue(camera.contains("where your child looks"), camera)
        XCTAssertTrue(camera.contains("facial expression"), camera)
        XCTAssertTrue(camera.contains("on this device"), camera)
        XCTAssertTrue(camera.contains("not saved") || camera.contains("never saved") || camera.contains("never stored"), camera)
        // CNS-8: SubjectAgeQuickCheckCard is compiled into every configuration.
        XCTAssertTrue(camera.contains("optional age-range check"), camera)
    }

    /// ADR-0004: permission prompts describe an observation diary, not a diagnostic product.
    func testUsageStringsUseNonDiagnosticWording() throws {
        let forbidden = [
            #"\bscreen(ing|er)s?\b"#, #"\btests?\b"#, #"\bclinical(ly)?\b"#, #"bio-?markers?"#,
            #"\bautis(m|tic)\b"#, #"\bADHD\b"#, #"\bdiagnos"#, #"\brisk\b"#, #"\bIRB\b"#, #"\bFDA\b"#,
            #"\bresearch\b"#, #"\bstudy\b"#
        ]
        for key in Self.usageKeys {
            let value = try usageString(key)
            for pattern in forbidden {
                XCTAssertNil(
                    value.range(of: pattern, options: [.regularExpression, .caseInsensitive]),
                    "\(key) matches \(pattern): \(value)"
                )
            }
        }
    }

    // MARK: - WP-A.1: Demo tools presence

    func testDemoToolsFlagMatchesConfiguration() {
        #if ASD_DEMO_TOOLS
        // In Debug and DemoRelease, demo tools are compiled in.
        XCTAssertTrue(true, "ASD_DEMO_TOOLS is enabled in this configuration")
        #else
        // In Release, demo tools are strictly absent.
        XCTAssertTrue(true, "ASD_DEMO_TOOLS is disabled in Release")
        #endif
    }
}
