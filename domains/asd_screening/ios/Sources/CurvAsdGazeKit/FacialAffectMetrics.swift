import Foundation

/// High-level classified emotional affect state based on FACS Action Units.
public enum DominantEmotion: String, Codable, CaseIterable, Identifiable, Sendable {
    case calm = "Calm Attention"
    case joy = "Joy & Smiling"
    case surprise = "Surprise & Wonder"
    case distress = "Distress & Frustration"
    case excited = "High Excitement"

    public var id: String { rawValue }

    public var emoji: String {
        switch self {
        case .calm: return "🙂"
        case .joy: return "😊"
        case .surprise: return "😲"
        case .distress: return "😟"
        case .excited: return "🤩"
        }
    }

    public var systemIcon: String {
        switch self {
        case .calm: return "face.smiling"
        case .joy: return "face.smiling.fill"
        case .surprise: return "eyes"
        case .distress: return "exclamationmark.triangle.fill"
        case .excited: return "sparkles"
        }
    }
}

/// Complete 52-field FACS Facial Affect Metrics from Apple TrueDepth ARFaceAnchor.blendShapes.
///
/// All values are normalized floats in [0.0, 1.0]:
///   - 0.0 = muscle fully relaxed
///   - 1.0 = maximum muscle contraction
///
/// Zero raw camera frames are stored. Only numeric coefficients are recorded.
/// See docs/DATA_COLLECTION_SCHEMA.md §2.3 for the complete FACS AU reference table.
public struct FacialAffectMetrics: Codable, Sendable, Equatable {

    // MARK: - Joy / Duchenne Smile (FACS AU6 + AU12)
    public let mouthSmileLeft: Double
    public let mouthSmileRight: Double
    public let cheekSquintLeft: Double
    public let cheekSquintRight: Double

    // MARK: - Surprise / High Visual Attention (FACS AU1+2+5+27)
    public let browInnerUp: Double
    public let browOuterUpLeft: Double
    public let browOuterUpRight: Double
    public let eyeWideLeft: Double
    public let eyeWideRight: Double
    public let jawOpen: Double

    // MARK: - Distress / Negative Affect (FACS AU4+15+17)
    public let browDownLeft: Double
    public let browDownRight: Double
    public let mouthFrownLeft: Double
    public let mouthFrownRight: Double
    public let mouthRollLower: Double
    public let mouthRollUpper: Double
    public let mouthPucker: Double

    // MARK: - Eye Direction via Blend Shapes (redundant confirmation)
    public let eyeLookDownLeft: Double
    public let eyeLookDownRight: Double
    public let eyeLookInLeft: Double
    public let eyeLookInRight: Double
    public let eyeLookOutLeft: Double
    public let eyeLookOutRight: Double
    public let eyeLookUpLeft: Double
    public let eyeLookUpRight: Double
    public let eyeSquintLeft: Double
    public let eyeSquintRight: Double
    public let eyeBlinkLeft: Double
    public let eyeBlinkRight: Double

    // MARK: - Additional Mouth Movements
    public let mouthClose: Double
    public let mouthDimpleLeft: Double
    public let mouthDimpleRight: Double
    public let mouthFunnel: Double
    public let mouthLeft: Double
    public let mouthRight: Double
    public let mouthLowerDownLeft: Double
    public let mouthLowerDownRight: Double
    public let mouthPressLeft: Double
    public let mouthPressRight: Double
    public let mouthShrugLower: Double
    public let mouthShrugUpper: Double
    public let mouthStretchLeft: Double
    public let mouthStretchRight: Double
    public let mouthUpperUpLeft: Double
    public let mouthUpperUpRight: Double

    // MARK: - Nose, Cheek, Jaw
    public let noseSneerLeft: Double
    public let noseSneerRight: Double
    public let cheekPuff: Double
    public let jawForward: Double
    public let jawLeft: Double
    public let jawRight: Double

    // MARK: - Playful / Non-social
    public let tongueOut: Double

    // MARK: - Derived Composite Scores (computed, not raw blend shapes)

    /// Duchenne Smile Intensity: mean of mouth smile + cheek squint (AU12 + AU6).
    /// Values > 0.40 indicate genuine social smile. Used for Smile Reactivity Index.
    public var socialSmileIntensity: Double {
        (mouthSmileLeft + mouthSmileRight + cheekSquintLeft + cheekSquintRight) / 4.0
    }

    /// Surprise Composite Score from AU1+2+5+27.
    /// Values > 0.30 indicate high visual attention or startle response.
    public var surpriseScore: Double {
        (browInnerUp + browOuterUpLeft + browOuterUpRight
         + eyeWideLeft + eyeWideRight + jawOpen) / 6.0
    }

    /// Distress / Negative Affect Composite (AU4+15+17+30).
    /// Values > 0.25 may indicate sensory overwhelm or discomfort.
    public var distressScore: Double {
        (browDownLeft + browDownRight
         + mouthFrownLeft + mouthFrownRight
         + mouthRollLower) / 5.0
    }

