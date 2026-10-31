import XCTest
@testable import CurvAsdGazeKit

final class PerVideoMetricsTests: XCTestCase {

    // MARK: Helpers

    private func frame(
        stimulusId: String?,
        mediaTime: Double?,
        x: Double,
        y: Double,
        aoiNames: [String],
        smile: Double = 0.1,
        valid: Bool = true,
        verdict: GazeQualityGate.Verdict = .optimal,
        sessionTime: Double? = nil
    ) -> GazeFrame {
        let sample = GazeTelemetrySample(
            timestampMs: (sessionTime ?? mediaTime ?? 0) * 1000.0,
            videoTimeSec: sessionTime ?? mediaTime ?? 0,
            xNorm: x,
            yNorm: y,
            confidence: valid ? 0.95 : 0.0,
            isValid: valid,
            stimulusId: stimulusId,
            clipMediaTimeSec: mediaTime
        )
        let metrics = FacialAffectMetrics(
            mouthSmileLeft: smile,
            mouthSmileRight: smile,
            cheekSquintLeft: smile,
            cheekSquintRight: smile
        )
        return GazeFrame(
            gaze: sample,
            affect: metrics,
            activeAOINames: aoiNames,
            qualityVerdict: verdict,
            stimulusId: stimulusId,
            clipMediaTimeSec: mediaTime
        )
    }

    // MARK: Grouping

    func testGroupsFramesByStimulusIdWithUntaggedFallback() {
        let frames = [
            frame(stimulusId: "stim_05_geopref_split_10s", mediaTime: 0.1, x: 0.2, y: 0.4, aoiNames: ["Social_Actor"]),
            frame(stimulusId: "stim_05_geopref_split_10s", mediaTime: 0.2, x: 0.2, y: 0.4, aoiNames: ["Social_Actor"]),
            frame(stimulusId: "stim_01_social_peekaboo_8s", mediaTime: 0.1, x: 0.5, y: 0.35, aoiNames: ["Eye_Region"]),
            frame(stimulusId: nil, mediaTime: 0.1, x: 0.5, y: 0.5, aoiNames: [])
        ]
        let sets = PerVideoMetricsExtractor.extract(from: frames)
        XCTAssertEqual(sets.count, 3)
        XCTAssertEqual(sets.map { $0.stimulusId }.sorted(), [
            "session_untagged", "stim_01_social_peekaboo_8s", "stim_05_geopref_split_10s"
        ])
        XCTAssertEqual(sets.first { $0.stimulusId == "stim_05_geopref_split_10s" }?.clipFrameCount, 2)
    }

    // MARK: GeoPref dwell and preference

    func testGeoPrefDwellRatiosAndPreferenceScore() {
        var frames: [GazeFrame] = []
        for i in 0..<60 {
            let social = i < 42
            frames.append(frame(
                stimulusId: "stim_05_geopref_split_10s",
                mediaTime: Double(i) / 60.0,
                x: social ? 0.25 : 0.75,
                y: 0.4,
                aoiNames: [social ? "Social_Actor" : "Geometric_Pattern"]
            ))
        }
        let set = PerVideoMetricsExtractor.extract(from: frames)[0]
        XCTAssertEqual(set.metrics[PerVideoMetricKey.socialDwellRatio.key(targetAOI: nil)]!, 0.70, accuracy: 1e-9)
        XCTAssertEqual(set.metrics[PerVideoMetricKey.geometricDwellRatio.key(targetAOI: nil)]!, 0.30, accuracy: 1e-9)
        XCTAssertEqual(set.metrics[PerVideoMetricKey.socialPreferenceScore.key(targetAOI: nil)]!, 0.40, accuracy: 1e-9)
        XCTAssertEqual(set.validSampleRatio, 1.0, accuracy: 1e-9)
    }

    // MARK: Latency

    func testFirstEntryLatencyUsesEarliestHitAndMissingAOIOmitsKey() {
        var frames: [GazeFrame] = []
        for i in 0..<180 {
            let t = Double(i) / 60.0
            let onTarget = t >= 2.5
            frames.append(frame(
                stimulusId: "stim_02_gaze_cue_right_8s",
                mediaTime: t,
                x: onTarget ? 0.8 : 0.5,
                y: 0.3,
                aoiNames: onTarget ? ["Gaze_Target_Right"] : ["Actor_Face"]
            ))
        }
        let set = PerVideoMetricsExtractor.extract(from: frames)[0]
        XCTAssertEqual(
            set.metrics[PerVideoMetricKey.firstEntryLatencyMs.key(targetAOI: "Gaze_Target_Right")]!,
            2500.0,
            accuracy: 50.0
        )
        // An AOI the child never fixated emits no latency key; scoring imputes the worst case.
        XCTAssertNil(set.metrics[PerVideoMetricKey.firstEntryLatencyMs.key(targetAOI: "Gaze_Target_Left")])
    }

    // MARK: Quality gate inputs

    func testValidSampleRatioExcludesRejectedAndInvalidFrames() {
        var frames: [GazeFrame] = []
        for i in 0..<100 {
            let rejected = i >= 70
            frames.append(frame(
                stimulusId: "stim_05_geopref_split_10s",
                mediaTime: Double(i) / 60.0,
                x: 0.25,
                y: 0.4,
                aoiNames: ["Social_Actor"],
                valid: !rejected,
                verdict: rejected ? .rejected : .optimal
            ))
        }
        let set = PerVideoMetricsExtractor.extract(from: frames)[0]
        XCTAssertEqual(set.validSampleRatio, 0.70, accuracy: 1e-9)
        XCTAssertEqual(set.validFrameCount, 70)
    }

