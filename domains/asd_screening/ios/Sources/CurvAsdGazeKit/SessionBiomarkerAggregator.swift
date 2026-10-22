import Foundation

/// Clinical roles for stimulus AOI names.
///
/// Preference compares person-directed regions with geometric or object regions.
/// Biological-motion controls (`Animal_Puppies`) and joint-attention targets
/// (`Gaze_Target_*`, `Center_Fixation_Target`) stay out of the preference ratio:
/// they are scored by their own per-clip metrics, and folding them into GeoPref
/// would credit a look away from a face as social preference.
public enum AOIRole {
    public static let social: Set<String> = [
        "Social_Actor",
        "Face_Center",
        "Face_Bounding_Box",
        "Eye_Region",
        "Actor_Face",
        "Actor_Eyes",
        "Cartoon_Character",
        "Teacher_Face",
        "Teacher_Mouth"
    ]

    public static let geometric: Set<String> = [
        "Geometric_Pattern",
        "Mechanical_Motion",
        "Gift_Box"
    ]

    public static let eye: Set<String> = [
        "Eye_Region",
        "Actor_Eyes"
    ]

    public static let mouth: Set<String> = [
        "Mouth_Region",
        "Teacher_Mouth"
    ]

    /// Person or eye regions used for first-orient latency.
    /// Matches the reported marker: time until the first fixation lands on the social actor.
    public static let orienting: Set<String> = [
        "Social_Actor",
        "Face_Center",
        "Face_Bounding_Box",
        "Eye_Region",
        "Actor_Face",
        "Actor_Eyes",
        "Teacher_Face"
    ]

    public static func intersects(_ names: [String], role: Set<String>) -> Bool {
        for name in names where role.contains(name) {
            return true
        }
        return false
    }
}

/// Session-level biomarker reduction shared by the live controller and the processing actor.
///
/// One frame counts at most once per role, so nested boxes (face + eyes) do not
/// double-count dwell. Eye and mouth ratios stay on the prior (0.28 / 0.14) only
/// when the session never hit a face-feature box, so a GeoPref-only clip is not
/// scored as if the child avoided the eyes. First-orient latency is the session
/// time of the first person/eye hit. Smile reactivity is the session peak minus
/// the mean smile in the first 500 ms. Per-clip scoring still applies that same
/// peak-minus-baseline formula inside each clip.
public enum SessionBiomarkerAggregator {
    public struct Aggregates: Equatable, Sendable {
        public let socialDwellSec: Double
        public let geometricDwellSec: Double
        public let socialPreferenceScore: Double
        public let eyeDwellRatio: Double
        public let mouthDwellRatio: Double
        public let firstOrientLatencyMs: Double
        public let socialSmileIntensityMean: Double
        public let smileReactivityIndex: Double
        public let flatAffectScore: Double
        public let surpriseScoreMean: Double
    }

    public static let unmeasuredEyeDwellRatio: Double = 0.28
    public static let unmeasuredMouthDwellRatio: Double = 0.14
    public static let frameIntervalSec: Double = 1.0 / 60.0

    public static func aggregate(validFrames: [GazeFrame], durationSec: Double) -> Aggregates {
        guard !validFrames.isEmpty else {
            return Aggregates(
                socialDwellSec: 0,
                geometricDwellSec: 0,
                socialPreferenceScore: 0,
                eyeDwellRatio: unmeasuredEyeDwellRatio,
                mouthDwellRatio: unmeasuredMouthDwellRatio,
                firstOrientLatencyMs: max(0, durationSec) * 1000.0,
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
            if AOIRole.intersects(frame.activeAOINames, role: AOIRole.social) {
                socialFrames += 1
            }
            if AOIRole.intersects(frame.activeAOINames, role: AOIRole.geometric) {
                geometricFrames += 1
            }
            if AOIRole.intersects(frame.activeAOINames, role: AOIRole.eye) {
                eyeFrames += 1
            }
            if AOIRole.intersects(frame.activeAOINames, role: AOIRole.mouth) {
                mouthFrames += 1
            }

            let sessionSec = frame.gaze.videoTimeSec
            if firstOrientSec == nil, AOIRole.intersects(frame.activeAOINames, role: AOIRole.orienting) {
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
        let eyeRatio = faceFeatureFrames > 0
            ? Double(eyeFrames) / Double(faceFeatureFrames)
            : unmeasuredEyeDwellRatio
        let mouthRatio = faceFeatureFrames > 0
            ? Double(mouthFrames) / Double(faceFeatureFrames)
            : unmeasuredMouthDwellRatio

        let count = Double(validFrames.count)
        let meanSmile = smileSum / count
        let baselineMean = baselineCount > 0 ? baselineSmileSum / Double(baselineCount) : meanSmile
        let reactivity = max(0.0, min(1.0, peakSmile - baselineMean))
        let meanExpressivity = expressivitySum / count
        let flatAffect = max(0.0, min(1.0, 1.0 - (meanExpressivity * 50.0)))
        let latencySec = firstOrientSec ?? durationSec

        return Aggregates(
            socialDwellSec: socialDwell,
            geometricDwellSec: geometricDwell,
            socialPreferenceScore: preference,
            eyeDwellRatio: eyeRatio,
            mouthDwellRatio: mouthRatio,
            firstOrientLatencyMs: max(0, latencySec) * 1000.0,
            socialSmileIntensityMean: meanSmile,
            smileReactivityIndex: reactivity,
            flatAffectScore: flatAffect,
            surpriseScoreMean: surpriseSum / count
        )
    }
}
