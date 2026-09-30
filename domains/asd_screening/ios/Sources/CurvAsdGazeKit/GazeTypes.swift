import Foundation
import CoreGraphics

/// Represents a single 60 Hz instantaneous gaze telemetry sample captured from TrueDepth.
public struct GazeTelemetrySample: Codable, Sendable, Equatable {
    public let timestampMs: Double
    public let videoTimeSec: Double
    public let xNorm: Double
    public let yNorm: Double
    public let confidence: Double
    public let leftEyeOpenness: Double
    public let rightEyeOpenness: Double
    public let headPitchRad: Double
    public let headYawRad: Double
    public let headRollRad: Double
    public let distanceToScreenCm: Double
    public let isValid: Bool
    /// Identifier of the stimulus clip playing when this sample was captured (e.g. "stim_05_geopref_split_10s").
    /// Nil for legacy sessions and single-battery GeoPref capture without playlist context.
    public let stimulusId: String?
    /// AVPlayer media clock seconds within the active clip (wraps on loop). Nil when unavailable.
    public let clipMediaTimeSec: Double?

    public init(
        timestampMs: Double,
        videoTimeSec: Double = 0.0,
        xNorm: Double,
        yNorm: Double,
        confidence: Double,
        leftEyeOpenness: Double = 1.0,
        rightEyeOpenness: Double = 1.0,
        headPitchRad: Double = 0.0,
        headYawRad: Double = 0.0,
        headRollRad: Double = 0.0,
        distanceToScreenCm: Double = 45.0,
        isValid: Bool = true,
        stimulusId: String? = nil,
        clipMediaTimeSec: Double? = nil
    ) {
        self.timestampMs = timestampMs
        self.videoTimeSec = videoTimeSec
        self.xNorm = xNorm
        self.yNorm = yNorm
        self.confidence = confidence
        self.leftEyeOpenness = leftEyeOpenness
        self.rightEyeOpenness = rightEyeOpenness
        self.headPitchRad = headPitchRad
        self.headYawRad = headYawRad
        self.headRollRad = headRollRad
        self.distanceToScreenCm = distanceToScreenCm
        self.isValid = isValid
        self.stimulusId = stimulusId
        self.clipMediaTimeSec = clipMediaTimeSec
    }

    enum CodingKeys: String, CodingKey {
        case timestampMs = "timestamp_ms"
        case videoTimeSec = "video_time_sec"
        case xNorm = "x_norm"
        case yNorm = "y_norm"
        case confidence
        case leftEyeOpenness = "left_eye_openness"
        case rightEyeOpenness = "right_eye_openness"
        case headPitchRad = "head_pitch_rad"
        case headYawRad = "head_yaw_rad"
        case headRollRad = "head_roll_rad"
        case distanceToScreenCm = "distance_to_screen_cm"
        case isValid = "valid"
        case stimulusId = "stimulus_id"
        case clipMediaTimeSec = "clip_media_time_sec"
    }
}

/// Output of projecting TrueDepth 3D ocular transforms onto 2D screen geometry.
public struct ScreenRaycastResult: Sendable, Equatable {
    public let screenPoint: CGPoint
    public let normalizedPoint: CGPoint
    public let leftEyePoint: CGPoint?
    public let rightEyePoint: CGPoint?
    public let distanceCm: Double
    public let isWithinScreen: Bool

    public init(
        screenPoint: CGPoint,
        normalizedPoint: CGPoint,
        leftEyePoint: CGPoint? = nil,
        rightEyePoint: CGPoint? = nil,
        distanceCm: Double,
        isWithinScreen: Bool
    ) {
        self.screenPoint = screenPoint
        self.normalizedPoint = normalizedPoint
        self.leftEyePoint = leftEyePoint
        self.rightEyePoint = rightEyePoint
        self.distanceCm = distanceCm
        self.isWithinScreen = isWithinScreen
    }
}

/// Represents a clinical Area of Interest (AOI) bounding box in normalized screen coordinates.
public struct StimulusAOI: Codable, Sendable, Equatable {
    public let name: String
    public let startTimeSec: Double
    public let endTimeSec: Double
    public let xMin: Double
    public let yMin: Double
    public let xMax: Double
    public let yMax: Double

    public init(
        name: String,
        startTimeSec: Double = 0.0,
        endTimeSec: Double = 9999.0,
        xMin: Double,
        yMin: Double,
        xMax: Double,
        yMax: Double
    ) {
        self.name = name
        self.startTimeSec = startTimeSec
        self.endTimeSec = endTimeSec
        self.xMin = xMin
        self.yMin = yMin
        self.xMax = xMax
        self.yMax = yMax
    }

    public func contains(x: Double, y: Double, atTime: Double = 0.0) -> Bool {
        guard atTime >= startTimeSec && atTime <= endTimeSec else { return false }
        return x >= xMin && x <= xMax && y >= yMin && y <= yMax
    }
}

/// Interactive calibration target point (e.g. 5-point bubble).
public struct CalibrationTarget: Identifiable, Sendable {
    public let id: Int
    public let position: CGPoint
    public let normalizedTarget: CGPoint
    public var isPopped: Bool

    public init(id: Int, position: CGPoint, normalizedTarget: CGPoint, isPopped: Bool = false) {
        self.id = id
        self.position = position
        self.normalizedTarget = normalizedTarget
        self.isPopped = isPopped
    }
}

// MARK: - 3-Stage Tracking Pipeline

/// Represents the current stage in the eye-gaze tracking pipeline.
///
/// The pipeline has 3 progressive stages that must all be healthy before a
/// calibrated gaze coordinate is trustworthy. The UI observes this enum to show
/// clear actionable guidance to the caregiver at each stage.
///
/// Stage order: `.searchingFace` → `.faceAligned` → `.eyesTracking` → `.gazeActive`
public enum TrackingStage: Equatable, Sendable {
    /// No face anchor detected by ARKit.
    case searchingFace

    /// Face detected and within usable distance/angle bounds.
    case faceAligned(distanceCm: Double)

    /// Face aligned and both eyes are open and being tracked.
    case eyesTracking(leftOpenness: Double, rightOpenness: Double)

    /// All stages healthy — a calibrated gaze coordinate is being produced.
    case gazeActive

    /// Returns true when face is detected (stage ≥ 1).
    public var hasFace: Bool {
        if case .searchingFace = self { return false }
        return true
    }

    /// Returns true when both face and eye tracking are healthy (stage ≥ 2).
    public var hasEyes: Bool {
        switch self {
        case .eyesTracking, .gazeActive: return true
        default: return false
        }
    }

    /// Returns true when all pipeline stages are healthy.
    public var isGazeActive: Bool {
        if case .gazeActive = self { return true }
        return false
    }

    /// Human-readable status label for UI display.
    public var label: String {
        switch self {
        case .searchingFace:        return "Searching for face…"
        case .faceAligned(let d):   return "Face aligned • \(Int(d)) cm"
        case .eyesTracking:         return "Eyes detected"
        case .gazeActive:           return "Gaze active ✓"
        }
    }
}
