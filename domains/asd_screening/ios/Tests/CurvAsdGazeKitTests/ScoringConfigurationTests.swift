import Foundation
import XCTest
@testable import CurvAsdGazeKit

/// WP-A.5: delivered config bundles are trusted only through the Rust core
/// (`config_accept_bundle`), and an accepted bundle's `ScoringProfile` and baselines reach
/// scoring through the Rust path. Fixtures: `apps/ios_asd_gaze_app/Tests/Fixtures/config-bundle/`
/// (signed by `asd-sync/tests/config_bundle.rs` with fixed test seeds; the delivered profile
/// has `greenFloor` 0.80 and `lowConfidenceValidRatio` 0.70).
final class ScoringConfigurationTests: XCTestCase {
    static let fixtures = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../../../../../apps/ios_asd_gaze_app/Tests/Fixtures/config-bundle")
        .standardized

    /// 2026-11-02T00:00:00Z, a day after the valid fixture was signed.
    static let now = Date(timeIntervalSince1970: 1_793_577_600)

    static func fixture(_ name: String) throws -> String {
        try String(contentsOf: fixtures.appendingPathComponent(name), encoding: .utf8)
    }

    static func key(_ role: String) throws -> TrustedBundleKey {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("test-keys.json"))
        let keys = try JSONDecoder().decode([String: TrustedBundleKey].self, from: data)
        return try XCTUnwrap(keys[role])
    }

    private func accept(
        _ name: String,
        keys: [TrustedBundleKey]? = nil,
        newest: String? = nil,
        maxAge: TimeInterval? = nil
    ) throws -> Result<ScoringConfiguration, BundleRejection> {
        ScoringConfiguration.accept(
            bundleJSON: try Self.fixture(name),
            trustedKeys: try keys ?? [Self.key("trusted")],
            newestAcceptedVersion: newest,
            now: Self.now,
            maxAge: maxAge
        )
    }

    func testBundledDefaultIsTheCompiledInConfiguration() {
        let bundled = ScoringConfiguration.bundled
        XCTAssertEqual(bundled.source, .bundled)
        XCTAssertNil(bundled.bundleVersion)
        XCTAssertNotNil(bundled.baselines)
        XCTAssertFalse(bundled.catalog.clips.isEmpty)
        XCTAssertEqual(bundled.qualityGate.minDistanceCm, GazeQualityGate().minDistanceCm)
    }

    func testValidBundleIsAcceptedAndDecoded() throws {
        let config = try accept("bundle-valid.json").get()
        XCTAssertEqual(config.bundleVersion, "2026.11.01-2")
        XCTAssertEqual(config.featureFlags["norm_collection"], false)
        XCTAssertNotNil(config.baselines)
        XCTAssertEqual(config.catalog.clips.count, ScoringConfiguration.bundled.catalog.clips.count)
    }

    func testTamperedBundleIsRefused() throws {
        let tampered = try Self.fixture("bundle-valid.json")
            .replacingOccurrences(of: "\"greenFloor\": 0.8", with: "\"greenFloor\": 0.1")
        XCTAssertNotEqual(tampered, try Self.fixture("bundle-valid.json"))
        let result = ScoringConfiguration.accept(
            bundleJSON: tampered, trustedKeys: [try Self.key("trusted")], newestAcceptedVersion: nil, now: Self.now
        )
        XCTAssertEqual(result.failure, .badSignature)
    }

    func testWrongAndUnknownKeysAreRefused() throws {
        XCTAssertEqual(try accept("bundle-wrong-key.json").failure, .badSignature)
        XCTAssertEqual(try accept("bundle-unknown-key.json").failure, .unknownKey)
        // Trusting only the other key: the valid bundle names a key the ring does not hold.
        XCTAssertEqual(try accept("bundle-valid.json", keys: [Self.key("untrusted")]).failure, .unknownKey)
        XCTAssertEqual(try accept("bundle-valid.json", keys: []).failure, .noTrustedKeys)
    }

    func testRotationTrustsBothKeysOfTheRing() throws {
        let ring = [try Self.key("untrusted"), try Self.key("trusted")]
        XCTAssertNotNil(try? accept("bundle-valid.json", keys: ring).get())
        XCTAssertNotNil(try? accept("bundle-unknown-key.json", keys: ring).get())
    }

    func testRollbackAndExpiryAreRefused() throws {
        XCTAssertEqual(try accept("bundle-older.json", newest: "2026.11.01-2").failure, .rollback)
        XCTAssertNotNil(try? accept("bundle-valid.json", newest: "2026.11.01-2").get(), "same version re-verifies")
        XCTAssertEqual(try accept("bundle-older.json", maxAge: 2 * 86_400).failure, .expired)
        XCTAssertNotNil(try? accept("bundle-valid.json", maxAge: 2 * 86_400).get())
    }

    /// The delivered profile reaches the Rust scoring and flag paths: a clip at 0.65 valid
    /// samples is low-confidence only under the delivered 0.70 ratio.
    func testDeliveredProfileDrivesScoring() throws {
        let delivered = try accept("bundle-valid.json").get()
        let set = PerVideoMetricSet(
            stimulusId: "stim_05_geopref_split_10s", clipFrameCount: 100, validFrameCount: 65,
            validSampleRatio: 0.65, observedDurationSec: 10, aoiDwellRatios: [:], metrics: [:]
        )
        let bundledScore = try XCTUnwrap(PerVideoScoringEngine.score(
            metricSets: [set], configuration: .bundled, ageMonths: 30
        ))
        let deliveredScore = try XCTUnwrap(PerVideoScoringEngine.score(
            metricSets: [set], configuration: delivered, ageMonths: 30
        ))
        XCTAssertEqual(bundledScore.videos.first?.lowConfidence, false)
        XCTAssertEqual(deliveredScore.videos.first?.lowConfidence, true)

        // A composite between the bundled 0.65 and the delivered 0.80 green floor.
        let strong = PerVideoSessionScore(
            ageBracketId: "default", ageMonths: 30, compositeScore: 0.72,
            videos: [PerVideoScoreReport(
                stimulusId: "a", title: "A", category: "GeoPref", rows: [], subscore: 0.72,
                validSampleRatio: 0.95, lowConfidence: false, hasBaseline: true, weight: 1
            )],
            baselineStatus: "provisional", baselineSchemaVersion: "1", scoredAtUtc: ""
        )
        let bundledFlag = AttentionFlagAlgorithm.assess(strong).flag
        let deliveredFlag = AttentionFlagAlgorithm.assess(strong, scoringProfileJSON: delivered.scoringProfileJSON).flag
        XCTAssertNotEqual(bundledFlag, deliveredFlag, "the delivered greenFloor changes the flag")
        XCTAssertNotEqual(deliveredFlag, .green)
    }

    @MainActor
    func testControllerKeepsTheConfigurationItWasCreatedWith() throws {
        let delivered = try accept("bundle-valid.json").get()
        ScoringConfiguration.install(delivered)
        defer { ScoringConfiguration.install(.bundled) }
        let controller = GazeSessionController()
        ScoringConfiguration.install(.bundled)
        XCTAssertNil(ScoringConfiguration.active.bundleVersion)
        XCTAssertEqual(controller.scoringConfiguration.bundleVersion, "2026.11.01-2")
        XCTAssertEqual(controller.scoringConfiguration.scoringProfileJSON, delivered.scoringProfileJSON)
        XCTAssertEqual(controller.scoringConfiguration.catalog, delivered.catalog)
    }

    func testInstallSwitchesTheActiveConfigurationAndCatalog() throws {
        let delivered = try accept("bundle-valid.json").get()
        defer { ScoringConfiguration.install(.bundled) }
        ScoringConfiguration.install(delivered)
        XCTAssertEqual(ScoringConfiguration.active.bundleVersion, "2026.11.01-2")
        XCTAssertEqual(StimulusCatalog.current, delivered.catalog)
        ScoringConfiguration.install(.bundled)
        XCTAssertNil(ScoringConfiguration.active.bundleVersion)
    }
}

extension Result {
    var failure: Failure? {
        if case .failure(let error) = self { return error }
        return nil
    }
}
