import AsdCore
import Foundation
import XCTest
@testable import CurvAsdGazeKit

/// The kit's boundary to the Rust core (FFI-3): frame mapping, mirrored constants, and the
/// conservative results when the core refuses a document.
final class AsdCoreBridgeTests: XCTestCase {

    func testCoreIsLinked() {
        XCTAssertFalse(AsdCoreBridge.coreVersion.isEmpty)
        XCTAssertEqual(AsdCoreBridge.coreVersion, coreVersion())
    }

    /// CON-5: the Swift constants the app still shows or passes equal the core's bundled
    /// default `ScoringProfile`.
    func testMirroredConstantsEqualTheDefaultScoringProfile() throws {
        let profile = try AsdCoreBridge.defaultProfile()
        XCTAssertEqual(profile.flag.greenFloor, AttentionFlagAlgorithm.greenFloor)
        XCTAssertEqual(profile.flag.yellowFloor, AttentionFlagAlgorithm.yellowFloor)
        XCTAssertEqual(profile.flag.lowConfidenceShare, AttentionFlagAlgorithm.lowConfidenceShare)
        XCTAssertEqual(profile.flag.atypicalShareForRed, AttentionFlagAlgorithm.atypicalShareForRed)
        XCTAssertEqual(profile.lowConfidenceValidRatio, PerVideoScoringEngine.lowConfidenceValidRatio)
        let gate = GazeQualityGate()
        XCTAssertEqual(profile.qualityGate, CoreQualityGate(
            minDistanceCm: gate.minDistanceCm,
            maxDistanceCm: gate.maxDistanceCm,
            maxYawRad: gate.maxYawRad,
            maxPitchRad: gate.maxPitchRad,
            minConfidence: gate.minConfidence,
            blinkThreshold: gate.blinkThreshold
        ))
    }

    /// The derived affect values are computed properties in Swift, so they must be sent
    /// explicitly; without them the core would read zero smile and expressivity.
    func testCoreFrameCarriesWhatScoringReads() throws {
        let affect = FacialAffectMetrics(
            mouthSmileLeft: 0.8, mouthSmileRight: 0.6, cheekSquintLeft: 0.4, cheekSquintRight: 0.2,
            browInnerUp: 0.3, eyeWideLeft: 0.1, jawOpen: 0.5, browDownLeft: 0.05
        )
        let frame = GazeFrame(
            gaze: GazeTelemetrySample(
                timestampMs: 1500, videoTimeSec: 1.5, xNorm: 0.25, yNorm: 0.75, confidence: 0.9,
                isValid: false, stimulusId: "gaze-level-id", clipMediaTimeSec: 0.75
            ),
            affect: affect,
            activeAOINames: ["Eye_Region", "Face_Center"],
            qualityVerdict: .degraded,
            stimulusId: "stim_01_social_peekaboo_8s",
            clipMediaTimeSec: 0.75
        )
        let json = try XCTUnwrap(try Golden.json(CoreFrame(frame)) as? [String: Any])
        Golden.assertClose(json, [
            "gaze": ["valid": false, "x_norm": 0.25, "y_norm": 0.75, "video_time_sec": 1.5, "clip_media_time_sec": 0.75],
            "affect": [
                "socialSmileIntensity": affect.socialSmileIntensity,
                "expressivityIndex": affect.expressivityIndex,
                "surpriseScore": affect.surpriseScore
            ],
            "activeAOINames": ["Eye_Region", "Face_Center"],
            "qualityVerdict": "degraded",
            "stimulusId": "stim_01_social_peekaboo_8s"
        ] as [String: Any], "frame")

        let untimed = GazeFrame(gaze: GazeTelemetrySample(timestampMs: 0, xNorm: 0, yNorm: 0, confidence: 1))
        let untimedJSON = try XCTUnwrap(try Golden.json(CoreFrame(untimed)) as? [String: Any])
        XCTAssertNil(untimedJSON["stimulusId"], "untagged frames are absent keys, not null")
        XCTAssertNil((untimedJSON["gaze"] as? [String: Any])?["clip_media_time_sec"])
        XCTAssertEqual(CoreFrame(untimed).attributed(to: "clip").stimulusId, "clip")
    }

    // MARK: - Refused documents (non-finite readings cannot be encoded as JSON)

    func testUnreadableSampleIsRejectedNotPassed() {
        let sample = GazeTelemetrySample(timestampMs: 0, xNorm: 0.5, yNorm: 0.5, confidence: 0.9, distanceToScreenCm: .nan)
        let evaluation = GazeQualityGate().evaluateSample(sample)
        XCTAssertFalse(evaluation.isValid)
        XCTAssertEqual(evaluation.verdict, .rejected)
        XCTAssertEqual(evaluation.failureReasons, ["Tracking data unreadable"])
    }

    func testUnreadableFramesYieldNoMetricsAndUnmeasuredAggregates() {
        let frame = GazeFrame(
            gaze: GazeTelemetrySample(timestampMs: 0, xNorm: .infinity, yNorm: 0.5, confidence: 1),
            activeAOINames: ["Eye_Region"]
        )
        XCTAssertEqual(PerVideoMetricsExtractor.extract(from: [frame]), [])
        let aggregates = SessionBiomarkerAggregator.aggregate(validFrames: [frame], durationSec: 1)
        XCTAssertNil(aggregates.eyeDwellRatio)
        XCTAssertNil(aggregates.firstOrientLatencyMs)
        XCTAssertEqual(aggregates.flatAffectScore, 1)
    }

    func testUnreadableScoreIsNeverGreenOrRed() {
        let baseline = NormativeBaselineLibrary.loadBundled()!
        let set = PerVideoMetricSet(
            stimulusId: "stim_05_geopref_split_10s", clipFrameCount: 1, validFrameCount: 1, validSampleRatio: 1,
            observedDurationSec: 1, aoiDwellRatios: [:], metrics: ["social_dwell_ratio": .nan]
        )
        let score = PerVideoScoringEngine.score(metricSets: [set], baseline: baseline, ageMonths: 30)
        XCTAssertEqual(score.videos, [])
        XCTAssertEqual(score.compositeScore, 0)
        XCTAssertEqual(AttentionFlagAlgorithm.assess(score).flag, .yellow)
    }
}
