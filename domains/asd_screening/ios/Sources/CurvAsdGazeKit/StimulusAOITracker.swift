import Foundation

/// Manages dynamic Areas of Interest (AOIs) and performs time-indexed hit testing.
public struct StimulusAOITracker: Sendable {
    public let aois: [StimulusAOI]

    public init(aois: [StimulusAOI] = []) {
        self.aois = aois
    }

    /// Returns all AOI names intersecting the gaze coordinate at the specified video timestamp.
    public func activeAOIs(atTimeSec timeSec: Double, xNorm: Double, yNorm: Double) -> [String] {
        return aois.compactMap { aoi in
            aoi.contains(x: xNorm, y: yNorm, atTime: timeSec) ? aoi.name : nil
        }
    }

    /// Standard split-screen Geometric vs. Social Preference (GeoPref) battery AOI schema.
    public static func makeGeoPrefBattery() -> StimulusAOITracker {
        let aois: [StimulusAOI] = [
            StimulusAOI(name: "Social_Actor", startTimeSec: 0.0, endTimeSec: 30.0, xMin: 0.0, yMin: 0.0, xMax: 0.5, yMax: 1.0),
            StimulusAOI(name: "Geometric_Pattern", startTimeSec: 0.0, endTimeSec: 30.0, xMin: 0.5, yMin: 0.0, xMax: 1.0, yMax: 1.0)
        ]
        return StimulusAOITracker(aois: aois)
    }

    /// Naturalistic face scanning battery with Eye and Mouth region AOIs.
    public static func makeFaceScanningBattery() -> StimulusAOITracker {
        let aois: [StimulusAOI] = [
            StimulusAOI(name: "Face_Bounding_Box", startTimeSec: 0.0, endTimeSec: 20.0, xMin: 0.25, yMin: 0.15, xMax: 0.75, yMax: 0.85),
            StimulusAOI(name: "Eye_Region", startTimeSec: 0.0, endTimeSec: 20.0, xMin: 0.35, yMin: 0.30, xMax: 0.65, yMax: 0.45),
            StimulusAOI(name: "Mouth_Region", startTimeSec: 0.0, endTimeSec: 20.0, xMin: 0.40, yMin: 0.58, xMax: 0.60, yMax: 0.72)
        ]
        return StimulusAOITracker(aois: aois)
    }

    /// Standardized 10-video clinical battery AOI mappings.
    public static func makeStandard10ClipBattery() -> [String: StimulusAOITracker] {
        return [
            "stim_01_social_peekaboo_8s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Face_Center", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.25, yMin: 0.15, xMax: 0.75, yMax: 0.85),
                StimulusAOI(name: "Eye_Region", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.35, yMin: 0.28, xMax: 0.65, yMax: 0.46)
            ]),
            "stim_02_gaze_cue_right_8s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Actor_Face", startTimeSec: 0.0, endTimeSec: 3.5, xMin: 0.30, yMin: 0.18, xMax: 0.70, yMax: 0.70),
                StimulusAOI(name: "Gaze_Target_Right", startTimeSec: 3.5, endTimeSec: 8.0, xMin: 0.65, yMin: 0.08, xMax: 0.98, yMax: 0.45)
            ]),
            "stim_03_gaze_cue_left_8s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Actor_Face", startTimeSec: 0.0, endTimeSec: 3.0, xMin: 0.30, yMin: 0.18, xMax: 0.70, yMax: 0.70),
                StimulusAOI(name: "Gaze_Target_Left", startTimeSec: 3.0, endTimeSec: 8.0, xMin: 0.02, yMin: 0.08, xMax: 0.35, yMax: 0.45)
            ]),
            "stim_04_surprise_giftbox_10s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Actor_Eyes", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.36, yMin: 0.20, xMax: 0.64, yMax: 0.38),
                StimulusAOI(name: "Gift_Box", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.28, yMin: 0.55, xMax: 0.72, yMax: 0.95)
            ]),
            "stim_05_geopref_split_10s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Social_Actor", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.0, yMin: 0.0, xMax: 0.5, yMax: 1.0),
                StimulusAOI(name: "Geometric_Pattern", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.5, yMin: 0.0, xMax: 1.0, yMax: 1.0)
            ]),
            "stim_06_dynamic_gears_marbles_8s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Mechanical_Motion", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.10, yMin: 0.10, xMax: 0.90, yMax: 0.90)
            ]),
            "stim_07_cartoon_slapstick_penguin_8s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Cartoon_Character", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.30, yMin: 0.30, xMax: 0.70, yMax: 0.78)
            ]),
            "stim_08_recenter_bubble_fox_3s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Center_Fixation_Target", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.38, yMin: 0.38, xMax: 0.62, yMax: 0.62)
            ]),
            "stim_09_social_turn_taking_10s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Teacher_Face", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.28, yMin: 0.15, xMax: 0.72, yMax: 0.75),
                StimulusAOI(name: "Teacher_Mouth", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.40, yMin: 0.48, xMax: 0.60, yMax: 0.64)
            ]),
            "stim_10_animal_puppy_play_8s": StimulusAOITracker(aois: [
                StimulusAOI(name: "Animal_Puppies", startTimeSec: 0.0, endTimeSec: 8.0, xMin: 0.18, yMin: 0.25, xMax: 0.82, yMax: 0.85)
            ])
        ]
    }

    /// Resolves the specific AOI tracker for a given video file name.
    public static func tracker(forVideoNamed name: String) -> StimulusAOITracker {
        let cleanName = name.replacingOccurrences(of: ".mp4", with: "")
        let battery = makeStandard10ClipBattery()
        if let match = battery[cleanName] {
            return match
        }
        // Default to split GeoPref or face scanning
        if cleanName.contains("geopref") || cleanName.contains("split") {
            return makeGeoPrefBattery()
        }
        return makeFaceScanningBattery()
    }
}
