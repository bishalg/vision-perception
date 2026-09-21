import SwiftUI

public enum OverlayPalette {
    public static let emerald = Color(red: 0.20, green: 0.84, blue: 0.29)
    public static let emeraldDeep = Color(red: 0.13, green: 0.75, blue: 0.42)
    public static let amber = Color(red: 1.00, green: 0.80, blue: 0.00)
}

/**
 * Shared Liquid Glass HUD: vignette, reticle, status pill, telemetry, shutter.
 * App-specific artwork is injected through `overlayContent` (palm silhouette,
 * vastu mandala, document quad, ...).
 */
public struct LiquidGlassHUD<OverlayContent: View, BottomLeading: View, BottomTrailing: View>: View {
    public let profile: OverlayProfile
    public let guidanceState: GuidanceState
    public let metrics: QualityMetrics
    public let statusMessageOverride: String?
    public let overlayContent: OverlayContent
    public let bottomLeading: BottomLeading
    public let bottomTrailing: BottomTrailing
    public let onManualCapture: () -> Void
    /// When false the bottom shutter button is hidden (e.g. auto-capture
    /// orbit mode, where a manual still capture would corrupt the session).
    public var showsShutter: Bool = true
    /// Device roll vs gravity in degrees (0 = vertical). nil = use live gyro.
    public var deviceRollDeg: Float?

    @StateObject private var tiltProvider = DeviceTiltProvider()

    public init(
        profile: OverlayProfile,
        guidanceState: GuidanceState,
        metrics: QualityMetrics,
        statusMessageOverride: String? = nil,
        deviceRollDeg: Float? = nil,
        onManualCapture: @escaping () -> Void,
        showsShutter: Bool = true,
        @ViewBuilder bottomLeading: () -> BottomLeading,
        @ViewBuilder bottomTrailing: () -> BottomTrailing,
        @ViewBuilder overlayContent: () -> OverlayContent
    ) {
        self.profile = profile
        self.guidanceState = guidanceState
        self.metrics = metrics
        self.statusMessageOverride = statusMessageOverride
        self.deviceRollDeg = deviceRollDeg
        self.onManualCapture = onManualCapture
        self.showsShutter = showsShutter
        self.bottomLeading = bottomLeading()
        self.bottomTrailing = bottomTrailing()
        self.overlayContent = overlayContent()
    }

    private var isReady: Bool { guidanceState.isReady }
    private var effectiveDeviceRoll: Float {
        deviceRollDeg ?? tiltProvider.deviceRollDeg
    }

    private var statusColor: Color { isReady ? OverlayPalette.emerald : OverlayPalette.amber }

    private var statusText: String {
        statusMessageOverride ?? profile.copy.message(for: guidanceState)
    }

    public var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let height = proxy.size.height

            // Device-aware boundaries protecting top chrome (Dynamic Island / notch / mode bar)
            // and bottom chrome (controls / cards / home indicator bar)
            let topInset = max(proxy.safeAreaInsets.top, 0)
            let bottomInset = max(proxy.safeAreaInsets.bottom, 16)

            // Top chrome: Mode capsule (~36pt) + coaching chip (~36pt) + safe margin
            let topBoundary = topInset + 88
            // Bottom chrome: Shutter / bottom card (~140pt) + bottom safe area
            let bottomBoundary = height - (bottomInset + (showsShutter ? 134 : 158))
            let availableH = max(240, bottomBoundary - topBoundary)
            let reticleCenterY = topBoundary + availableH / 2

            let reticleH = min(height * profile.reticleHeightFraction, availableH - 8)
            let reticleW = min(width * profile.reticleWidthFraction, reticleH * 0.65)