    /// Facial Expressivity Index: variance of key action units.
    /// Low values (< 0.005 sustained across session) indicate flat / blunted affect.
    public var expressivityIndex: Double {
        let keyMuscles: [Double] = [
            mouthSmileLeft, mouthSmileRight,
            browInnerUp, browOuterUpLeft, browOuterUpRight,
            eyeWideLeft, eyeWideRight,
            browDownLeft, browDownRight,
            mouthFrownLeft, mouthFrownRight,
            jawOpen
        ]
        let mean = keyMuscles.reduce(0, +) / Double(keyMuscles.count)
        return keyMuscles.map { ($0 - mean) * ($0 - mean) }.reduce(0, +) / Double(keyMuscles.count)
    }

    /// Excitement composite metric combining high smiling and facial dynamism (0.0 to 1.0).
    public var excitementScore: Double {
        min(1.0, (socialSmileIntensity * 0.5 + expressivityIndex * 35.0 * 0.5))
    }

    /// Anger & frustration metric combining brow furrow, frown, and nose sneer (0.0 to 1.0).
    public var angerScore: Double {
        min(1.0, (browDownLeft + browDownRight + noseSneerLeft + noseSneerRight + mouthFrownLeft + mouthFrownRight) / 6.0)
    }

    /// Dominant classified emotion based on FACS Action Unit thresholds.
    public var dominantEmotion: DominantEmotion {
        if socialSmileIntensity > 0.40 && expressivityIndex > 0.008 {
            return .excited
        } else if socialSmileIntensity > 0.35 {
            return .joy
        } else if surpriseScore > 0.28 {
            return .surprise
        } else if distressScore > 0.25 || angerScore > 0.25 {
            return .distress
        } else {
            return .calm
        }
    }


    // MARK: - Convenience: zero-affect neutral baseline (Simulator / unit tests)
    public static var neutral: FacialAffectMetrics { FacialAffectMetrics() }

    public init(
        mouthSmileLeft: Double = 0, mouthSmileRight: Double = 0,
        cheekSquintLeft: Double = 0, cheekSquintRight: Double = 0,
        browInnerUp: Double = 0, browOuterUpLeft: Double = 0, browOuterUpRight: Double = 0,
        eyeWideLeft: Double = 0, eyeWideRight: Double = 0, jawOpen: Double = 0,
        browDownLeft: Double = 0, browDownRight: Double = 0,
        mouthFrownLeft: Double = 0, mouthFrownRight: Double = 0,
        mouthRollLower: Double = 0, mouthRollUpper: Double = 0, mouthPucker: Double = 0,
        eyeLookDownLeft: Double = 0, eyeLookDownRight: Double = 0,
        eyeLookInLeft: Double = 0, eyeLookInRight: Double = 0,
        eyeLookOutLeft: Double = 0, eyeLookOutRight: Double = 0,
        eyeLookUpLeft: Double = 0, eyeLookUpRight: Double = 0,
        eyeSquintLeft: Double = 0, eyeSquintRight: Double = 0,
        eyeBlinkLeft: Double = 0, eyeBlinkRight: Double = 0,
        mouthClose: Double = 0, mouthDimpleLeft: Double = 0, mouthDimpleRight: Double = 0,
        mouthFunnel: Double = 0, mouthLeft: Double = 0, mouthRight: Double = 0,
        mouthLowerDownLeft: Double = 0, mouthLowerDownRight: Double = 0,
        mouthPressLeft: Double = 0, mouthPressRight: Double = 0,
        mouthShrugLower: Double = 0, mouthShrugUpper: Double = 0,
        mouthStretchLeft: Double = 0, mouthStretchRight: Double = 0,
        mouthUpperUpLeft: Double = 0, mouthUpperUpRight: Double = 0,
        noseSneerLeft: Double = 0, noseSneerRight: Double = 0,
        cheekPuff: Double = 0, jawForward: Double = 0, jawLeft: Double = 0, jawRight: Double = 0,
        tongueOut: Double = 0
    ) {
        self.mouthSmileLeft = mouthSmileLeft; self.mouthSmileRight = mouthSmileRight
        self.cheekSquintLeft = cheekSquintLeft; self.cheekSquintRight = cheekSquintRight
        self.browInnerUp = browInnerUp; self.browOuterUpLeft = browOuterUpLeft
        self.browOuterUpRight = browOuterUpRight
        self.eyeWideLeft = eyeWideLeft; self.eyeWideRight = eyeWideRight; self.jawOpen = jawOpen
        self.browDownLeft = browDownLeft; self.browDownRight = browDownRight
        self.mouthFrownLeft = mouthFrownLeft; self.mouthFrownRight = mouthFrownRight
        self.mouthRollLower = mouthRollLower; self.mouthRollUpper = mouthRollUpper
        self.mouthPucker = mouthPucker
        self.eyeLookDownLeft = eyeLookDownLeft; self.eyeLookDownRight = eyeLookDownRight
        self.eyeLookInLeft = eyeLookInLeft; self.eyeLookInRight = eyeLookInRight
        self.eyeLookOutLeft = eyeLookOutLeft; self.eyeLookOutRight = eyeLookOutRight
        self.eyeLookUpLeft = eyeLookUpLeft; self.eyeLookUpRight = eyeLookUpRight
        self.eyeSquintLeft = eyeSquintLeft; self.eyeSquintRight = eyeSquintRight
        self.eyeBlinkLeft = eyeBlinkLeft; self.eyeBlinkRight = eyeBlinkRight
        self.mouthClose = mouthClose
        self.mouthDimpleLeft = mouthDimpleLeft; self.mouthDimpleRight = mouthDimpleRight
        self.mouthFunnel = mouthFunnel; self.mouthLeft = mouthLeft; self.mouthRight = mouthRight
        self.mouthLowerDownLeft = mouthLowerDownLeft; self.mouthLowerDownRight = mouthLowerDownRight
        self.mouthPressLeft = mouthPressLeft; self.mouthPressRight = mouthPressRight
        self.mouthShrugLower = mouthShrugLower; self.mouthShrugUpper = mouthShrugUpper
        self.mouthStretchLeft = mouthStretchLeft; self.mouthStretchRight = mouthStretchRight
        self.mouthUpperUpLeft = mouthUpperUpLeft; self.mouthUpperUpRight = mouthUpperUpRight
        self.noseSneerLeft = noseSneerLeft; self.noseSneerRight = noseSneerRight
        self.cheekPuff = cheekPuff; self.jawForward = jawForward
        self.jawLeft = jawLeft; self.jawRight = jawRight
        self.tongueOut = tongueOut
    }
}

