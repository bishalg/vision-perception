import CoreGraphics
import CoreVideo
import Foundation

/**
 * On-device Swift computer vision classifier for dermatoglyphic patterns.
 * Analyzes epidermal ridge orientation flow, singular points (cores/deltas),
 * and spatial ridge frequency from the finger pad ROI.
 *
 * Privacy-preserving: extracts pattern category (Arch, Loop, Whorl),
 * ridge frequency, and quality score only; never stores biometric minutiae templates.
 */
public enum DermatoglyphicClassifier {

    public static func classify(
        pixelBuffer: CVPixelBuffer,
        landmarks: [CGPoint],
        digit: FingertipDigit
    ) -> FingertipScanResult {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        guard width > 64, height > 64,
              let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            return fallbackResult(digit: digit)
        }

        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let pixelFormat = CVPixelBufferGetPixelFormatType(pixelBuffer)

        // 1. Determine finger pad center in image coordinates
        let padCenter: CGPoint
        if landmarks.count > digit.tipIndex {
            let tip = landmarks[digit.tipIndex]
            let dipIndex = digit.dipIndex
            if landmarks.count > dipIndex {
                let dip = landmarks[dipIndex]
                // The fleshy pad is located between tip and DIP, ~35% towards the DIP joint
                padCenter = CGPoint(
                    x: tip.x * 0.65 + dip.x * 0.35,
                    y: tip.y * 0.65 + dip.y * 0.35
                )
            } else {
                padCenter = CGPoint(x: tip.x, y: min(1.0, tip.y + 0.04))
            }
        } else {
            // Default center region when macro close-up obscures whole hand
            padCenter = CGPoint(x: 0.5, y: 0.48)
        }

        let centerX = Int(padCenter.x * CGFloat(width))
        let centerY = Int(padCenter.y * CGFloat(height))

        // Crop ROI of size 192x192 pixels around the pad
        let roiRadius = min(96, min(width, height) / 4)
        let minX = max(1, centerX - roiRadius)
        let maxX = min(width - 2, centerX + roiRadius)
        let minY = max(1, centerY - roiRadius)
        let maxY = min(height - 2, centerY + roiRadius)

        let roiW = maxX - minX
        let roiH = maxY - minY
        guard roiW >= 64, roiH >= 64 else {
            return fallbackResult(digit: digit)
        }

        // 2. Extract grayscale luminance ROI
        var gray = [Float](repeating: 0, count: roiW * roiH)
        let rawPtr = baseAddress.assumingMemoryBound(to: UInt8.self)

        for y in 0..<roiH {
            let srcY = minY + y
            let rowStart = srcY * bytesPerRow
            for x in 0..<roiW {
                let srcX = minX + x
                let lum: Float
                if pixelFormat == kCVPixelFormatType_32BGRA {
                    let offset = rowStart + srcX * 4
                    let b = Float(rawPtr[offset])
                    let g = Float(rawPtr[offset + 1])
                    let r = Float(rawPtr[offset + 2])
                    lum = 0.299 * r + 0.587 * g + 0.114 * b
                } else {
                    // Y / Gray / 420YpCbCr
                    lum = Float(rawPtr[rowStart + srcX])
                }
                gray[y * roiW + x] = lum
            }
        }

        // Contrast stretch / local normalization
        var minLum: Float = 255
        var maxLum: Float = 0
        for val in gray {
            if val < minLum { minLum = val }
            if val > maxLum { maxLum = val }
        }
        let range = max(1.0, maxLum - minLum)
        for i in 0..<gray.count {
            gray[i] = (gray[i] - minLum) / range
        }

        // 3. Compute horizontal and vertical gradients using Sobel operators
        var gradX = [Float](repeating: 0, count: roiW * roiH)
        var gradY = [Float](repeating: 0, count: roiW * roiH)

        for y in 1..<(roiH - 1) {
            for x in 1..<(roiW - 1) {
                let p00 = gray[(y - 1) * roiW + (x - 1)]
                let p01 = gray[(y - 1) * roiW + x]
                let p02 = gray[(y - 1) * roiW + (x + 1)]
                let p10 = gray[y * roiW + (x - 1)]
                let p12 = gray[y * roiW + (x + 1)]
                let p20 = gray[(y + 1) * roiW + (x - 1)]
                let p21 = gray[(y + 1) * roiW + x]
                let p22 = gray[(y + 1) * roiW + (x + 1)]

                let gx = (p02 + 2 * p12 + p22) - (p00 + 2 * p10 + p20)
                let gy = (p20 + 2 * p21 + p22) - (p00 + 2 * p01 + p02)
                gradX[y * roiW + x] = gx
                gradY[y * roiW + x] = gy
            }
        }

        // 4. Block-based orientation field and coherence tensor (16x16 blocks)
        let blockSize = 16
        let gridW = roiW / blockSize
        let gridH = roiH / blockSize
        guard gridW >= 3, gridH >= 3 else {
            return fallbackResult(digit: digit)
        }

        var blockTheta = [Float](repeating: 0, count: gridW * gridH)
        var blockCoherence = [Float](repeating: 0, count: gridW * gridH)
        var totalCoherence: Float = 0