            ZStack {
                Color.black.opacity(0.35)
                    .mask(
                        Rectangle()
                            .overlay(
                                RoundedRectangle(cornerRadius: profile.reticleCornerRadius, style: .continuous)
                                    .frame(width: reticleW, height: reticleH)
                                    .position(x: width / 2, y: reticleCenterY)
                                    .blendMode(.destinationOut)
                            )
                    )
                    .ignoresSafeArea()
                    .allowsHitTesting(false)

                RoundedRectangle(cornerRadius: profile.reticleCornerRadius, style: .continuous)
                    .strokeBorder(
                        isReady
                            ? LinearGradient(
                                colors: [OverlayPalette.emerald, OverlayPalette.emeraldDeep],
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
                    .frame(width: reticleW, height: reticleH)
                    .shadow(
                        color: isReady ? OverlayPalette.emerald.opacity(0.6) : Color.clear,
                        radius: 16
                    )
                    .overlay(overlayContent)
                    .overlay(alignment: .top) {
                        // Status instruction pill shown during still photo mode (other modes own their HUD)
                        if showsShutter {
                            statusInstructionPill
                                .padding(.top, 12)
                        }
                    }
                    .overlay {
                        if let digit = profile.copy.countdownDigit(for: guidanceState) {
                            Text("\(digit)")
                                .font(.system(size: 96, weight: .bold, design: .rounded))
                                .foregroundColor(OverlayPalette.emerald)
                                .shadow(color: OverlayPalette.emerald.opacity(0.55), radius: 18)
                                .contentTransition(.numericText())
                                .animation(.easeInOut(duration: 0.2), value: digit)
                        }
                    }
                    .position(x: width / 2, y: reticleCenterY)
                    .allowsHitTesting(false)

                // ── Left / Right tilt rails ──
                HStack {
                    TiltRailView(
                        deviceRollDeg: effectiveDeviceRoll,
                        handRollDeg: metrics.rollAngleDeg,
                        isLeft: true
                    )
                    Spacer()
                    TiltRailView(
                        deviceRollDeg: effectiveDeviceRoll,
                        handRollDeg: metrics.rollAngleDeg,
                        isLeft: false
                    )
                }
                .frame(width: reticleW + 28, height: reticleH)
                .position(x: width / 2, y: reticleCenterY)
                .allowsHitTesting(false)
                .animation(.easeOut(duration: 0.12), value: effectiveDeviceRoll)
                .animation(.easeOut(duration: 0.12), value: metrics.rollAngleDeg)

                if showsShutter {
                    VStack {
                        Spacer()

                        VStack(spacing: 14) {
                            HStack(spacing: 8) {
                                bottomLeading

                                Spacer()

                                HStack(spacing: 8) {
                                    ForEach(profile.telemetry, id: \.rawValue) { channel in
                                        telemetryChip(channel)
                                            .animation(.easeOut(duration: 0.12), value: metrics.meanLuminance)
                                            .animation(.easeOut(duration: 0.12), value: metrics.glareRatio)
                                            .animation(.easeOut(duration: 0.12), value: metrics.rollAngleDeg)
                                    }
                                }

                                Spacer()

                                bottomTrailing
                            }
                            .padding(.horizontal, 16)

                            Button(action: onManualCapture) {
                                ZStack {
                                    Circle()
                                        .fill(Color.black.opacity(0.55))
                                        .frame(width: 76, height: 76)
                                        .overlay(Circle().strokeBorder(Color.white.opacity(0.6), lineWidth: 2))

                                    Circle()
                                        .fill(isReady ? OverlayPalette.emerald : Color.white)
                                        .frame(width: 60, height: 60)
                                        .shadow(
                                            color: (isReady ? OverlayPalette.emerald : Color.white).opacity(0.5),
                                            radius: 8
                                        )
                                }
                            }
                        }
                        .padding(.bottom, bottomInset + 12)
                    }
                }
            }
        }
        .onAppear { tiltProvider.start() }
        .onDisappear { tiltProvider.stop() }
    }

    @ViewBuilder
    private func telemetryChip(_ channel: TelemetryChannel) -> some View {
        switch channel {
        case .lux:
            chip(title: "Lux", value: String(format: "%.0f", metrics.meanLuminance), isValid: metrics.isIlluminationOk)
        case .glare:
            chip(
                title: "Glare",
                value: String(format: "%.1f%%", metrics.glareRatio * 100),
                isValid: metrics.glareRatio <= profile.thresholds.maxGlareRatio
            )
        case .tilt:
            chip(title: "Tilt", value: String(format: "%.0f°", metrics.rollAngleDeg), isValid: metrics.isAngleOk)
        }
    }

    private var statusInstructionPill: some View {
        HStack(spacing: 7) {
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)
                .shadow(color: statusColor.opacity(0.8), radius: 3)

            Text(statusText)
                .font(.system(
                    size: profile.copy.countdownDigit(for: guidanceState) != nil ? 18 : 13,
                    weight: .semibold,
                    design: .rounded
                ))
                .foregroundColor(.white)
                .lineLimit(1)
                .minimumScaleFactor(0.75)
                .contentTransition(.numericText())
                .animation(.easeInOut(duration: 0.15), value: statusText)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 7)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.65))
                .overlay(Capsule().strokeBorder(Color.white.opacity(0.24), lineWidth: 0.8))
        )
        .shadow(color: Color.black.opacity(0.3), radius: 6, y: 2)
    }

    private func chip(title: String, value: String, isValid: Bool) -> some View {
        HStack(spacing: 4) {
            Text(title)
                .font(.system(size: 11, weight: .regular))
                .foregroundColor(.white.opacity(0.7))
            Text(value)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .foregroundColor(isValid ? OverlayPalette.emerald : OverlayPalette.amber)
                .contentTransition(.numericText())
                .animation(.linear(duration: 0.1), value: value)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.4))
                .overlay(Capsule().stroke(Color.white.opacity(0.15), lineWidth: 0.5))
        )
    }
}

extension LiquidGlassHUD where BottomLeading == EmptyView, BottomTrailing == EmptyView {
    public init(
        profile: OverlayProfile,
        guidanceState: GuidanceState,
        metrics: QualityMetrics,
        statusMessageOverride: String? = nil,
        deviceRollDeg: Float? = nil,
        onManualCapture: @escaping () -> Void,
        showsShutter: Bool = true,
        @ViewBuilder overlayContent: () -> OverlayContent
    ) {
        self.init(
            profile: profile,
            guidanceState: guidanceState,
            metrics: metrics,
            statusMessageOverride: statusMessageOverride,
            deviceRollDeg: deviceRollDeg,
            onManualCapture: onManualCapture,
            showsShutter: showsShutter,
            bottomLeading: { EmptyView() },
            bottomTrailing: { EmptyView() },
            overlayContent: overlayContent
        )
    }
}
