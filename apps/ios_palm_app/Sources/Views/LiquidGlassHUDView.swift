import SwiftUI

/**
 * Liquid Glass HUD overlay displaying the palm framing guide,
 * real-time telemetry pills, and glowing emerald auto-capture aura.
 */
public struct LiquidGlassHUDView: View {
    public let guidanceState: PalmGuidanceState
    public let metrics: PalmQualityMetrics
    public let onManualCapture: () -> Void

    public init(
        guidanceState: PalmGuidanceState,
        metrics: PalmQualityMetrics,
        onManualCapture: @escaping () -> Void
    ) {
        self.guidanceState = guidanceState
        self.metrics = metrics
        self.onManualCapture = onManualCapture
    }

    private var isReady: Bool {
        if case .ready = guidanceState { return true }
        return false
    }

    private var statusColor: Color {
        if isReady {
            return Color(red: 0.20, green: 0.84, blue: 0.29) // Emerald Green (#32D74B)
        }
        return Color(red: 1.00, green: 0.80, blue: 0.00) // Amber Yellow (#FFCC00)
    }

    public var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let height = proxy.size.height

            ZStack {
                // Dimmed surrounding vignette outside target frame
                Color.black.opacity(0.35)
                    .mask(
                        Rectangle()
                            .overlay(
                                RoundedRectangle(cornerRadius: 36, style: .continuous)
                                    .frame(width: width * 0.76, height: height * 0.58)
                                    .blendMode(.destinationOut)
                            )
                    )
                    .ignoresSafeArea()

                // Palm Framing Reticle with Liquid Glass border
                RoundedRectangle(cornerRadius: 36, style: .continuous)
                    .strokeBorder(
                        isReady
                            ? LinearGradient(
                                colors: [Color(red: 0.20, green: 0.84, blue: 0.29), Color(red: 0.13, green: 0.75, blue: 0.42)],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            )
                            : LinearGradient(
                                colors: [Color.white.opacity(0.6), Color.white.opacity(0.2)],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            ),
                        lineWidth: isReady ? 3.5 : 2.0
                    )
                    .frame(width: width * 0.76, height: height * 0.58)
                    .shadow(
                        color: isReady ? Color(red: 0.20, green: 0.84, blue: 0.29).opacity(0.6) : Color.clear,
                        radius: 16
                    )
                    .overlay(
                        // Palm hand silhouette icon in center
                        Image(systemName: "hand.raised.fill")
                            .resizable()
                            .scaledToFit()
                            .frame(width: 90, height: 90)
                            .foregroundColor(isReady ? Color(red: 0.20, green: 0.84, blue: 0.29).opacity(0.35) : Color.white.opacity(0.18))
                    )

                // Top Guidance Status Pill Badge
                VStack {
                    HStack(spacing: 8) {
                        Circle()
                            .fill(statusColor)
                            .frame(width: 10, height: 10)
                            .shadow(color: statusColor.opacity(0.8), radius: 4)

                        Text(guidanceState.message)
                            .font(.system(size: 15, weight: .semibold, design: .rounded))
                            .foregroundColor(.white)
                    }
                    .padding(.horizontal, 18)
                    .padding(.vertical, 10)
                    .background(
                        Capsule()
                            .fill(.ultraThinMaterial)
                            .overlay(
                                Capsule()
                                    .strokeBorder(Color.white.opacity(0.25), lineWidth: 1)
                            )
                    )
                    .shadow(color: Color.black.opacity(0.25), radius: 10, y: 4)
                    .padding(.top, 60)

                    Spacer()

                    // Bottom Telemetry Bar & Manual Capture Button
                    VStack(spacing: 16) {
                        // Diagnostic Telemetry indicators
                        HStack(spacing: 12) {
                            telemetryChip(
                                title: "Lux",
                                value: String(format: "%.0f", metrics.meanLuminance),
                                isValid: metrics.isIlluminationOk
                            )
                            telemetryChip(
                                title: "Glare",
                                value: String(format: "%.1f%%", metrics.glareRatio * 100),
                                isValid: metrics.glareRatio <= 0.04
                            )
                            telemetryChip(
                                title: "Tilt",
                                value: String(format: "%.0f°", metrics.rollAngleDeg),
                                isValid: metrics.isAngleOk
                            )
                        }

                        // Manual Capture Shutter Button
                        Button(action: onManualCapture) {
                            ZStack {
                                Circle()
                                    .fill(.ultraThinMaterial)
                                    .frame(width: 76, height: 76)
                                    .overlay(
                                        Circle()
                                            .strokeBorder(Color.white.opacity(0.6), lineWidth: 2)
                                    )

                                Circle()
                                    .fill(isReady ? Color(red: 0.20, green: 0.84, blue: 0.29) : Color.white)
                                    .frame(width: 60, height: 60)
                                    .shadow(color: (isReady ? Color(red: 0.20, green: 0.84, blue: 0.29) : Color.white).opacity(0.5), radius: 8)
                            }
                        }
                        .padding(.bottom, 40)
                    }
                }
            }
        }
    }

    private func telemetryChip(title: String, value: String, isValid: Bool) -> some View {
        HStack(spacing: 4) {
            Text(title)
                .font(.system(size: 11, weight: .regular))
                .foregroundColor(.white.opacity(0.7))
            Text(value)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .foregroundColor(isValid ? Color(red: 0.20, green: 0.84, blue: 0.29) : Color(red: 1.00, green: 0.80, blue: 0.00))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.4))
                .overlay(
                    Capsule().stroke(Color.white.opacity(0.15), lineWidth: 0.5)
                )
        )
    }
}