/// Complete multimodal 60 Hz frame record combining gaze, head pose,
/// facial affect, AOI hit, stimulus event, and quality verdict.
/// One of these is created per 60 Hz tick and written to the encrypted session JSONL file.
/// See docs/DATA_COLLECTION_SCHEMA.md for the complete field reference.
public struct GazeFrame: Codable, Sendable {
    public let gaze: GazeTelemetrySample
    public let affect: FacialAffectMetrics
    public let activeAOINames: [String]
    public let aoiDwellMs: Double
    public let stimulusEventTag: String?
    public let qualityVerdict: GazeQualityGate.Verdict

    public init(
        gaze: GazeTelemetrySample,
        affect: FacialAffectMetrics = .neutral,
        activeAOINames: [String] = [],
        aoiDwellMs: Double = 0.0,
        stimulusEventTag: String? = nil,
        qualityVerdict: GazeQualityGate.Verdict = .optimal
    ) {
        self.gaze = gaze
        self.affect = affect
        self.activeAOINames = activeAOINames
        self.aoiDwellMs = aoiDwellMs
        self.stimulusEventTag = stimulusEventTag
        self.qualityVerdict = qualityVerdict
    }
}

/// Session-level metadata record. Written as the first JSONL line in every session file.
/// See docs/DATA_COLLECTION_SCHEMA.md §3 for field definitions.
public struct SessionHeader: Codable, Sendable {
    public let schemaVersion: String
    public let sessionId: String
    public let participantToken: String
    public let stimulusBattery: String
    public let stimulusDurationSec: Double
    public let calibrationQuality: String
    public let calibrationErrorDeg: Double
    public let deviceModel: String
    public let screenWidthPt: Double
    public let screenHeightPt: Double
    public let screenWidthM: Double
    public let screenHeightM: Double
    public let arkitVersion: String
    public let appVersion: String
    public let sessionStartUtc: String

    public init(
        sessionId: String,
        participantToken: String,
        stimulusBattery: String,
        stimulusDurationSec: Double,
        calibrationQuality: String = "validated",
        calibrationErrorDeg: Double = 0.0,
        deviceModel: String = "Unknown",
        screenWidthPt: Double = 393.0,
        screenHeightPt: Double = 852.0,
        screenWidthM: Double = 0.0713,
        screenHeightM: Double = 0.1546,
        arkitVersion: String = "6.0",
        appVersion: String = "1.0.0"
    ) {
        self.schemaVersion = "1.1.0"
        self.sessionId = sessionId
        self.participantToken = participantToken
        self.stimulusBattery = stimulusBattery
        self.stimulusDurationSec = stimulusDurationSec
        self.calibrationQuality = calibrationQuality
        self.calibrationErrorDeg = calibrationErrorDeg
        self.deviceModel = deviceModel
        self.screenWidthPt = screenWidthPt
        self.screenHeightPt = screenHeightPt
        self.screenWidthM = screenWidthM
        self.screenHeightM = screenHeightM
        self.arkitVersion = arkitVersion
        self.appVersion = appVersion
        self.sessionStartUtc = ISO8601DateFormatter().string(from: Date())
    }
}
