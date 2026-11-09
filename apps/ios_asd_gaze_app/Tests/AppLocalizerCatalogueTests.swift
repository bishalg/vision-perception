@testable import AsdGazeCollector
import XCTest

/// WP-LN.1 (i18n-readiness I-2, I-3): the string catalogues replace the in-code dictionary.
@MainActor
final class AppLocalizerCatalogueTests: XCTestCase {
    private static let appDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()


    /// `key -> language -> (state, value)` read from an `.xcstrings` file in the source tree.
    private func catalogue(_ name: String) throws -> [String: [String: (state: String, value: String)]] {
        let url = Self.appDir.appendingPathComponent("Resources/\(name).xcstrings")
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        XCTAssertEqual(root["sourceLanguage"] as? String, "en")
        let strings = try XCTUnwrap(root["strings"] as? [String: Any])
        var out: [String: [String: (state: String, value: String)]] = [:]
        for (key, entry) in strings {
            let loc = ((entry as? [String: Any])?["localizations"] as? [String: Any]) ?? [:]
            var langs: [String: (state: String, value: String)] = [:]
            for (lang, v) in loc {
                let unit = (v as? [String: Any])?["stringUnit"] as? [String: String]
                langs[lang] = (unit?["state"] ?? "", unit?["value"] ?? "")
            }
            out[key] = langs
        }
        return out
    }

    func testEveryKeyHasEnglishHindiAndNepaliValues() throws {
        for table in ["Localizable", "InfoPlist"] {
            let cat = try catalogue(table)
            XCTAssertFalse(cat.isEmpty, table)
            for (key, langs) in cat {
                for lang in ["en", "hi", "ne"] {
                    let unit = langs[lang]
                    XCTAssertNotNil(unit, "\(table).\(key) lacks \(lang)")
                    XCTAssertFalse(
                        (unit?.value ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                        "\(table).\(key) \(lang) is empty"
                    )
                }
                XCTAssertEqual(langs["en"]?.state, "translated", "\(table).\(key) en")
                // I-3: machine drafts stay needs_review until native review (H-10).
                for lang in ["hi", "ne"] {
                    XCTAssertTrue(
                        ["needs_review", "translated"].contains(langs[lang]?.state ?? ""),
                        "\(table).\(key) \(lang) state \(langs[lang]?.state ?? "nil")"
                    )
                }
            }
        }
    }

    func testTrFallbackOrder() throws {
        let cat = try catalogue("Localizable")
        let hi = try XCTUnwrap(cat["dashboard_title"]?["hi"]?.value)
        let ne = try XCTUnwrap(cat["dashboard_title"]?["ne"]?.value)
        let en = try XCTUnwrap(cat["dashboard_title"]?["en"]?.value)
        XCTAssertEqual(Localizer.tr("dashboard_title", lang: .hindi), hi)
        XCTAssertEqual(Localizer.tr("dashboard_title", lang: .nepali), ne)
        XCTAssertEqual(Localizer.tr("dashboard_title", lang: .english), en)
        XCTAssertNotEqual(hi, en)
        // Unknown keys return the key in every language.
        for locale in AppLocale.allCases {
            XCTAssertEqual(Localizer.tr("no_such_key_ln1", lang: locale), "no_such_key_ln1")
        }
        // The default (no override) resolves like the current locale.
        XCTAssertEqual(Localizer.tr("dashboard_title"), Localizer.tr("dashboard_title", lang: AppLocale.current))
    }

    func testBuiltInfoPlistDeclaresLocalisations() throws {
        let info = try XCTUnwrap(Bundle.main.infoDictionary)
        let locs = try XCTUnwrap(info["CFBundleLocalizations"] as? [String])
        for lang in ["en", "hi", "ne"] {
            XCTAssertTrue(locs.contains(lang), "CFBundleLocalizations lacks \(lang): \(locs)")
        }
        XCTAssertEqual(info["CFBundleDevelopmentRegion"] as? String, "en")
    }

    func testUsageDescriptionEnglishMatchesProjectYAML() throws {
        let cat = try catalogue("InfoPlist")
        let project = try String(contentsOf: Self.appDir.appendingPathComponent("project.yml"), encoding: .utf8)
        var seen = 0
        for line in project.split(separator: "\n") {
            let t = line.trimmingCharacters(in: .whitespaces)
            guard t.hasPrefix("INFOPLIST_KEY_NS"), t.contains("UsageDescription:") else { continue }
            let parts = t.dropFirst("INFOPLIST_KEY_".count).components(separatedBy: ": ")
            let key = parts[0]
            let value = parts.dropFirst().joined(separator: ": ").trimmingCharacters(in: CharacterSet(charactersIn: "\""))
            XCTAssertEqual(cat[key]?["en"]?.value, value, "\(key) differs between project.yml and InfoPlist.xcstrings")
            seen += 1
        }
        XCTAssertEqual(seen, 7)
        XCTAssertEqual(cat["CFBundleDisplayName"]?["en"]?.value, "EarlyGaze")
    }

    func testCompiledBundlesHaveHindiAndNepali() {
        for lang in ["hi", "ne"] {
            XCTAssertNotNil(Bundle.main.path(forResource: lang, ofType: "lproj"), "\(lang).lproj missing from the app bundle")
        }
    }
}
