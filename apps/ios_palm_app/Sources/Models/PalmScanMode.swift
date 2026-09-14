import Foundation

/**
 * Capture protocol selector — Mode A (single still) vs enriched 3D scan modes.
 */
public enum PalmScanMode: String, CaseIterable, Identifiable {
    case stillPhoto = "still"
    case orbitScan = "orbit"
    case closeUpMosaic = "closeup"
    case fingertipScan = "fingertip"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .stillPhoto: return "Still Photo"
        case .orbitScan: return "Orbit Scan"
        case .closeUpMosaic: return "Close-up Lines"
        case .fingertipScan: return "Fingertips"
        }
    }

    public var subtitle: String {
        switch self {
        case .stillPhoto: return "Classic single capture"
        case .orbitScan: return "Tilt hand for mount height map"
        case .closeUpMosaic: return "Fine crease detail tiles"
        case .fingertipScan: return "Dermatoglyphic pattern class"
        }
    }

    public var iconName: String {
        switch self {
        case .stillPhoto: return "camera.fill"
        case .orbitScan: return "rotate.3d"
        case .closeUpMosaic: return "plus.magnifyingglass"
        case .fingertipScan: return "hand.point.up.fill"
        }
    }

    public var shortTitle: String {
        switch self {
        case .stillPhoto: return "Still"
        case .orbitScan: return "3D Orbit"
        case .closeUpMosaic: return "Close-Up"
        case .fingertipScan: return "Fingertips"
        }
    }
}
