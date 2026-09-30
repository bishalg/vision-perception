import XCTest
@testable import CurvAsdGazeKit

final class PerVideoScoringEngineTests: XCTestCase {

    private var baseline: NormativeBaselineSet {
        guard let set = NormativeBaselineLibrary.loadBundled() else {
            XCTFail("Bundled normative baseline JSON failed to decode")
            return NormativeBaselineSet(schemaVersion: "0", status: "", provenance: nil, ageBrackets: [], videos: [:])
        }
        return set
    }

    private func metricSet(
        _ stimulusId: String,
        metrics: [String: Double],
        validRatio: Double = 0.95,
        durationSec: Double = 8.0
    ) -> PerVideoMetricSet {
        PerVideoMetricSet(
            stimulusId: stimulusId,
            clipFrameCount: Int(durationSec * 60),
            validFrameCount: Int(durationSec * 60 * validRatio),
            validSampleRatio: validRatio,
            observedDurationSec: durationSec,
            aoiDwellRatios: [:],
            metrics: metrics
        )
    }

    // MARK: Status bands

    func testOnTargetGeoPrefProducesFullSubscore() {
        let set = metricSet("stim_05_geopref_split_10s", metrics: [
            "social_dwell_ratio": 0.80,
            "geometric_dwell_ratio": 0.20,
            "social_preference_score": 0.60
        ])
        let report = PerVideoScoringEngine.scoreVideo(
            metricSet: set,
            video: baseline.video("stim_05_geopref_split_10s")!,
            baseline: baseline,
            bracketId: "24-35"
        )
        XCTAssertTrue(report.hasBaseline)
        XCTAssertTrue(report.rows.allSatisfy { $0.status == .onTarget })
        XCTAssertEqual(report.subscore, 1.0, accuracy: 1e-9)
    }

    func testMonitorBandForSocialDwell() {
        let set = metricSet("stim_05_geopref_split_10s", metrics: ["social_dwell_ratio": 0.60])
        let report = PerVideoScoringEngine.scoreVideo(
            metricSet: set,
            video: baseline.video("stim_05_geopref_split_10s")!,
            baseline: baseline,
            bracketId: "24-35"
        )
        let row = report.rows.first { $0.biomarker == "social_dwell_ratio" }!
        XCTAssertEqual(row.status, .monitor)
        XCTAssertEqual(row.score, 0.6667, accuracy: 0.01)
    }

    func testAtypicalBeyondTolerance() {
        let set = metricSet("stim_05_geopref_split_10s", metrics: ["social_dwell_ratio": 0.30])
        let report = PerVideoScoringEngine.scoreVideo(
            metricSet: set,
            video: baseline.video("stim_05_geopref_split_10s")!,
            baseline: baseline,
            bracketId: "24-35"
        )
        XCTAssertEqual(report.rows.first { $0.biomarker == "social_dwell_ratio" }!.status, .atypical)
    }

    func testLatencyDirectionAwareScoring() {
        func latencyReport(_ ms: Double) -> PerVideoScoreRow {
            let set = metricSet("stim_01_social_peekaboo_8s", metrics: ["first_entry_latency_ms::Eye_Region": ms])
            let report = PerVideoScoringEngine.scoreVideo(
                metricSet: set,
                video: baseline.video("stim_01_social_peekaboo_8s")!,
                baseline: baseline,
                bracketId: "24-35"
            )
            return report.rows.first { $0.biomarker == "first_entry_latency_ms" }!
        }
        XCTAssertEqual(latencyReport(300).status, .onTarget)
        XCTAssertEqual(latencyReport(500).status, .monitor)
        XCTAssertEqual(latencyReport(900).status, .atypical)
        XCTAssertEqual(latencyReport(1200).score, 0.0, accuracy: 1e-9)
    }

    func testAgeBracketChangesExpectedLatency() {
        let set = metricSet("stim_01_social_peekaboo_8s", metrics: ["first_entry_latency_ms::Eye_Region": 500])
        let video = baseline.video("stim_01_social_peekaboo_8s")!
        // 500ms is monitor for 24-35 (expected 420) but onTarget for 12-17 (expected 500).
        let older = PerVideoScoringEngine.scoreVideo(metricSet: set, video: video, baseline: baseline, bracketId: "24-35")
        let younger = PerVideoScoringEngine.scoreVideo(metricSet: set, video: video, baseline: baseline, bracketId: "12-17")
        XCTAssertEqual(older.rows.first { $0.biomarker == "first_entry_latency_ms" }!.status, .monitor)
        XCTAssertEqual(younger.rows.first { $0.biomarker == "first_entry_latency_ms" }!.status, .onTarget)
    }

