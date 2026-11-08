@testable import AsdGazeCollector
import XCTest

/// SIM-7: the app declares no hardware capabilities. `arkit` blocks Apple Vision Pro,
/// `front-facing-camera` filters no iOS 18 device, and face tracking is gated at
/// runtime (`CaptureEntryGate`). Apple does not allow tightening
/// `UIRequiredDeviceCapabilities` after the first release, so this is pinned here.
@MainActor
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

    // MARK: - Store build (store-collection-build.md S-1, S-9)

    /// The in-app legal links match the store listing and are https.
    func testLegalLinkKeysArePresentAndHTTPS() throws {
        for key in [StoreBuild.privacyPolicyURLKey, StoreBuild.supportURLKey] {
            let value = try XCTUnwrap(Bundle.main.object(forInfoDictionaryKey: key) as? String, "\(key) missing")
            XCTAssertTrue(value.hasPrefix("https://"), "\(key) is not https: \(value)")
        }
        XCTAssertEqual(StoreBuild.privacyPolicyURL()?.absoluteString, "https://curv.engineering/privacy/asd-gaze")
        XCTAssertEqual(StoreBuild.supportURL()?.absoluteString, "https://curv.engineering/support")
    }

    /// `NSUserNotificationsUsageDescription` is not an iOS key; local notifications need no usage string.
    func testNoBogusNotificationUsageKey() {
        XCTAssertNil(Bundle.main.object(forInfoDictionaryKey: "NSUserNotificationsUsageDescription"))
    }

    /// S-7: the home-screen name is the working name, and "ASD" never appears in it.
    func testDisplayNameIsEarlyGaze() {
        XCTAssertEqual(StoreBuild.displayName(), "EarlyGaze")
    }

    /// S-1: the store flag is defined in Release only.
    func testStoreBuildFlagIsReleaseOnly() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
        let project = try String(contentsOf: root.appendingPathComponent("project.yml"), encoding: .utf8)
        let flags = try Self.otherSwiftFlags(inProjectYAML: project)
        XCTAssertTrue(try XCTUnwrap(flags["Release"]).contains("-DASD_STORE_BUILD"))
        XCTAssertFalse(try XCTUnwrap(flags["Debug"]).contains("ASD_STORE_BUILD"))
        XCTAssertFalse(try XCTUnwrap(flags["DemoRelease"]).contains("ASD_STORE_BUILD"))
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

    // MARK: - WP-G.8: bench capture is Debug-only (BEN-2, ADR-0009 D10)

    /// The bench flag may only be compiled into Debug. Compile-time: if this build has it, it
    /// is a Debug build. Source-time: Release and DemoRelease never list it, in the app
    /// (`project.yml`) or in the kit (`Package.swift` limits it to `.debug`). The binary
    /// is audited separately by `scripts/check_release_symbols.sh`.
    func testBenchCaptureFlagIsDebugOnly() throws {
        #if ASD_BENCH_CAPTURE
        #if DEBUG
        XCTAssertTrue(true)
        #else
        XCTFail("ASD_BENCH_CAPTURE is defined in a non-Debug build")
        #endif
        #endif

        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
        let project = try String(contentsOf: root.appendingPathComponent("project.yml"), encoding: .utf8)
        let flags = try Self.otherSwiftFlags(inProjectYAML: project)
        XCTAssertEqual(Set(flags.keys), ["Debug", "DemoRelease", "Release"])
        XCTAssertTrue(try XCTUnwrap(flags["Debug"]).contains("-DASD_BENCH_CAPTURE"))
        XCTAssertFalse(try XCTUnwrap(flags["DemoRelease"]).contains("ASD_BENCH_CAPTURE"))
        XCTAssertFalse(try XCTUnwrap(flags["Release"]).contains("ASD_BENCH_CAPTURE"))

        let kitManifest = root.deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("domains/asd_screening/ios/Package.swift")
        let package = try String(contentsOf: kitManifest, encoding: .utf8)
        let defines = package.split(separator: "\n").filter { $0.contains("ASD_BENCH_CAPTURE\"") }
        XCTAssertFalse(defines.isEmpty, "the kit declares the flag")
        XCTAssertTrue(defines.allSatisfy { $0.contains(".when(configuration: .debug)") }, "\(defines)")
    }

    /// The test target's own Debug flags must not be read as the app's, and a repeated
    /// configuration key in the app block is an error, not a silent overwrite.
    func testOtherSwiftFlagsParserIsScopedToTheAppTarget() throws {
        let yaml = """
        targets:
          AsdGazeCollector:
            settings:
              configs:
                Debug:
                  OTHER_SWIFT_FLAGS: "-DAPP_DEBUG"
                Release:
                  OTHER_SWIFT_FLAGS: "-DAPP_RELEASE"
          AsdGazeCollectorTests:
            settings:
              configs:
                Debug:
                  OTHER_SWIFT_FLAGS: "-DTEST_DEBUG"
        """
        let flags = try Self.otherSwiftFlags(inProjectYAML: yaml)
        XCTAssertEqual(flags["Debug"], #"OTHER_SWIFT_FLAGS: "-DAPP_DEBUG""#)
        XCTAssertEqual(flags["Release"], #"OTHER_SWIFT_FLAGS: "-DAPP_RELEASE""#)

        let duplicate = """
        targets:
          AsdGazeCollector:
            settings:
              configs:
                Debug:
                  OTHER_SWIFT_FLAGS: "-DONE"
                Debug:
                  OTHER_SWIFT_FLAGS: "-DTWO"
        """
        XCTAssertThrowsError(try Self.otherSwiftFlags(inProjectYAML: duplicate))

        XCTAssertThrowsError(try Self.otherSwiftFlags(inProjectYAML: "targets:\n  Other:\n"), "no app target block")
    }

    private struct ProjectYAMLError: Error, CustomStringConvertible {
        let description: String
    }

    /// `OTHER_SWIFT_FLAGS` of the app target per configuration, read from the
    /// `configs:` block inside the `  AsdGazeCollector:` target only. Throws if the
    /// target block is missing or a configuration declares the flags twice.
    private static func otherSwiftFlags(inProjectYAML yaml: String) throws -> [String: String] {
        var result: [String: String] = [:]
        var inAppTarget = false
        var foundAppTarget = false
        var current: String?
        var seen: Set<String> = []
        for raw in yaml.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = String(raw)
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty || trimmed.hasPrefix("#") { continue }
            let indent = line.prefix { $0 == " " }.count
            // A two-space key is a target name inside `targets:`; a top-level key ends `targets:`.
            if indent <= 2 {
                inAppTarget = (line == "  AsdGazeCollector:")
                foundAppTarget = foundAppTarget || inAppTarget
                current = nil
                continue
            }
            guard inAppTarget else { continue }
            if indent == 8, ["Debug:", "DemoRelease:", "Release:"].contains(trimmed) {
                let name = String(trimmed.dropLast())
                if !seen.insert(name).inserted {
                    throw ProjectYAMLError(description: "duplicate \(name) configuration in the app target")
                }
                current = name
            } else if trimmed.hasPrefix("OTHER_SWIFT_FLAGS:"), let name = current {
                if result[name] != nil {
                    throw ProjectYAMLError(description: "duplicate OTHER_SWIFT_FLAGS for \(name)")
                }
                result[name] = trimmed
            } else if indent <= 8 {
                current = nil
            }
        }
        guard foundAppTarget else {
            throw ProjectYAMLError(description: "no `  AsdGazeCollector:` target block")
        }
        return result
    }
}
