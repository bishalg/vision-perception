import CoreGraphics
import SwiftUI

/**
 * Shared shape-spec runtime for the native overlays.
 *
 * Anatomy-free core: decodes segment lists from the shared shapes JSON files
 * under apps/mobile_overlay/shared/shapes (single source of truth) and
 * builds normalized-coordinate `Path`s. Palm, Vastu, and camera chrome all
 * build on this; no screen-specific geometry lives here.
 *
 * Rules (mirroring `shared/shapes/README.md`):
 * - Normalized 0..1 space, y down. Map with x * rect.width / y * rect.height.
 * - Mirror x for the opposite hand; never mirror labels, only geometry.
 * - Fill the closed silhouette loop once; stroke finger segments individually.
 */
public struct GuideSegment: Codable, Equatable {
    public let move: [Double]
    public let curves: [[Double]]

    public init(move: [Double], curves: [[Double]]) {
        self.move = move
        self.curves = curves
    }

    public var startPoint: CGPoint? {
        guard move.count == 2 else { return nil }
        return CGPoint(x: move[0], y: move[1])
    }

    public var endPoint: CGPoint? {
        if let last = curves.last, last.count == 6 {
            return CGPoint(x: last[4], y: last[5])
        }
        return startPoint
    }

    public var isWellFormed: Bool {
        guard move.count == 2 else { return false }
        return curves.allSatisfy { $0.count == 6 }
    }
}

public enum GuideSpecError: Error, Equatable {
    case malformedSegment(String)
    case emptyDocument
}

public enum GuideSpec {
    /// Decodes the `segments` subtree of silhouette.json into dotted keys
    /// ("fingers.pinky"). Throws on malformed documents — never partial art.
    public static func loadSegments(from data: Data) throws -> [String: GuideSegment] {
        let json = try JSONSerialization.jsonObject(with: data)
        guard let doc = json as? [String: Any],
              let segments = doc["segments"] as? [String: Any] else {
            throw GuideSpecError.emptyDocument
        }
        var out: [String: GuideSegment] = [:]
        flatten(node: segments, prefix: "", into: &out)
        if out.isEmpty { throw GuideSpecError.emptyDocument }
        for (key, segment) in out where !segment.isWellFormed {
            throw GuideSpecError.malformedSegment(key)
        }
        return out
    }

    private static func flatten(node: [String: Any], prefix: String, into out: inout [String: GuideSegment]) {
        for (key, value) in node {
            let dotted = prefix.isEmpty ? key : "\(prefix).\(key)"
            if let dict = value as? [String: Any],
               let move = dict["move"] as? [Double],
               let curves = dict["curves"] as? [[Double]] {
                out[dotted] = GuideSegment(move: move, curves: curves)
            } else if let dict = value as? [String: Any] {
                flatten(node: dict, prefix: dotted, into: &out)
            }
        }
    }

    /// Builds a strokable/fillable path for one segment in a target rect.
    public static func path(for segment: GuideSegment, in rect: CGRect, mirror: Bool = false) -> Path {
        func px(_ x: Double) -> CGFloat {
            let nx = mirror ? 1.0 - x : x
            return rect.minX + CGFloat(nx) * rect.width
        }
        func py(_ y: Double) -> CGFloat {
            rect.minY + CGFloat(y) * rect.height
        }
        var path = Path()
        guard segment.move.count == 2 else { return path }
        path.move(to: CGPoint(x: px(segment.move[0]), y: py(segment.move[1])))
        for curve in segment.curves where curve.count == 6 {
            path.addCurve(
                to: CGPoint(x: px(curve[4]), y: py(curve[5])),
                control1: CGPoint(x: px(curve[0]), y: py(curve[1])),
                control2: CGPoint(x: px(curve[2]), y: py(curve[3]))
            )
        }
        return path
    }
}

public struct GuideTheme: Codable, Equatable {
    public let colors: [String: String]

    public func color(_ name: String, alpha: Double = 1.0) -> Color? {
        guard let hex = colors[name] else { return nil }
        return Color(hex: hex, alpha: alpha)
    }
}

extension Color {
    /// Parses "#RRGGBB" (as stored in overlay_theme.json).
    public init?(hex: String, alpha: Double = 1.0) {
        var cleaned = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if cleaned.hasPrefix("#") { cleaned.removeFirst() }
        guard cleaned.count == 6, let value = UInt64(cleaned, radix: 16) else { return nil }
        let divisor = 255.0
        self.init(
            .sRGB,
            red: Double((value >> 16) & 0xFF) / divisor,
            green: Double((value >> 8) & 0xFF) / divisor,
            blue: Double(value & 0xFF) / divisor,
            opacity: alpha
        )
    }
}
