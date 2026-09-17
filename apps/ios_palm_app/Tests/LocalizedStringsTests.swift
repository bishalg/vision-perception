import XCTest
@testable import PalmScanner

final class LocalizedStringsTests: XCTestCase {
    func testLanguagesExposeDistinctDisplayNames() {
        XCTAssertEqual(AppLanguage.allCases, [.english, .nepali, .hindi])
        XCTAssertEqual(AppLanguage.allCases.map(\.displayName), ["English", "नेपाली", "हिन्दी"])
        XCTAssertEqual(AppLanguage.allCases.map(\.id), ["en", "ne", "hi"])
        XCTAssertEqual(AppLanguage(rawValue: "ne"), .nepali)
        XCTAssertNil(AppLanguage(rawValue: "fr"))
    }

    func testSharedStringTableIsBundledAndCoversKnownKeysInAllLanguages() {
        let bundle = Bundle.main
        let url = bundle.url(forResource: "palm_strings", withExtension: "json")
        XCTAssertNotNil(url, "palm_strings.json must be bundled into the host app")
        guard let url, let data = try? Data(contentsOf: url),
              let table = try? JSONDecoder().decode([String: [String: String]].self, from: data) else {
            return XCTFail("palm_strings.json could not be decoded as [String: [String: String]]")
        }
        for key in ["line.heart", "line.head", "line.life", "line.fate", "sheet.title",
                    "result.detectedCreases", "button.retake"] {
            let entry = table[key]
            XCTAssertNotNil(entry, "missing shared key \(key)")
            if let entry {
                for lang in ["en", "ne", "hi"] {
                    let value = entry[lang]
                    XCTAssertNotNil(value, "\(key) missing \(lang)")
                    if let value {
                        XCTAssertFalse(value.isEmpty, "\(key)[\(lang)] is empty")
                    }
                }
                XCTAssertEqual(entry["en"], entry["en"]?.trimmingCharacters(in: .whitespacesAndNewlines))
            }
        }
    }

    func testLookupReturnsRealTranslationsForEverySupportedLanguage() {
        for key in ["safety.disclaimer", "safety.privacy", "sheet.emptyTitle", "sheet.emptyBody"] {
            for language in AppLanguage.allCases {
                XCTAssertNotEqual(LocalizedStrings.tr(key, lang: language), key)
                XCTAssertFalse(LocalizedStrings.tr(key, lang: language).isEmpty)
            }
        }
        let english = LocalizedStrings.tr("line.heart", lang: .english)
        XCTAssertNotEqual(english, "line.heart")
        XCTAssertNotEqual(LocalizedStrings.tr("line.heart", lang: .nepali), english, "Nepali entry should not be the English string")
        XCTAssertNotEqual(LocalizedStrings.tr("line.heart", lang: .hindi), english, "Hindi entry should not be the English string")
    }

    func testLookupFallsBackToTheKeyItselfForUnknownKeys() {
        XCTAssertEqual(LocalizedStrings.tr("__definitely_missing__", lang: .nepali), "__definitely_missing__")
        XCTAssertEqual(LocalizedStrings.tr("__definitely_missing__", lang: .english), "__definitely_missing__")
    }
}
