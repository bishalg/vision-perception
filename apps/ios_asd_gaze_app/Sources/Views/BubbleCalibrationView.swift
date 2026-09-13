import SwiftUI
import CurvAsdGazeKit

/// Interactive 5-point child-friendly popping bubble calibration view.
public struct BubbleCalibrationView: View {
    @State private var currentTargetIndex: Int = 0
    @State private var bubbleScale: CGFloat = 0.8
    @State private var isPopping: Bool = false
    @State private var fixatedTimeMs: Double = 0.0

    public let onCalibrationComplete: () -> Void

    // 5 anchor points in normalized coordinates [x, y]
    private let targetPoints: [CGPoint] = [
        CGPoint(x: 0.50, y: 0.50), // 1. Center
        CGPoint(x: 0.20, y: 0.20), // 2. Top-Left
        CGPoint(x: 0.80, y: 0.20), // 3. Top-Right
        CGPoint(x: 0.20, y: 0.80), // 4. Bottom-Left
        CGPoint(x: 0.80, y: 0.80)  // 5. Bottom-Right
    ]

    public init(onCalibrationComplete: @escaping () -> Void) {
        self.onCalibrationComplete = onCalibrationComplete
    }

    public var body: some View {
        GeometryReader { geometry in
            ZStack {
                // Pitch black background for zero glare/reflection
                Color.black.ignoresSafeArea()

                // Header guidance HUD for caregiver
                VStack {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(Localizer.tr("calibrate_title"))
                                .font(.system(size: 18, weight: .bold, design: .rounded))
                                .foregroundColor(.white)

                            Text(Localizer.tr("calibrate_instruction"))
                                .font(.system(size: 12))
                                .foregroundColor(Color.white.opacity(0.7))
                        }
                        Spacer()

                        // Step counter badge
                        Text("\(currentTargetIndex + 1) / \(targetPoints.count)")
                            .font(.system(size: 14, weight: .bold, design: .monospaced))
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(Capsule().fill(LiquidGlass.cardBackground))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }
                    .glassCard(cornerRadius: 12, padding: 12)
                    .padding(.horizontal, 16)
                    .padding(.top, 10)

                    Spacer()
                }

                // Active popping bubble target
                if currentTargetIndex < targetPoints.count {
                    let norm = targetPoints[currentTargetIndex]
                    let targetX = norm.x * geometry.size.width
                    let targetY = norm.y * geometry.size.height

                    ZStack {
                        // Outer pulsing halo
                        Circle()
                            .fill(LiquidGlass.celestialTeal.opacity(0.25))
                            .frame(width: isPopping ? 140 : 85 * bubbleScale, height: isPopping ? 140 : 85 * bubbleScale)

                        // Core glossy bubble
                        Circle()
                            .fill(
                                RadialGradient(
                                    colors: [Color.white, LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                                    center: .topLeading,
                                    startRadius: 5,
                                    endRadius: 40
                                )
                            )
                            .frame(width: isPopping ? 0 : 60, height: isPopping ? 0 : 60)
                            .shadow(color: LiquidGlass.celestialTeal.opacity(0.8), radius: 12)
                            .opacity(isPopping ? 0 : 1.0)
                    }
                    .position(x: targetX, y: targetY)
                    .onTapGesture {
                        triggerBubblePop()
                    }
                }
            }
            .onAppear {
                startPulsingAnimation()
            }
        }
    }

    private func startPulsingAnimation() {
        withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
            bubbleScale = 1.15
        }
    }

    private func triggerBubblePop() {
        withAnimation(.easeOut(duration: 0.25)) {
            isPopping = true
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            isPopping = false
            if currentTargetIndex + 1 < targetPoints.count {
                currentTargetIndex += 1
            } else {
                onCalibrationComplete()
            }
        }
    }
}
