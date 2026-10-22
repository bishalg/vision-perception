import XCTest
@testable import CurvAsdGazeKit

final class SessionBiomarkerAggregatorTests: XCTestCase {
    private func frame(
        timeSec: Double,
        names: [String],
        smile: Double = 0
    ) -> GazeFrame {
        let sample = GazeTelemetrySample(
            timestampMs: timeSec * 1000.0,
            videoTimeSec: timeSec,
            xNorm: 0.4,
            yNorm: 0.4,
            confidence: 0.95
        )
        return GazeFrame(
            gaze: sample,
            affect: FacialAffectMetrics(
                mouthSmileLeft: smile,
                mouthSmileRight: smile,
                cheekSquintLeft: smile,
                cheekSquintRight: smile
            ),
            activeAOINames: names,
            qualityVerdict: .optimal
        )
    }

    func testNestedFaceAndEyesCountOnceTowardSocialDwell() {
        let frames = [
            frame(timeSec: 0.0, names: ["Face_Center", "Eye_Region"]),
            frame(timeSec: 1.0 / 60.0, names: ["Geometric_Pattern"])
        ]
        let result = SessionBiomarkerAggregator.aggregate(validFrames: frames, durationSec: 2.0 / 60.0)

        XCTAssertEqual(result.socialDwellSec, 1.0 / 60.0, accuracy: 1e-9)
        XCTAssertEqual(result.geometricDwellSec, 1.0 / 60.0, accuracy: 1e-9)
        XCTAssertEqual(result.socialPreferenceScore, 0.0, accuracy: 1e-9)
        XCTAssertEqual(result.eyeDwellRatio, 1.0, accuracy: 1e-9)
        XCTAssertEqual(result.firstOrientLatencyMs, 0.0, accuracy: 1e-6)
    }

    func testActorEyesAndTeacherMouthEnterFaceFeatureRatio() {
        let frames = [
            frame(timeSec: 0.2, names: ["Actor_Eyes"]),
            frame(timeSec: 0.4, names: ["Teacher_Mouth"]),
            frame(timeSec: 0.6, names: ["Teacher_Mouth"])
        ]
        let result = SessionBiomarkerAggregator.aggregate(validFrames: frames, durationSec: 1.0)
        XCTAssertEqual(result.eyeDwellRatio, 1.0 / 3.0, accuracy: 1e-9)
        XCTAssertEqual(result.mouthDwellRatio, 2.0 / 3.0, accuracy: 1e-9)
        XCTAssertEqual(result.firstOrientLatencyMs, 200.0, accuracy: 1e-6)
    }

    func testGiftBoxCompetesWithSocialPreference() {
        let frames = [
            frame(timeSec: 0.0, names: ["Actor_Eyes"]),
            frame(timeSec: 0.1, names: ["Gift_Box"]),
            frame(timeSec: 0.2, names: ["Gift_Box"])
        ]
        let result = SessionBiomarkerAggregator.aggregate(validFrames: frames, durationSec: 0.3)
        XCTAssertEqual(result.socialPreferenceScore, (1.0 - 2.0) / 3.0, accuracy: 1e-9)
    }

    func testPuppyControlDoesNotInflateSocialPreference() {
        let frames = [frame(timeSec: 0.2, names: ["Animal_Puppies"])]
        let result = SessionBiomarkerAggregator.aggregate(validFrames: frames, durationSec: 1.0)
        XCTAssertEqual(result.socialPreferenceScore, 0.0, accuracy: 1e-9)
        XCTAssertEqual(result.socialDwellSec, 0.0, accuracy: 1e-9)
        XCTAssertEqual(result.firstOrientLatencyMs, 1000.0, accuracy: 1e-6)
        XCTAssertEqual(result.eyeDwellRatio, SessionBiomarkerAggregator.unmeasuredEyeDwellRatio, accuracy: 1e-9)
    }

    func testSmileReactivityIsPeakAboveOpeningBaseline() {
        let frames = [
            frame(timeSec: 0.1, names: ["Social_Actor"], smile: 0.10),
            frame(timeSec: 0.4, names: ["Social_Actor"], smile: 0.10),
            frame(timeSec: 2.0, names: ["Social_Actor"], smile: 0.70)
        ]
        let result = SessionBiomarkerAggregator.aggregate(validFrames: frames, durationSec: 2.0)
        XCTAssertEqual(result.smileReactivityIndex, 0.60, accuracy: 1e-9)
    }

    func testTurnTakingAOIStillHitsAfterEightSeconds() {
        let tracker = StimulusAOITracker.tracker(forVideoNamed: "stim_09_social_turn_taking_10s")
        let hits = tracker.activeAOIs(atTimeSec: 9.2, xNorm: 0.50, yNorm: 0.40)
        XCTAssertTrue(hits.contains("Teacher_Face"))
        let mouth = tracker.activeAOIs(atTimeSec: 9.2, xNorm: 0.50, yNorm: 0.55)
        XCTAssertTrue(mouth.contains("Teacher_Mouth"))
    }

    func testPerClipEyeRatioIncludesActorEyes() {
        let sample = GazeTelemetrySample(
            timestampMs: 100,
            videoTimeSec: 0.1,
            xNorm: 0.5,
            yNorm: 0.3,
            confidence: 0.9,
            clipMediaTimeSec: 0.1
        )
        let frame = GazeFrame(
            gaze: sample,
            activeAOINames: ["Actor_Eyes"],
            qualityVerdict: .optimal,
            stimulusId: "stim_04_surprise_giftbox_10s",
            clipMediaTimeSec: 0.1
        )
        let sets = PerVideoMetricsExtractor.extract(from: [frame])
        XCTAssertEqual(sets[0].metrics[PerVideoMetricKey.eyeDwellRatio.key(targetAOI: nil)]!, 1.0, accuracy: 1e-9)
    }
}
