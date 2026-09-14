import SwiftUI

// MARK: - Procedural Oculomotor Stimulus Modes

/// Specific clinical oculomotor testing paradigms generated entirely in code.
public enum ProceduralStimulusMode: String, CaseIterable, Sendable {
    /// Smooth continuous circular or sinusoidal target movement for pursuit gain.
    case smoothPursuit = "Smooth Pursuit"
    /// Instantaneous discrete target jumps for saccadic latency measurement.
    case saccadicStep = "Saccadic Step"
    /// High-contrast moving grating bars to elicit optokinetic tracking.
    case contrastGratings = "Contrast Gratings"
    /// Mathematical rotating kaleidoscope & nested polygons for GeoPref dynamic contrast.
    case geometricKaleidoscope = "Geometric Kaleidoscope"
}

// MARK: - Procedural Coordinate Mathematics

/// Pure mathematical generator of ground-truth target coordinates $(x(t), y(t)) \in [0.0, 1.0]$.
public struct ProceduralOculomotorMath: Sendable {

    /// Computes the exact normalized ground truth target coordinate $(x, y)$ at timestamp `timeSec`.
    public static func targetCoordinate(
        mode: ProceduralStimulusMode,
        timeSec: Double
    ) -> CGPoint {
        switch mode {
        case .smoothPursuit:
            // Smooth circular/elliptical orbit with radius 0.32 centered at (0.5, 0.5)
            // Period = 4.0 seconds (angular velocity 90 deg/sec)
            let period = 4.0
            let angle = (timeSec.truncatingRemainder(dividingBy: period) / period) * 2.0 * .pi
            let x = 0.5 + 0.32 * cos(angle)
            let y = 0.5 + 0.28 * sin(angle)
            return CGPoint(x: x, y: y)

        case .saccadicStep:
            // Step jump every 1.5 seconds across 5 canonical screen anchor points
            let stepInterval = 1.5
            let stepIndex = Int(timeSec / stepInterval) % 5
            switch stepIndex {
            case 0: return CGPoint(x: 0.50, y: 0.50) // Center
            case 1: return CGPoint(x: 0.18, y: 0.30) // Top-Left
            case 2: return CGPoint(x: 0.82, y: 0.30) // Top-Right
            case 3: return CGPoint(x: 0.82, y: 0.70) // Bottom-Right
            case 4: return CGPoint(x: 0.18, y: 0.70) // Bottom-Left
            default: return CGPoint(x: 0.50, y: 0.50)
            }

        case .contrastGratings:
            // Horizontal drifting phase [0.0, 1.0]
            let phase = (timeSec * 0.5).truncatingRemainder(dividingBy: 1.0)
            return CGPoint(x: phase, y: 0.5)

        case .geometricKaleidoscope:
            // Center of the kaleidoscope pattern
            return CGPoint(x: 0.50, y: 0.50)
        }
    }

    /// Evaluates whether a gaze point $(x, y)$ is within the target area of interest at `timeSec`.
    public static func isGazeOnTarget(
        gazePoint: CGPoint,
        mode: ProceduralStimulusMode,
        timeSec: Double,
        hitRadius: CGFloat = 0.12
    ) -> Bool {
        let target = targetCoordinate(mode: mode, timeSec: timeSec)
        let dx = gazePoint.x - target.x
        let dy = gazePoint.y - target.y
        let dist = sqrt(dx * dx + dy * dy)
        return dist <= hitRadius
    }
}

// MARK: - SwiftUI Procedural Stimulus View

/// Renders real-time, 60 Hz code-driven oculomotor and geometric stimuli.
/// Provides pure mathematical ground-truth for smooth pursuit, saccade latencies,
/// and customizable color-contrast patterns.
public struct ProceduralOculomotorStimulusView: View {
    public let mode: ProceduralStimulusMode
    public let elapsedSec: Double
    public let primaryColor: Color
    public let secondaryColor: Color

    public init(
        mode: ProceduralStimulusMode = .smoothPursuit,
        elapsedSec: Double,
        primaryColor: Color = Color(red: 0.0, green: 0.824, blue: 0.769), // celestialTeal
        secondaryColor: Color = Color(red: 1.0, green: 0.718, blue: 0.302) // warmAmber
    ) {
        self.mode = mode
        self.elapsedSec = elapsedSec
        self.primaryColor = primaryColor
        self.secondaryColor = secondaryColor
    }

