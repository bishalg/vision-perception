import Foundation

public enum HandSide: String, CaseIterable, Equatable {
    case auto = "auto"
    case left = "left"
    case right = "right"

    public var displayName: String {
        switch self {
        case .auto: return "Auto"
        case .left: return "Left"
        case .right: return "Right"
        }
    }
    public var icon: String {
        switch self {
        case .auto: return "hands.clap"
        case .left: return "hand.raised"
        case .right: return "hand.raised.fill"
        }
    }
}
