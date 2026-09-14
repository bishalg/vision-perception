import SwiftUI

/// Animated Liquid Glass splash screen presenting the app icon and title on app launch.
public struct SplashScreenView: View {
    @State private var iconScale: CGFloat = 0.82
    @State private var iconOpacity: Double = 0.0
    @State private var textOpacity: Double = 0.0
    @State private var glowRadius: CGFloat = 8.0

    public let onFinished: () -> Void

    public init(onFinished: @escaping () -> Void) {
        self.onFinished = onFinished
    }

    public var body: some View {
        ZStack {
            // Adaptive Liquid Glass background
            LinearGradient(
                colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 24) {
                Spacer()

                // App Icon with Luminous Liquid Glass Glow
                ZStack {
                    // Outer diffuse aura glow
                    Circle()
                        .fill(
                            RadialGradient(
                                colors: [
                                    LiquidGlass.celestialTeal.opacity(0.35),
                                    LiquidGlass.emeraldMint.opacity(0.12),
                                    Color.clear
                                ],
                                center: .center,
                                startRadius: 20,
                                endRadius: 90
                            )
                        )
                        .frame(width: 180, height: 180)
                        .scaleEffect(iconScale * 1.1)

                    // App Logo Icon
                    Image("AppLogo")
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .frame(width: 112, height: 112)
                        .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 26, style: .continuous)
                                .stroke(LiquidGlass.cardBorder, lineWidth: 1.2)
                        )
                        .shadow(color: LiquidGlass.celestialTeal.opacity(0.4), radius: glowRadius, x: 0, y: 4)
                }
                .scaleEffect(iconScale)
                .opacity(iconOpacity)

                // Title & Subtitle Branding
                VStack(spacing: 6) {
                    Text("ASD Gaze")
                        .font(.system(size: 26, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.textPrimary)

                    Text("Pediatric Vision & Attention Perception")
                        .font(.system(size: 13, weight: .medium, design: .rounded))
                        .foregroundColor(LiquidGlass.textSecondary)
                }
                .opacity(textOpacity)

                Spacer()

                // Subtle activity indicator
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle(tint: LiquidGlass.celestialTeal))
                    .scaleEffect(0.9)
                    .opacity(textOpacity)
                    .padding(.bottom, 36)
            }
        }
        .onAppear {
            withAnimation(.spring(response: 0.65, dampingFraction: 0.72)) {
                iconScale = 1.0
                iconOpacity = 1.0
                glowRadius = 18.0
            }

            withAnimation(.easeOut(duration: 0.45).delay(0.25)) {
                textOpacity = 1.0
            }

            // Transition into main experience after brief branded introduction
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.4) {
                withAnimation(.easeInOut(duration: 0.35)) {
                    onFinished()
                }
            }
        }
    }
}