    // MARK: Fixations

    func testFixationStatsDetectsSplitBySaccade() {
        var frames: [GazeFrame] = []
        for i in 0..<120 {
            let t = Double(i) / 60.0
            let x = t < 1.0 ? 0.3 : 0.8 // saccade at t=1.0 splits the run
            frames.append(frame(
                stimulusId: "stim_05_geopref_split_10s",
                mediaTime: t,
                x: x,
                y: 0.4,
                aoiNames: []
            ))
        }
        let set = PerVideoMetricsExtractor.extract(from: frames)[0]
        XCTAssertEqual(set.metrics[PerVideoMetricKey.fixationCount.key(targetAOI: nil)]!, 2.0, accuracy: 1e-9)
        XCTAssertEqual(set.metrics[PerVideoMetricKey.meanFixationDurationMs.key(targetAOI: nil)]!, 1000.0, accuracy: 100.0)
    }

    // MARK: Affect

    func testSmileReactivitySubtractsEarlyBaseline() {
        var frames: [GazeFrame] = []
        for i in 0..<120 {
            let t = Double(i) / 60.0
            let smile = t < 1.0 ? 0.10 : 0.70
            frames.append(frame(
                stimulusId: "stim_04_surprise_giftbox_10s",
                mediaTime: t,
                x: 0.5,
                y: 0.4,
                aoiNames: [],
                smile: smile
            ))
        }
        let set = PerVideoMetricsExtractor.extract(from: frames)[0]
        XCTAssertEqual(set.metrics[PerVideoMetricKey.smileReactivityIndex.key(targetAOI: nil)]!, 0.60, accuracy: 0.02)
        // Mean over 1s at 0.10 and 1s at 0.70
        XCTAssertEqual(set.metrics[PerVideoMetricKey.socialSmileIntensityMean.key(targetAOI: nil)]!, 0.40, accuracy: 0.02)
    }

    func testSectionWindowMeasuresGazeAndSmileFromThatSliceOnly() {
        let frames = [
            frame(stimulusId: "clip", mediaTime: 0.2, x: 0.4, y: 0.4, aoiNames: ["Actor_Face"], smile: 0.1),
            frame(stimulusId: "clip", mediaTime: 1.0, x: 0.4, y: 0.4, aoiNames: ["Actor_Face"], smile: 0.1),
            frame(stimulusId: "clip", mediaTime: 4.0, x: 0.8, y: 0.2, aoiNames: ["Gaze_Target_Right"], smile: 0.8),
            frame(stimulusId: "clip", mediaTime: 5.0, x: 0.8, y: 0.2, aoiNames: ["Gaze_Target_Right"], smile: 0.8)
        ]
        // The section window is cut inside the core (WP-A.11 removed the Swift `extractClip`):
        // a segment starting at 3.5 s sees only the last two frames, with latency measured
        // from the segment start. The whole-clip set (which has Actor_Face) is the fallback
        // and must not be used while the window has frames.
        func expectation(_ biomarker: String, _ aoi: String?, _ direction: NormativeDirection) -> NormativeExpectation {
            NormativeExpectation(
                biomarker: biomarker,
                direction: direction,
                targetAOI: aoi,
                byAge: ["default": NormativeRange(expected: 0.5, acceptableLow: 0.25, acceptableHigh: 0.75)],
                citation: "test",
                source: "test",
                note: nil
            )
        }
        let video = NormativeVideo(
            title: "clip",
            category: "test",
            weight: 1,
            segments: [NormativeSegment(
                id: "late",
                description: "from 3.5 s",
                startSec: 3.5,
                endSec: 10,
                expectations: [
                    expectation("aoi_dwell_ratio", "Gaze_Target_Right", .higherBetter),
                    expectation("aoi_dwell_ratio", "Actor_Face", .higherBetter),
                    expectation("first_entry_latency_ms", "Gaze_Target_Right", .lowerBetter),
                    expectation("social_smile_intensity_mean", nil, .higherBetter)
                ]
            )]
        )
        let wholeClip = PerVideoMetricsExtractor.extract(from: frames)[0]
        XCTAssertNotNil(wholeClip.metrics[PerVideoMetricKey.aoiDwellRatio.key(targetAOI: "Actor_Face")])
        let report = PerVideoScoringEngine.scoreVideo(
            metricSet: wholeClip,
            video: video,
            baseline: NormativeBaselineLibrary.loadBundled()!,
            bracketId: "default",
            clipFrames: frames
        )
        func observed(_ key: String) -> Double? { report.rows.first { $0.metricKey == key }?.observed }
        let target = PerVideoMetricKey.aoiDwellRatio.key(targetAOI: "Gaze_Target_Right")
        let face = PerVideoMetricKey.aoiDwellRatio.key(targetAOI: "Actor_Face")
        XCTAssertEqual(observed(target)!, 1.0, accuracy: 0.001)
        XCTAssertNil(observed(face))
        let latency = PerVideoMetricKey.firstEntryLatencyMs.key(targetAOI: "Gaze_Target_Right")
        XCTAssertEqual(observed(latency)!, 500, accuracy: 1)
        XCTAssertEqual(report.rows.first { $0.metricKey == latency }?.imputed, false)
        XCTAssertEqual(observed(PerVideoMetricKey.socialSmileIntensityMean.key(targetAOI: nil))!, 0.8, accuracy: 0.001)
    }
}
