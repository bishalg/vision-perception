import Foundation

/**
 * Discrete viewing bins for Mode B orbit capture.
 * User keeps fingers rigid and tilts the whole hand to fill each bin.
 */
public enum OrbitBin: String, CaseIterable, Codable, Sendable {
    case flatCenter = "flat_center"
    case tiltLeft = "tilt_left"
    case tiltRight = "tilt_right"
    case thumbToward = "thumb_toward"
    case thumbAway = "thumb_away"
    case tiltForward = "tilt_forward"

    public var displayName: String {
        switch self {
        case .flatCenter: return "Hold palm flat, centered"
        case .tiltLeft: return "Tilt hand left ~20°"
        case .tiltRight: return "Tilt hand right ~20°"
        case .thumbToward: return "Rotate thumb toward camera"
        case .thumbAway: return "Rotate thumb away from camera"
        case .tiltForward: return "Tilt fingers toward camera"
        }
    }

    /// Expected device roll band (degrees) for coaching when hand is flat in frame.
    public var expectedRollDeg: ClosedRange<Float> {
        switch self {
        case .flatCenter: return -8...8
        case .tiltLeft: return 12...28
        case .tiltRight: return (-28)...(-12)
        case .thumbToward: return -5...5
        case .thumbAway: return -5...5
        case .tiltForward: return -5...5
        }
    }

    /// Mounts this bin primarily illuminates for coverage HUD.
    public var primaryMounts: [String] {
        switch self {
        case .flatCenter: return ["venus", "jupiter", "saturn", "apollo", "mercury", "luna", "mars"]
        case .tiltLeft: return ["venus", "jupiter", "mars"]
        case .tiltRight: return ["luna", "mercury", "apollo"]
        case .thumbToward: return ["venus", "jupiter"]
        case .thumbAway: return ["luna", "mars"]
        case .tiltForward: return ["venus", "saturn", "apollo"]
        }
    }
}
