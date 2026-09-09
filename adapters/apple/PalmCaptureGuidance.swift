import Foundation
import CoreGraphics
import CoreVideo

/**
 * Configuration thresholds for real-time palm capture guidance.
 * Kept strictly synchronized with Android PalmCaptureGuidance.kt.
 */
public struct PalmGuidanceThresholds {
    public var minPalmScale: Float = 0.35          // Palm length / frame height
    public var maxPalmScale: Float = 0.60
    public var maxCenterOffset: Float = 0.18        // Max distance from frame center (0.5, 0.5)
    public var maxRollAngleDeg: Float = 22.0        // Max roll tilt relative to vertical
    public var minFlatnessRatio: Float = 0.70       // Width / height ratio proxy for planar tilt
    public var minLuminance: Float = 70.0          // Minimum average skin luminance (0..255)
    public var maxGlareRatio: Float = 0.04         // Max percentage of specular glare pixels (> 245)
    public var maxMotionDisplacement: Float = 0.025 // Max EMA landmark drift across frames
    public var steadyDurationSec: TimeInterval = 0.30 // Time in ready state before auto-trigger

    public init() {}
}

/**
 * Real-time guidance states for rendering Liquid Glass HUD cues.
 */
public enum PalmGuidanceState: Equatable {
    case searching(message: String)
    case centerHand(message: String)
    case moveCloser(message: String)
    case moveBack(message: String)
    case holdFlat(message: String)
    case tooDark(message: String)
    case glareDetected(message: String)
    case holdSteady(message: String)
    case ready(countdownProgress: Float, message: String)

    public var isReady: Bool {
        if case .ready = self { return true }
        return false
    }

    public var message: String {
        switch self {
        case .searching(let m),
             .centerHand(let m),
             .moveCloser(let m),
             .moveBack(let m),
             .holdFlat(let m),
             .tooDark(let m),
             .glareDetected(let m),
             .holdSteady(let m),
             .ready(_, let m):
            return m
        }
    }
}

/**
 * Diagnostic metrics emitted every frame for HUD telemetry.
 */
public struct PalmQualityMetrics {
    public var scale: Float = 0.0
    public var centerOffset: Float = 0.0
    public var rollAngleDeg: Float = 0.0
    public var flatnessRatio: Float = 0.0
    public var meanLuminance: Float = 0.0
    public var glareRatio: Float = 0.0
    public var motionDisplacement: Float = 0.0

    public var isCenteringOk: Bool = false
    public var isScaleOk: Bool = false
    public var isAngleOk: Bool = false
    public var isIlluminationOk: Bool = false
    public var isSteadinessOk: Bool = false
    public var isReadyForCapture: Bool = false

    public init() {}
}

/**
 * High-performance 60 FPS real-time capture guidance evaluator.
 */
public final class PalmCaptureGuidance {
    public var thresholds: PalmGuidanceThresholds
    private var previousLandmarks: [CGPoint] = []
    private var steadyStartTime: TimeInterval?

    public init(thresholds: PalmGuidanceThresholds = PalmGuidanceThresholds()) {
        self.thresholds = thresholds
    }

    public func reset() {
        previousLandmarks.removeAll()
        steadyStartTime = nil
    }