        for by in 0..<gridH {
            for bx in 0..<gridW {
                var sumGxx: Float = 0
                var sumGyy: Float = 0
                var sumGxy: Float = 0

                for py in (by * blockSize)..<((by + 1) * blockSize) {
                    for px in (bx * blockSize)..<((bx + 1) * blockSize) {
                        let gx = gradX[py * roiW + px]
                        let gy = gradY[py * roiW + px]
                        sumGxx += gx * gx
                        sumGyy += gy * gy
                        sumGxy += gx * gy
                    }
                }

                let vX = 2 * sumGxy
                let vY = sumGxx - sumGyy
                let mag = sqrt(vX * vX + vY * vY)
                let denom = sumGxx + sumGyy + 1e-4
                let coherence = mag / denom

                // Ridge flow orientation: perpendicular to gradient
                let theta = 0.5 * atan2(vX, vY)
                let idx = by * gridW + bx
                blockTheta[idx] = theta
                blockCoherence[idx] = coherence
                totalCoherence += coherence
            }
        }

        let meanCoherence = totalCoherence / Float(gridW * gridH)
        let quality = min(0.96, max(0.35, meanCoherence * 1.5 + (range / 255.0) * 0.4))

        // 5. Dermatoglyphic pattern classification from orientation field
        // Center block coordinates:
        let centerBx = Float(gridW) * 0.5
        let centerBy = Float(gridH) * 0.5

        var circularFlowSum: Float = 0
        var loopCurvatureCount: Int = 0
        var horizontalCount: Int = 0
        var evaluatedBlocks: Int = 0

        for by in 0..<gridH {
            for bx in 0..<gridW {
                let idx = by * gridW + bx
                guard blockCoherence[idx] > 0.15 else { continue }
                evaluatedBlocks += 1

                let dx = Float(bx) - centerBx
                let dy = Float(by) - centerBy
                let dist = sqrt(dx * dx + dy * dy)
                if dist > 0.5 && dist <= Float(gridW) * 0.45 {
                    // Tangential angle around center:
                    let tangentAngle = atan2(dy, dx) + Float.pi / 2.0
                    let theta = blockTheta[idx]
                    // Ridge orientation difference vs concentric tangent
                    let diff = abs(sin(theta - tangentAngle))
                    circularFlowSum += (1.0 - diff)
                }

                let angle = blockTheta[idx]
                // Flat horizontal ridges (Arch characteristic):
                if abs(sin(angle)) < 0.35 {
                    horizontalCount += 1
                }
                // High vertical curve / loop turnaround:
                if abs(cos(angle)) < 0.40 {
                    loopCurvatureCount += 1
                }
            }
        }

        let validBlocks = max(1, evaluatedBlocks)
        let circularScore = circularFlowSum / Float(validBlocks)
        let horizontalRatio = Float(horizontalCount) / Float(validBlocks)
        let loopRatio = Float(loopCurvatureCount) / Float(validBlocks)

        let pattern: DermatoglyphicPattern
        if circularScore > 0.38 {
            pattern = .whorl
        } else if loopRatio > 0.22 || (circularScore > 0.22 && horizontalRatio < 0.65) {
            pattern = .loop
        } else if horizontalRatio > 0.50 {
            pattern = .arch
        } else {
            pattern = .loop
        }

        // 6. Ridge spatial frequency estimation (bandpass zero-crossing spacing)
        var ridgeSpacings: [Float] = []
        let midY = roiH / 2
        for offset in [-12, 0, 12] {
            let row = midY + offset
            guard row >= 0, row < roiH else { continue }
            var lastPeak = -1
            for x in 1..<(roiW - 1) {
                let prev = gray[row * roiW + (x - 1)]
                let curr = gray[row * roiW + x]
                let next = gray[row * roiW + (x + 1)]
                if curr > prev && curr > next && curr > 0.5 {
                    if lastPeak >= 0 {
                        let spacing = Float(x - lastPeak)
                        if spacing >= 4 && spacing <= 25 {
                            ridgeSpacings.append(spacing)
                        }
                    }
                    lastPeak = x
                }
            }
        }

        let meanSpacingPx: Float = ridgeSpacings.isEmpty
            ? 10.0
            : (ridgeSpacings.reduce(0, +) / Float(ridgeSpacings.count))
        // Typical adult epidermal ridge frequency: ~0.8 to 1.1 mm^-1
        let ridgeFrequency = max(0.4, min(1.4, 8.5 / meanSpacingPx))

        return FingertipScanResult(
            digit: digit,
            pattern: pattern,
            ridgeFrequency: (ridgeFrequency * 100).rounded() / 100,
            quality: (quality * 100).rounded() / 100,
            timestamp: Date().timeIntervalSince1970
        )
    }

    private static func fallbackResult(digit: FingertipDigit) -> FingertipScanResult {
        FingertipScanResult(
            digit: digit,
            pattern: .loop,
            ridgeFrequency: 0.85,
            quality: 0.5,
            timestamp: Date().timeIntervalSince1970
        )
    }
}
