import Foundation

/**
 * Standard anatomical palmar pads for clinical and anthropometric research.
 *
 * Provides bidirectional translation to/from traditional palmistry mount names
 * (Samudrika Shastra) while keeping the core scanner and biometrics domain-agnostic.
 */
public enum AnatomicalPad: String, CaseIterable, Codable, Sendable {
    case thenar = "thenar"
    case hypothenar = "hypothenar"
    case radialInterdigital = "radial_interdigital"
    case centralInterdigital = "central_interdigital"
    case ringInterdigital = "ring_interdigital"
    case ulnarInterdigital = "ulnar_interdigital"
    case palmarFossa = "palmar_fossa"

    public var landmarkAnchors: [Int] {
        switch self {
        case .thenar: return [1, 2, 5]
        case .radialInterdigital: return [5, 6, 7]
        case .centralInterdigital: return [9, 10, 11]
        case .ringInterdigital: return [13, 14, 15]
        case .ulnarInterdigital: return [17, 18, 19]
        case .hypothenar: return [0, 17, 18]
        case .palmarFossa: return [5, 9, 13, 17]
        }
    }

    /// Translates from an astrological/Samudrika mount key (e.g. "venus") to the anatomical pad.
    public static func fromAstrologicalMount(_ name: String) -> AnatomicalPad? {
        switch name.lowercased() {
        case "venus": return .thenar
        case "luna", "moon": return .hypothenar
        case "jupiter": return .radialInterdigital
        case "saturn": return .centralInterdigital
        case "apollo", "sun": return .ringInterdigital
        case "mercury": return .ulnarInterdigital
        case "mars", "plain_of_mars": return .palmarFossa
        default: return nil
        }
    }

    /// Traditional astrological mount counterpart in Samudrika Shastra / palmistry.
    public var astrologicalMountName: String {
        switch self {
        case .thenar: return "venus"
        case .hypothenar: return "luna"
        case .radialInterdigital: return "jupiter"
        case .centralInterdigital: return "saturn"
        case .ringInterdigital: return "apollo"
        case .ulnarInterdigital: return "mercury"
        case .palmarFossa: return "mars"
        }
    }

    public var scientificDisplayName: String {
        switch self {
        case .thenar: return "Thenar Eminence"
        case .hypothenar: return "Hypothenar Eminence"
        case .radialInterdigital: return "Radial Interdigital Pad (II)"
        case .centralInterdigital: return "Central Interdigital Pad (III)"
        case .ringInterdigital: return "Ring Interdigital Pad (IV)"
        case .ulnarInterdigital: return "Ulnar Interdigital Pad (V)"
        case .palmarFossa: return "Palmar Fossa (Mid-Palm)"
        }
    }
}
