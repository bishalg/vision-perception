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
                // Pitch black canvas for stimulus contrast
                Color.black.ignoresSafeArea()

                // Split-Screen Stimulus Representation (GeoPref)
                HStack(spacing: 2) {
                    // Left: Social scene (children playing)
                    ZStack {
                        LinearGradient(
                            colors: [Color(red: 0.1, green: 0.15, blue: 0.25), Color(red: 0.05, green: 0.08, blue: 0.15)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        VStack(spacing: 8) {
                            Image(systemName: "person.2.fill")
                                .font(.system(size: 48))
                                .foregroundColor(.white.opacity(0.8))
                            Text("Social Scene (Actor)")
                                .font(.system(size: 14, weight: .semibold, design: .rounded))
                                .foregroundColor(.white)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                    // Right: Geometric pattern (dynamic shapes)
                    ZStack {
                        LinearGradient(
                            colors: [Color(red: 0.2, green: 0.1, blue: 0.25), Color(red: 0.1, green: 0.05, blue: 0.15)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        VStack(spacing: 8) {
                            Image(systemName: "circle.hexagongrid.fill")
                                .font(.system(size: 48))
                                .foregroundColor(LiquidGlass.celestialTeal.opacity(0.8))
                            Text("Dynamic Geometric Pattern")
                                .font(.system(size: 14, weight: .semibold, design: .rounded))
                                .foregroundColor(.white)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }

                // Real-time gaze reticle (TrueDepth 60 Hz point)
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
                            Circle()
                                .fill(LiquidGlass.emeraldMint)
                                .frame(width: 10, height: 10)
                            Text("60 Hz TrueDepth")
                                .font(.system(size: 12, weight: .bold, design: .monospaced))
                                .foregroundColor(.white)
                            Text("Dist: \(Int(distanceCm))cm")
                                .font(.system(size: 12, weight: .medium, design: .monospaced))
                                .foregroundColor(Color.white.opacity(0.8))
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
