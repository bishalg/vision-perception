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
        case .flatCenter: return "Hold your palm flat"
        case .tiltLeft: return "Tilt your hand a little to the left"
        case .tiltRight: return "Tilt your hand a little to the right"
        case .thumbToward: return "Turn your thumb slightly toward you"
        case .thumbAway: return "Turn your thumb slightly away"
        case .tiltForward: return "Tip your fingertips toward the camera"
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

    public var systemImageName: String {
        switch self {
        case .flatCenter: return "hand.raised.fill"
        case .tiltLeft: return "arrow.left.circle.fill"
        case .tiltRight: return "arrow.right.circle.fill"
        case .thumbToward: return "arrow.clockwise.circle.fill"
        case .thumbAway: return "arrow.counterclockwise.circle.fill"
        case .tiltForward: return "arrow.up.circle.fill"
        }
    }

    public var arrowSymbol: String {
        switch self {
        case .flatCenter: return "✋"
        case .tiltLeft: return "⇦"
        case .tiltRight: return "⇨"
        case .thumbToward: return "↻"
        case .thumbAway: return "↺"
        case .tiltForward: return "⇧"
        }
    }

    public var shortLabel: String {
        switch self {
        case .flatCenter: return "Flat"
        case .tiltLeft: return "Tilt Left"
        case .tiltRight: return "Tilt Right"
        case .thumbToward: return "Thumb In"
        case .thumbAway: return "Thumb Out"
        case .tiltForward: return "Fingers Up"
        }
    }

    public var coachingTip: String {
        switch self {
        case .flatCenter: return "Face your open palm flat to the camera. This sets the base level."
        case .tiltLeft: return "Keep the phone steady — slowly tilt only your hand left. When the ring turns green, hold still."
        case .tiltRight: return "Keep the phone steady — slowly tilt only your hand right. When the ring turns green, hold still."
        case .thumbToward: return "Keep the phone steady — turn your thumb a little toward the camera, then hold still."
        case .thumbAway: return "Keep the phone steady — turn your thumb a little away from the camera, then hold still."
        case .tiltForward: return "Keep the phone steady — tip your fingers slightly toward the camera, then hold still."
        }
    }
}
