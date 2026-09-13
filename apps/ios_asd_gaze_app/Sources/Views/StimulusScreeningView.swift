import SwiftUI
import CurvAsdGazeKit

/// View coordinating stimulus presentation and real-time gaze overlay HUD.
public struct StimulusScreeningView: View {
    @State private var gazePointNorm = CGPoint(x: 0.35, y: 0.45)
    @State private var distanceCm: Double = 46.0
    @State private var currentSec: Double = 0.0
    @State private var timerActive = true
    @State private var activeAOI: String = "Social_Actor"

    public let onFinishSession: () -> Void

    private let tracker = StimulusAOITracker.makeGeoPrefBattery()

    public init(onFinishSession: @escaping () -> Void) {
        self.onFinishSession = onFinishSession
    }

    public var body: some View {
        GeometryReader { geometry in
            ZStack {
                // Adaptive Liquid Glass canvas (dark twilight in dark mode, airy porcelain in light mode)
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .ignoresSafeArea()

                // Split-Screen Stimulus Representation (GeoPref)
                HStack(spacing: 2) {
                    // Left: Social scene (children playing)
                    ZStack {
                        LinearGradient(
                            colors: [LiquidGlass.stimulusSocialTop, LiquidGlass.stimulusSocialBottom],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        VStack(spacing: 8) {
                            Image(systemName: "person.2.fill")
                                .font(.system(size: 48))
                                .foregroundColor(LiquidGlass.celestialTeal)
                            Text("Social Scene (Actor)")
                                .font(.system(size: 14, weight: .semibold, design: .rounded))
                                .foregroundColor(LiquidGlass.textPrimary)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                    // Right: Geometric pattern (dynamic shapes)
                    ZStack {
                        LinearGradient(
                            colors: [LiquidGlass.stimulusGeometryTop, LiquidGlass.stimulusGeometryBottom],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        VStack(spacing: 8) {
                            Image(systemName: "circle.hexagongrid.fill")
                                .font(.system(size: 48))
                                .foregroundColor(LiquidGlass.celestialTeal.opacity(0.8))
                            Text("Dynamic Geometric Pattern")
                                .font(.system(size: 14, weight: .semibold, design: .rounded))
                                .foregroundColor(LiquidGlass.textPrimary)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }

                // Real-time gaze reticle (TrueDepth point)
                Circle()
                    .stroke(LiquidGlass.celestialTeal, lineWidth: 3)
                    .background(Circle().fill(LiquidGlass.celestialTeal.opacity(0.3)))
                    .frame(width: 32, height: 32)
                    .position(
                        x: gazePointNorm.x * geometry.size.width,
                        y: gazePointNorm.y * geometry.size.height
                    )
                    .shadow(color: LiquidGlass.celestialTeal, radius: 8)

                // Top Quality Gate & AOI HUD
                VStack {
                    HStack {
                        HStack(spacing: 12) {
                            HStack(spacing: 5) {
                                Image(systemName: "eye.fill")
                                    .font(.system(size: 11, weight: .bold))
                                    .foregroundColor(LiquidGlass.emeraldMint)
                                Text("Tracking")
                                    .font(.system(size: 12, weight: .semibold, design: .rounded))
                                    .foregroundColor(LiquidGlass.textPrimary)
                            }
                            Text("Dist: \(Int(distanceCm))cm")
                                .font(.system(size: 12, weight: .medium, design: .monospaced))
                                .foregroundColor(LiquidGlass.textSecondary)
                            Text("AOI: \(activeAOI)")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }
                        .glassCard(cornerRadius: 12, padding: 10)

                        Spacer()

                        Button(action: onFinishSession) {
                            Text("Complete")
                                .font(.system(size: 13, weight: .semibold))
                                .padding(.horizontal, 14)
                                .padding(.vertical, 8)
                                .background(Capsule().fill(Color.red.opacity(0.8)))
                                .foregroundColor(.white)
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.top, 10)

                    Spacer()
                }
            }
        }
    }
}