    /**
     * Evaluates a frame at 60 FPS using 21 normalized landmarks (0..1 coordinates).
     */
    public func evaluate(
        landmarks: [CGPoint],
        pixelBuffer: CVPixelBuffer?,
        timestamp: TimeInterval
    ) -> (PalmGuidanceState, PalmQualityMetrics) {
        var metrics = PalmQualityMetrics()

        guard landmarks.count >= 21 else {
            steadyStartTime = nil
            previousLandmarks.removeAll()
            return (.searching(message: "Place hand inside the outline"), metrics)
        }

        // Key anatomical landmarks (MediaPipe / Apple Vision topology)
        let wrist = landmarks[0]
        let indexMcp = landmarks[5]
        let middleMcp = landmarks[9]
        let pinkyMcp = landmarks[17]

        // 1. Centering Check
        let palmCenter = CGPoint(
            x: (wrist.x + middleMcp.x) * 0.5,
            y: (wrist.y + middleMcp.y) * 0.5
        )
        let centerDx = Float(palmCenter.x - 0.5)
        let centerDy = Float(palmCenter.y - 0.5)
        metrics.centerOffset = sqrt(centerDx * centerDx + centerDy * centerDy)
        metrics.isCenteringOk = metrics.centerOffset <= thresholds.maxCenterOffset

        // 2. Scale / Distance Check
        let palmLengthDx = Float(middleMcp.x - wrist.x)
        let palmLengthDy = Float(middleMcp.y - wrist.y)
        metrics.scale = sqrt(palmLengthDx * palmLengthDx + palmLengthDy * palmLengthDy)
        metrics.isScaleOk = (metrics.scale >= thresholds.minPalmScale && metrics.scale <= thresholds.maxPalmScale)

        // 3. Angle & Planar Flatness Check
        // Roll angle relative to vertical axis
        let rollRad = atan2(palmLengthDx, -palmLengthDy) // Y inverted in screen coords
        metrics.rollAngleDeg = abs(rollRad * 180.0 / .pi)

        let palmWidthDx = Float(pinkyMcp.x - indexMcp.x)
        let palmWidthDy = Float(pinkyMcp.y - indexMcp.y)
        let palmWidth = sqrt(palmWidthDx * palmWidthDx + palmWidthDy * palmWidthDy)
        metrics.flatnessRatio = (metrics.scale > 0.001) ? (palmWidth / (metrics.scale * 0.75)) : 1.0

        metrics.isAngleOk = (metrics.rollAngleDeg <= thresholds.maxRollAngleDeg) && (metrics.flatnessRatio >= thresholds.minFlatnessRatio)

        // 4. Illumination & Glare Check
        if let pb = pixelBuffer {
            let (lum, glare) = evaluatePhotometrics(pixelBuffer: pb, palmCenter: palmCenter, radius: metrics.scale * 0.3)
            metrics.meanLuminance = lum
            metrics.glareRatio = glare
        } else {
            metrics.meanLuminance = 128.0
            metrics.glareRatio = 0.0
        }
        metrics.isIlluminationOk = (metrics.meanLuminance >= thresholds.minLuminance) && (metrics.glareRatio <= thresholds.maxGlareRatio)

        // 5. Motion Steadiness Check
        if previousLandmarks.count >= 21 {
            var sumDist: Float = 0.0
            for i in 0..<21 {
                let dx = Float(landmarks[i].x - previousLandmarks[i].x)
                let dy = Float(landmarks[i].y - previousLandmarks[i].y)
                sumDist += sqrt(dx * dx + dy * dy)
            }
            metrics.motionDisplacement = sumDist / 21.0
        } else {
            metrics.motionDisplacement = 0.0
        }
        previousLandmarks = landmarks
        metrics.isSteadinessOk = metrics.motionDisplacement <= thresholds.maxMotionDisplacement

        // Evaluate overall state with prioritized feedback
        if !metrics.isCenteringOk {
            steadyStartTime = nil
            return (.centerHand(message: "Center your palm in the guide"), metrics)
        }
        if metrics.scale < thresholds.minPalmScale {
            steadyStartTime = nil
            return (.moveCloser(message: "Move closer to the camera"), metrics)
        }
        if metrics.scale > thresholds.maxPalmScale {
            steadyStartTime = nil
            return (.moveBack(message: "Move slightly further back"), metrics)
        }
        if !metrics.isAngleOk {
            steadyStartTime = nil
            return (.holdFlat(message: "Hold palm flat facing the camera"), metrics)
        }
        if metrics.meanLuminance < thresholds.minLuminance {
            steadyStartTime = nil
            return (.tooDark(message: "Too dark — increase lighting"), metrics)
        }
        if metrics.glareRatio > thresholds.maxGlareRatio {
            steadyStartTime = nil
            return (.glareDetected(message: "Glare on skin — angle away from light"), metrics)
        }
        if !metrics.isSteadinessOk {
            steadyStartTime = nil
            return (.holdSteady(message: "Hold steady..."), metrics)
        }

        // All 5 gates PASS: Start or advance steady countdown
        if let start = steadyStartTime {
            let elapsed = timestamp - start
            let progress = Float(min(1.0, elapsed / thresholds.steadyDurationSec))
            metrics.isReadyForCapture = (progress >= 1.0)
            return (.ready(countdownProgress: progress, message: progress >= 1.0 ? "Capturing..." : "Hold steady..."), metrics)
        } else {
            steadyStartTime = timestamp
            return (.ready(countdownProgress: 0.0, message: "Hold steady..."), metrics)
        }
    }

    /**
     * Fast sub-sampled photometric luminance & specular glare evaluation on CVPixelBuffer.
     */
    private func evaluatePhotometrics(
        pixelBuffer: CVPixelBuffer,
        palmCenter: CGPoint,
        radius: Float
    ) -> (luminance: Float, glareRatio: Float) {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return (128.0, 0.0)
        }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)

        let cx = Int(palmCenter.x * CGFloat(width))
        let cy = Int(palmCenter.y * CGFloat(height))
        let rPx = Int(CGFloat(radius) * CGFloat(min(width, height)))

        let x0 = max(0, cx - rPx)
        let x1 = min(width - 1, cx + rPx)
        let y0 = max(0, cy - rPx)
        let y1 = min(height - 1, cy + rPx)

        if x1 <= x0 || y1 <= y0 { return (128.0, 0.0) }

        var totalLum: Float = 0.0
        var glareCount: Int = 0
        var sampleCount: Int = 0

        // Subsample grid (step by 4 for 60 FPS efficiency)
        let step = 4
        for y in stride(from: y0, through: y1, by: step) {
            let rowOffset = y * bytesPerRow
            for x in stride(from: x0, through: x1, by: step) {
                let pixelOffset = rowOffset + x * 4 // BGRA
                let b = Float(ptr[pixelOffset])
                let g = Float(ptr[pixelOffset + 1])
                let r = Float(ptr[pixelOffset + 2])

                // Standard Rec. 601 Luma
                let yVal = 0.299 * r + 0.587 * g + 0.114 * b
                totalLum += yVal
                if yVal > 245.0 { glareCount += 1 }
                sampleCount += 1
            }
        }

        if sampleCount == 0 { return (128.0, 0.0) }
        let avgLum = totalLum / Float(sampleCount)
        let glareRatio = Float(glareCount) / Float(sampleCount)
        return (avgLum, glareRatio)
    }
}