    func testMissingLatencyImputedAtClipDurationWorstCase() {
        let set = metricSet("stim_02_gaze_cue_right_8s", metrics: ["aoi_dwell_ratio::Gaze_Target_Right": 0.10])
        let report = PerVideoScoringEngine.scoreVideo(
            metricSet: set,
            video: baseline.video("stim_02_gaze_cue_right_8s")!,
            baseline: baseline,
            bracketId: "24-35"
        )
        let latencyRow = report.rows.first { $0.biomarker == "first_entry_latency_ms" }!
        XCTAssertTrue(latencyRow.imputed)
        XCTAssertEqual(latencyRow.observed, 8000.0, accuracy: 500.0)
        XCTAssertEqual(latencyRow.status, .atypical)
    }

    // MARK: Composite

    func testCompositeIsWeightedMeanOverClipsWithBaseline() {
        let geo = metricSet("stim_05_geopref_split_10s", metrics: [
            "social_dwell_ratio": 0.80,
            "geometric_dwell_ratio": 0.20,
            "social_preference_score": 0.60
        ])
        let peekaboo = metricSet("stim_01_social_peekaboo_8s", metrics: [
            "eye_dwell_ratio": 0.35,
            "first_entry_latency_ms::Eye_Region": 380.0,
            "smile_reactivity_index": 0.30
        ])
        let untagged = metricSet(PerVideoMetricsExtractor.untaggedClipId, metrics: ["social_dwell_ratio": 0.9])

        let session = PerVideoScoringEngine.score(
            metricSets: [geo, peekaboo, untagged],
            baseline: baseline,
            ageMonths: 24
        )

        XCTAssertEqual(session.ageBracketId, "24-35")
        XCTAssertEqual(session.videos.count, 3)
        XCTAssertFalse(session.videos.first { $0.stimulusId == PerVideoMetricsExtractor.untaggedClipId }!.hasBaseline)

        let geoSub = 1.0
        let peekabooSub = session.videos.first { $0.stimulusId == "stim_01_social_peekaboo_8s" }!.subscore
        let expectedComposite = (0.30 * geoSub + 0.20 * peekabooSub) / 0.50
        XCTAssertEqual(session.compositeScore, expectedComposite, accuracy: 1e-9)
    }

    func testLowConfidenceQualityGate() {
        let set = metricSet("stim_05_geopref_split_10s", metrics: ["social_dwell_ratio": 0.8], validRatio: 0.45)
        let report = PerVideoScoringEngine.scoreVideo(
            metricSet: set,
            video: baseline.video("stim_05_geopref_split_10s")!,
            baseline: baseline,
            bracketId: "24-35"
        )
        XCTAssertTrue(report.lowConfidence)
    }

    func testBandDirectionEyeDwell() {
        func eyeRow(_ value: Double) -> PerVideoScoreRow {
            let set = metricSet("stim_01_social_peekaboo_8s", metrics: ["eye_dwell_ratio": value])
            let report = PerVideoScoringEngine.scoreVideo(
                metricSet: set,
                video: baseline.video("stim_01_social_peekaboo_8s")!,
                baseline: baseline,
                bracketId: "24-35"
            )
            return report.rows.first { $0.biomarker == "eye_dwell_ratio" }!
        }
        XCTAssertEqual(eyeRow(0.35).status, .onTarget)
        XCTAssertEqual(eyeRow(0.17).status, .monitor)
        XCTAssertEqual(eyeRow(0.05).status, .atypical)
    }

    func testSessionScoreIsCodable() throws {
        let set = metricSet("stim_05_geopref_split_10s", metrics: ["social_dwell_ratio": 0.8])
        let session = PerVideoScoringEngine.score(metricSets: [set], baseline: baseline, ageMonths: 24)
        let data = try JSONEncoder().encode(session)
        let decoded = try JSONDecoder().decode(PerVideoSessionScore.self, from: data)
        XCTAssertEqual(decoded, session)
    }
}
