import CoreGraphics
import Foundation

/**
 * Verifies the hand stays rigid during orbit capture — only wrist/arm may move.
 * Rejects bins when finger joints flex relative to the reference frame.
 */
public enum RigidHandChecker {
    /// Maximum normalized displacement allowed per landmark vs reference (hand-normalized).
    public static let maxJointDrift: CGFloat = 0.095
    /// Minimum spread between index and pinky MCP (normalized) — fingers must stay open.
    public static let minMcpSpread: CGFloat = 0.10

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

        // Anchor on the rigid metacarpal arch (knuckles 5, 9, 13, 17) — excluding wrist (0)
        // so bending or tilting at the wrist joint does NOT cause false rejects!
        let baseIndices = [5, 9, 13, 17]
        let curBaseX = baseIndices.map { current[$0].x }.reduce(0, +) / CGFloat(baseIndices.count)
        let curBaseY = baseIndices.map { current[$0].y }.reduce(0, +) / CGFloat(baseIndices.count)
        let refBaseX = baseIndices.map { reference[$0].x }.reduce(0, +) / CGFloat(baseIndices.count)
        let refBaseY = baseIndices.map { reference[$0].y }.reduce(0, +) / CGFloat(baseIndices.count)

        // Scale reference is palm breadth (5 to 17), strictly on the palm plate and invariant to wrist bending
        let curSpan = hypot(current[17].x - current[5].x, current[17].y - current[5].y)
        let refSpan = hypot(reference[17].x - reference[5].x, reference[17].y - reference[5].y)
        let scaleFactor = (refSpan > 0.001 && curSpan > 0.001) ? (curSpan / refSpan) : 1.0

        // Finger tips (4–8, 12, 16, 20) and IPs are most sensitive to flex.
        let sensitiveIndices = [4, 8, 12, 16, 20, 3, 7, 11, 15, 19]
        var maxDrift: CGFloat = 0
        for idx in sensitiveIndices {
            let expectedX = curBaseX + (reference[idx].x - refBaseX) * scaleFactor
            let expectedY = curBaseY + (reference[idx].y - refBaseY) * scaleFactor
            let d = hypot(current[idx].x - expectedX, current[idx].y - expectedY)
            maxDrift = max(maxDrift, d)
        }

        let isRigid = maxDrift <= maxJointDrift
        let reason: String? = isRigid ? nil : "Keep fingers still — move wrist only"
        return Result(isRigid: isRigid, digitSpreadOk: digitSpreadOk, maxDrift: maxDrift, reason: reason)
    }
}