    public var body: some View {
        GeometryReader { geo in
            let targetPoint = ProceduralOculomotorMath.targetCoordinate(mode: mode, timeSec: elapsedSec)
            let pixelX = targetPoint.x * geo.size.width
            let pixelY = targetPoint.y * geo.size.height

            ZStack {
                // Background canvas
                Color.black.ignoresSafeArea()

                switch mode {
                case .smoothPursuit:
                    renderSmoothPursuitTarget(pixelX: pixelX, pixelY: pixelY)

                case .saccadicStep:
                    renderSaccadicStepTarget(pixelX: pixelX, pixelY: pixelY)

                case .contrastGratings:
                    renderContrastGratings(width: geo.size.width, height: geo.size.height)

                case .geometricKaleidoscope:
                    renderGeometricKaleidoscope(center: CGPoint(x: geo.size.width / 2, y: geo.size.height / 2))
                }
            }
        }
    }

    // MARK: - Subviews

    @ViewBuilder
    private func renderSmoothPursuitTarget(pixelX: CGFloat, pixelY: CGFloat) -> some View {
        ZStack {
            // Soft pulsating halo
            Circle()
                .fill(
                    RadialGradient(
                        colors: [primaryColor.opacity(0.6), primaryColor.opacity(0.0)],
                        center: .center,
                        startRadius: 10,
                        endRadius: 42
                    )
                )
                .frame(width: 84, height: 84)

            // Dynamic rotating star target (appealing to toddlers)
            Image(systemName: "sparkle")
                .font(.system(size: 38, weight: .bold))
                .foregroundColor(secondaryColor)
                .rotationEffect(.degrees(elapsedSec * 120.0))

            // High-contrast center bead
            Circle()
                .fill(Color.white)
                .frame(width: 14, height: 14)
                .shadow(color: .white, radius: 4)
        }
        .position(x: pixelX, y: pixelY)
    }

    @ViewBuilder
    private func renderSaccadicStepTarget(pixelX: CGFloat, pixelY: CGFloat) -> some View {
        ZStack {
            // High-contrast bullseye target
            Circle()
                .stroke(secondaryColor, lineWidth: 4)
                .frame(width: 56, height: 56)

            Circle()
                .fill(primaryColor)
                .frame(width: 32, height: 32)

            Circle()
                .fill(Color.white)
                .frame(width: 12, height: 12)
        }
        .position(x: pixelX, y: pixelY)
    }

    @ViewBuilder
    private func renderContrastGratings(width: CGFloat, height: CGFloat) -> some View {
        Canvas { context, size in
            let barWidth: CGFloat = 36.0
            let offset = CGFloat((elapsedSec * 60.0).truncatingRemainder(dividingBy: Double(barWidth * 2)))
            var currentX: CGFloat = -barWidth * 2 + offset

            while currentX < size.width + barWidth * 2 {
                let rect = CGRect(x: currentX, y: 0, width: barWidth, height: size.height)
                context.fill(Path(rect), with: .color(primaryColor.opacity(0.85)))
                currentX += barWidth * 2
            }
        }
    }

    @ViewBuilder
    private func renderGeometricKaleidoscope(center: CGPoint) -> some View {
        ZStack {
            ForEach(0..<6) { i in
                let rotation = Double(i) * 60.0 + elapsedSec * 35.0
                let scale = 0.8 + 0.2 * sin(elapsedSec * 2.0 + Double(i))

                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(
                        i % 2 == 0 ? primaryColor : secondaryColor,
                        lineWidth: 3
                    )
                    .frame(width: 160 * scale, height: 160 * scale)
                    .rotationEffect(.degrees(rotation))
            }

            Circle()
                .fill(
                    RadialGradient(
                        colors: [secondaryColor, primaryColor.opacity(0.4)],
                        center: .center,
                        startRadius: 5,
                        endRadius: 40
                    )
                )
                .frame(width: 50, height: 50)
                .rotationEffect(.degrees(-elapsedSec * 90))
        }
        .position(x: center.x, y: center.y)
    }
}
