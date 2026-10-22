import XCTest
@testable import CurvAsdGazeKit

final class NormativeBaselineLibraryTests: XCTestCase {

    static var allVideoIds: [String] { StimulusCatalog.current.clipIds }

    private var baseline: NormativeBaselineSet {
        guard let set = NormativeBaselineLibrary.loadBundled() else {
            XCTFail("Bundled normative baseline JSON failed to decode")
            return NormativeBaselineSet(schemaVersion: "0", status: "", provenance: nil, ageBrackets: [], videos: [:])
        }
        return set
    }

    func testBundledBaselineDecodesWithProvisionalStatus() {
        let set = baseline
        XCTAssertEqual(set.schemaVersion, "1.0.0")
        XCTAssertEqual(set.status, "provisional-literature-seeded")
        XCTAssertNotNil(set.provenance)
        XCTAssertEqual(set.ageBrackets.count, 5)
    }

    func testAllTenStimulusVideosHaveProfiles() {
        let set = baseline
        let problems = NormativeBaselineLibrary.validate(set, requiredVideoIds: Self.allVideoIds)
        XCTAssertTrue(problems.isEmpty, "baseline coverage problems: \(problems)")
        XCTAssertEqual(set.videos.count, Self.allVideoIds.count)
    }

    func testBracketResolutionAndClamping() {
        let set = baseline
        XCTAssertEqual(set.bracketId(forAgeMonths: 20), "18-23")
        XCTAssertEqual(set.bracketId(forAgeMonths: 12), "12-17")
        XCTAssertEqual(set.bracketId(forAgeMonths: 60), "48-60")
        XCTAssertNil(set.bracketId(forAgeMonths: 11))
        XCTAssertNil(set.bracketId(forAgeMonths: 61))
        // Out-of-range ages clamp to the nearest bracket for scoring.
        XCTAssertEqual(PerVideoScoringEngine.resolvedBracketId(baseline: set, ageMonths: 11), "12-17")
        XCTAssertEqual(PerVideoScoringEngine.resolvedBracketId(baseline: set, ageMonths: 61), "48-60")
    }

    func testLiteratureAnchorsArePresentWithCitations() {
        let set = baseline

        let geo = set.video("stim_05_geopref_split_10s")
        let geoExpectation = geo?.segments[0].expectations.first { $0.biomarker == "social_dwell_ratio" }
        XCTAssertEqual(geoExpectation?.range(forBracketId: "24-35")?.expected, 0.70)
        XCTAssertEqual(geoExpectation?.range(forBracketId: "24-35")?.acceptableLow, 0.55)
        XCTAssertEqual(geoExpectation?.source, "literature")
        XCTAssertTrue(geoExpectation?.citation.contains("eLife 2019") ?? false)

        let geoRatio = geo?.segments[0].expectations.first { $0.biomarker == "geometric_dwell_ratio" }
        XCTAssertEqual(geoRatio?.range(forBracketId: "24-35")?.acceptableHigh, 0.60)
        XCTAssertTrue(geoRatio?.citation.contains("69%") ?? false)

        let peekaboo = set.video("stim_01_social_peekaboo_8s")
        let latency = peekaboo?.segments[0].expectations.first { $0.biomarker == "first_entry_latency_ms" }
        XCTAssertEqual(latency?.range(forBracketId: "24-35")?.expected, 420.0)
        XCTAssertEqual(latency?.range(forBracketId: "24-35")?.acceptableHigh, 600.0)
        // Younger bracket override deviates from the literature default.
        XCTAssertEqual(latency?.range(forBracketId: "12-17")?.expected, 500.0)
        XCTAssertEqual(latency?.source, "literature")
    }

    func testRangeFallsBackToDefaultForUnspecifiedBracket() {
        let set = baseline
        let latency = set.video("stim_01_social_peekaboo_8s")?
            .segments[0].expectations.first { $0.biomarker == "first_entry_latency_ms" }
        XCTAssertEqual(latency?.range(forBracketId: "36-47")?.expected, 420.0)
    }

    func testInterpolatedSourcesAreMarked() {
        // At least one expectation in the set is an explicit interpolation, and every
        // expectation carries a source tag from the allowed vocabulary.
        let set = baseline
        let sources = set.videos.values.flatMap { $0.segments.flatMap { $0.expectations.map { $0.source } } }
        XCTAssertTrue(sources.contains("interpolated"))
        XCTAssertTrue(sources.allSatisfy { $0 == "literature" || $0 == "interpolated" })
    }
}
