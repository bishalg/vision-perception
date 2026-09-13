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
}
