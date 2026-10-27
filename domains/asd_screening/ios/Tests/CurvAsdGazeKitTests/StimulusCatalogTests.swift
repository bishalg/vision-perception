import XCTest
@testable import CurvAsdGazeKit

final class StimulusCatalogTests: XCTestCase {
    func testBundledCatalogCoversTheCurrentBattery() {
        let catalog = StimulusCatalog.current
        XCTAssertEqual(catalog.schemaVersion, "1.0.0")
        XCTAssertEqual(catalog.clips.count, 10)
        XCTAssertEqual(StimulusAOITracker.makeStandard10ClipBattery().count, 10)

        for clip in catalog.clips {
            XCTAssertEqual(clip.durationSec, 8.0, "Every shipped clip has a measured duration of exactly 8.0s")
        }

        let express = catalog.playlist(forTargetDuration: 30)
        XCTAssertEqual(express.map(\.id), [
            "stim_05_geopref_split_10s",
            "stim_01_social_peekaboo_8s",
            "stim_02_gaze_cue_right_8s",
            "stim_07_cartoon_slapstick_penguin_8s"
        ])
        XCTAssertEqual(express.reduce(0) { $0 + $1.durationSec }, 32.0, accuracy: 0.01)

        let full = catalog.playlist(forTargetDuration: 60)
        XCTAssertEqual(full.count, 8)
        XCTAssertEqual(full.reduce(0) { $0 + $1.durationSec }, 64.0, accuracy: 0.01)
        XCTAssertEqual(full.first?.id, "stim_05_geopref_split_10s")
        XCTAssertFalse(full.contains { $0.id == "stim_06_dynamic_gears_marbles_8s" })
    }

    func testAgeRangeDropsClipsOutsideTheChildsBracket() throws {
        let json = """
        {
          "schemaVersion": "1",
          "languages": ["en"],
          "fallbackLanguage": "en",
          "agePacks": [
            { "id": "age_12_24", "minMonths": 0, "maxMonths": 23 },
            { "id": "age_36_60", "minMonths": 36, "maxMonths": 60 }
          ],
          "clips": [
            {
              "id": "shared_geopref",
              "title": "Shared",
              "category": "GeoPref",
              "durationSec": 8,
              "minMonths": 12,
              "maxMonths": 60,
              "protocols": [{ "id": "express_30s", "order": 0 }],
              "areas": [
                { "name": "Social_Actor", "startSec": 0, "endSec": 8, "xMin": 0, "yMin": 0, "xMax": 0.5, "yMax": 1, "roles": ["social"] }
              ]
            },
            {
              "id": "preschool_humor",
              "title": "Preschool humor",
              "category": "Dynamic Affect",
              "durationSec": 7,
              "minMonths": 36,
              "maxMonths": 60,
              "protocols": [{ "id": "express_30s", "order": 1 }],
              "areas": [
                { "name": "Preschool_Face", "startSec": 0, "endSec": 7, "xMin": 0.2, "yMin": 0.2, "xMax": 0.8, "yMax": 0.8, "roles": ["social", "orienting"] }
              ]
            }
          ]
        }
        """.data(using: .utf8)!
        let catalog = try XCTUnwrap(StimulusCatalog.decode(json))

        let toddler = catalog.playlist(forTargetDuration: 30, ageMonths: 20).map(\.id)
        XCTAssertEqual(toddler, ["shared_geopref"])

        let preschool = catalog.playlist(forTargetDuration: 30, ageMonths: 40).map(\.id)
        XCTAssertEqual(preschool, ["shared_geopref", "preschool_humor"])
        XCTAssertTrue(catalog.hasRole("Preschool_Face", "orienting"))
        XCTAssertEqual(catalog.resourcePackTag(ageMonths: 40, localeCode: "hi"), "pack_age_36_60_en")
    }

    func testPackTagsFollowCatalogLanguages() {
        let catalog = StimulusCatalog.current
        XCTAssertEqual(catalog.resourcePackTag(ageMonths: 14, localeCode: "en"), "pack_age_12_24_en")
        XCTAssertEqual(catalog.resourcePackTag(ageMonths: 28, localeCode: "en"), "pack_age_24_36_en")
        XCTAssertEqual(catalog.resourcePackTag(ageMonths: 54, localeCode: "zh"), "pack_age_36_60_zh")
        XCTAssertEqual(catalog.resourcePackTag(ageMonths: 20, localeCode: "xx"), "pack_age_12_24_en")
    }
}
