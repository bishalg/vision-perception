import CoreGraphics
import CurvCameraOverlay
import Foundation

/**
 * Maps MediaPipe / Vision 21-point hand landmarks into shared `SubjectPose`.
 * Scale = wrist → middle fingertip (full hand framing).
 */
public struct HandFraming: Equatable {
    public let pose: SubjectPose
    /// False when fingertips / wrist sit too close to the frame edge.
    public let isFullyInFrame: Bool
}

public enum HandLandmarkPose {
    /// Soft edge pad — only reject capture readiness when tips are nearly clipped.
    private static let hardMargin: CGFloat = 0.015
    /// Comfortable pad used for “move back a little” coaching.
    private static let softMargin: CGFloat = 0.06

    public static func framing(from landmarks: [CGPoint]) -> HandFraming? {
        guard landmarks.count >= 21 else { return nil }

        let wrist = landmarks[0]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let middleTip = landmarks[12]
        let pinkyMcp = landmarks[17]
        let tips = [landmarks[4], landmarks[8], middleTip, landmarks[16], landmarks[20], wrist]

        // Hard reject only when geometry is unusable (off-screen).
        for tip in tips {
            if tip.x < hardMargin || tip.x > 1 - hardMargin || tip.y < hardMargin || tip.y > 1 - hardMargin {
                return nil
            }
        }

        let fullyInFrame = tips.allSatisfy {
            $0.x >= softMargin && $0.x <= 1 - softMargin &&
            $0.y >= softMargin && $0.y <= 1 - softMargin
        }

        let palmCenter = CGPoint(
            x: (wrist.x + middleMcp.x) * 0.5,
            y: (wrist.y + middleMcp.y) * 0.5
        )

        let handDx = Float(middleTip.x - wrist.x)
        let handDy = Float(middleTip.y - wrist.y)
        let scale = sqrt(handDx * handDx + handDy * handDy)

        let rollDx = Float(middleMcp.x - wrist.x)
        let rollDy = Float(middleMcp.y - wrist.y)
        let rollDeg = abs(atan2(rollDx, -rollDy) * 180.0 / .pi)

        let widthDx = Float(pinkyMcp.x - indexMcp.x)
        let widthDy = Float(pinkyMcp.y - indexMcp.y)
        let palmWidth = sqrt(widthDx * widthDx + widthDy * widthDy)
        let palmLengthDx = Float(middleMcp.x - wrist.x)
        let palmLengthDy = Float(middleMcp.y - wrist.y)
        let palmLength = sqrt(palmLengthDx * palmLengthDx + palmLengthDy * palmLengthDy)
        let aspect = palmLength > 0.001 ? (palmWidth / (palmLength * 0.75)) : 1.0

        let pose = SubjectPose(
            centroid: palmCenter,
            scale: scale,
            rollDeg: rollDeg,
            aspectRatio: aspect,
            samplePoints: Array(landmarks.prefix(21)),
            confidence: fullyInFrame ? 1 : 0.7
        )
        return HandFraming(pose: pose, isFullyInFrame: fullyInFrame)
    }

    public static func from(_ landmarks: [CGPoint]) -> SubjectPose? {
        framing(from: landmarks)?.pose
    }

    /// Mirrors core/geometry/RoiWarper::detectIsLeftHand — robust thumb vs pinky lateral position.
    public static func isLeftHand(_ landmarks: [CGPoint]) -> Bool {
        guard landmarks.count >= 18 else { return false }
        let p0 = landmarks[0] // wrist
        let p9 = landmarks[9] // middle MCP
        let upX = Float(p9.x - p0.x)
        let upY = Float(p9.y - p0.y)
        let upLen = sqrt(upX*upX + upY*upY)
        guard upLen > 1e-4 else { return false }
        let rightX = -upY / upLen
        let rightY = upX / upLen

        // 1. Primary: Thumb lateral vs Pinky lateral relative to palm centerline (wrist -> middle MCP)
        if landmarks.count >= 21 {
            let p2 = landmarks[2]  // thumb MP
            let p4 = landmarks[4]  // thumb tip
            let p17 = landmarks[17] // pinky MCP
            let p20 = landmarks[20] // pinky tip

            let thumbX = Float((p2.x + p4.x) * 0.5 - p0.x)
            let thumbY = Float((p2.y + p4.y) * 0.5 - p0.y)
            let thumbLateral = thumbX * rightX + thumbY * rightY

            let pinkyX = Float((p17.x + p20.x) * 0.5 - p0.x)
            let pinkyY = Float((p17.y + p20.y) * 0.5 - p0.y)
            let pinkyLateral = pinkyX * rightX + pinkyY * rightY

            // For palm facing camera: Left palm has thumb on the right (thumbLateral > pinkyLateral)
            return thumbLateral > pinkyLateral
        }

        // 2. Fallback: Index MCP (p5) to Pinky MCP (p17) dot product
        let p5 = landmarks[5]
        let p17 = landmarks[17]
        let palmWX = Float(p17.x - p5.x)
        let palmWY = Float(p17.y - p5.y)
        let dot = palmWX * rightX + palmWY * rightY
        return dot < 0
    }
}
