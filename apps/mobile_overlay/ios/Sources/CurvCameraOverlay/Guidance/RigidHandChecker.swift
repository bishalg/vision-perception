import CoreGraphics
import Foundation

/**
 * Verifies the hand stays rigid during orbit capture — only wrist/arm may move.
 * Rejects bins when finger joints flex relative to the reference frame.
 */
public enum RigidHandChecker {
    /// Maximum normalized displacement allowed per landmark vs reference.
    public static let maxJointDrift: CGFloat = 0.065
    /// Minimum spread between index and pinky MCP (normalized) — fingers must stay open.
    public static let minMcpSpread: CGFloat = 0.14

    public struct Result: Sendable {
        public let isRigid: Bool
        public let digitSpreadOk: Bool
        public let maxDrift: CGFloat
        public let reason: String?

        public init(isRigid: Bool, digitSpreadOk: Bool, maxDrift: CGFloat, reason: String? = nil) {
            self.isRigid = isRigid
            self.digitSpreadOk = digitSpreadOk
            self.maxDrift = maxDrift
            self.reason = reason
        }

        public var accepted: Bool { isRigid && digitSpreadOk }
    }

    /// Compare current 21-point landmarks against the orbit session reference.
    public static func evaluate(current: [CGPoint], reference: [CGPoint]?) -> Result {
        guard current.count >= 21 else {
            return Result(isRigid: false, digitSpreadOk: false, maxDrift: 1, reason: "Incomplete landmarks")
        }

        let spread = hypot(current[17].x - current[5].x, current[17].y - current[5].y)
        let digitSpreadOk = spread >= minMcpSpread
        if !digitSpreadOk {
            return Result(isRigid: false, digitSpreadOk: false, maxDrift: 0, reason: "Keep fingers spread")
        }

        guard let reference, reference.count >= 21 else {
            return Result(isRigid: true, digitSpreadOk: true, maxDrift: 0)
        }

        // Finger tips (4–8, 12, 16, 20) and IPs are most sensitive to flex.
        let sensitiveIndices = [4, 8, 12, 16, 20, 3, 7, 11, 15, 19]
        var maxDrift: CGFloat = 0
        for idx in sensitiveIndices {
            let d = hypot(current[idx].x - reference[idx].x, current[idx].y - reference[idx].y)
            maxDrift = max(maxDrift, d)
        }

        let isRigid = maxDrift <= maxJointDrift
        let reason: String? = isRigid ? nil : "Keep fingers still — move wrist only"
        return Result(isRigid: isRigid, digitSpreadOk: digitSpreadOk, maxDrift: maxDrift, reason: reason)
    }
}
