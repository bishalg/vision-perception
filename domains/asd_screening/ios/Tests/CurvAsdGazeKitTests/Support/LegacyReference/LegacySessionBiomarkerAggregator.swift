// Former Swift body of `SessionBiomarkerAggregator` at dev@3718ea6, moved here by WP-A.11 (FFI-3).
// The production type now delegates to the Rust core; this copy is the parity oracle for
// one release (GoldenParityTests, LegacyParityTests). Do not change its behaviour.
@testable import CurvAsdGazeKit
import Foundation

enum LegacySessionBiomarkerAggregator {
    typealias Aggregates = SessionBiomarkerAggregator.Aggregates

    static let frameIntervalSec: Double = 1.0 / 60.0

    static func aggregate(validFrames: [GazeFrame], durationSec: Double) -> Aggregates {
        guard !validFrames.isEmpty else {
            return Aggregates(
                socialDwellSec: 0,
                geometricDwellSec: 0,
                socialPreferenceScore: 0,
                eyeDwellRatio: nil,
                mouthDwellRatio: nil,
                firstOrientLatencyMs: nil,
                socialSmileIntensityMean: 0,
                smileReactivityIndex: 0,
                flatAffectScore: 1,
                surpriseScoreMean: 0
            )
        }

        var socialFrames = 0
        var geometricFrames = 0
        var eyeFrames = 0
        var mouthFrames = 0
        var firstOrientSec: Double?
        var smileSum = 0.0
        var surpriseSum = 0.0
        var expressivitySum = 0.0
        var peakSmile = 0.0
        var baselineSmileSum = 0.0
        var baselineCount = 0

        for frame in validFrames {
            if AOIRole.intersects(frame.activeAOINames, kind: .social) {
                socialFrames += 1
            }
            if AOIRole.intersects(frame.activeAOINames, kind: .geometric) {
                geometricFrames += 1
            }
            if AOIRole.intersects(frame.activeAOINames, kind: .eye) {
                eyeFrames += 1
            }
            if AOIRole.intersects(frame.activeAOINames, kind: .mouth) {
                mouthFrames += 1
            }

            let sessionSec = frame.gaze.videoTimeSec
            if firstOrientSec == nil, AOIRole.intersects(frame.activeAOINames, kind: .orienting) {
                firstOrientSec = sessionSec
            }

            let smile = frame.affect.socialSmileIntensity
            smileSum += smile
            peakSmile = max(peakSmile, smile)
            surpriseSum += frame.affect.surpriseScore
            expressivitySum += frame.affect.expressivityIndex
            if sessionSec <= 0.5 {
                baselineSmileSum += smile
                baselineCount += 1
            }
        }

        let socialDwell = Double(socialFrames) * frameIntervalSec
        let geometricDwell = Double(geometricFrames) * frameIntervalSec
        let totalPref = socialDwell + geometricDwell
        let preference = totalPref > 0 ? (socialDwell - geometricDwell) / totalPref : 0

        let faceFeatureFrames = eyeFrames + mouthFrames
        let eyeRatio: Double? = faceFeatureFrames > 0
            ? Double(eyeFrames) / Double(faceFeatureFrames)
            : nil
        let mouthRatio: Double? = faceFeatureFrames > 0
            ? Double(mouthFrames) / Double(faceFeatureFrames)
            : nil

        let count = Double(validFrames.count)
        let meanSmile = smileSum / count
        let baselineMean = baselineCount > 0 ? baselineSmileSum / Double(baselineCount) : meanSmile
        let reactivity = max(0.0, min(1.0, peakSmile - baselineMean))
        let meanExpressivity = expressivitySum / count
        let flatAffect = max(0.0, min(1.0, 1.0 - (meanExpressivity * 50.0)))

        return Aggregates(
            socialDwellSec: socialDwell,
            geometricDwellSec: geometricDwell,
            socialPreferenceScore: preference,
            eyeDwellRatio: eyeRatio,
            mouthDwellRatio: mouthRatio,
            firstOrientLatencyMs: firstOrientSec.map { max(0, $0) * 1000.0 },
            socialSmileIntensityMean: meanSmile,
            smileReactivityIndex: reactivity,
            flatAffectScore: flatAffect,
            surpriseScoreMean: surpriseSum / count
        )
    }
}
