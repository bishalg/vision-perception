import SwiftUI

/**
 * Shared scanner chrome: live camera + Liquid Glass HUD + brand breadcrumb +
 * analyzing veil. Apps layer domain results (splines, sheets) on top.
 */
public struct CameraOverlayHost<OverlayContent: View, BottomLeading: View, BottomTrailing: View>: View {
    public let profile: OverlayProfile
    public let session: CameraSession
    public let guidanceState: GuidanceState
    public let metrics: QualityMetrics
    public let isAnalyzing: Bool
    public let statusMessageOverride: String?
    public let deviceRollDeg: Float?
    public let bottomLeading: BottomLeading
    public let bottomTrailing: BottomTrailing
    public let overlayContent: OverlayContent
    public let onManualCapture: () -> Void
    /// Hides the bottom shutter button (auto-capture modes supply their own UI).
    public let showsShutter: Bool

    public init(
        profile: OverlayProfile,
        session: CameraSession,
        guidanceState: GuidanceState,
        metrics: QualityMetrics,
        isAnalyzing: Bool,
        statusMessageOverride: String? = nil,
        deviceRollDeg: Float? = nil,
        onManualCapture: @escaping () -> Void,
        showsShutter: Bool = true,
        @ViewBuilder bottomLeading: () -> BottomLeading,
        @ViewBuilder bottomTrailing: () -> BottomTrailing,
        @ViewBuilder overlayContent: () -> OverlayContent
    ) {
        self.profile = profile
        self.session = session
        self.guidanceState = guidanceState
        self.metrics = metrics
        self.isAnalyzing = isAnalyzing
        self.statusMessageOverride = statusMessageOverride
        self.deviceRollDeg = deviceRollDeg
        self.onManualCapture = onManualCapture
        self.showsShutter = showsShutter
        self.bottomLeading = bottomLeading()
        self.bottomTrailing = bottomTrailing()
        self.overlayContent = overlayContent()
    }

    public var body: some View {
        ZStack {
            CameraPreviewView(session: session.captureSession)
                .ignoresSafeArea()

            LiquidGlassHUD(
                profile: profile,
                guidanceState: guidanceState,
                metrics: metrics,
                statusMessageOverride: statusMessageOverride,
                deviceRollDeg: deviceRollDeg,
                onManualCapture: onManualCapture,
                showsShutter: showsShutter,
                bottomLeading: { bottomLeading },
                bottomTrailing: { bottomTrailing }
            ) {
                overlayContent
            }

            // NOTE: no brand breadcrumb — the top notch area stays fully clear
            // for the mode capsule and scan progress.

            if isAnalyzing {
                ZStack {
                    Color.black.opacity(0.6).ignoresSafeArea()
                    VStack(spacing: 16) {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle(tint: .white))
                            .scaleEffect(1.5)
                        Text(profile.copy.processing)
                            .font(.system(size: 15, weight: .semibold, design: .rounded))
                            .foregroundColor(.white)
                    }
                    .padding(28)
                    .background(
                        RoundedRectangle(cornerRadius: 24, style: .continuous)
                            .fill(.ultraThinMaterial)
                            .overlay(
                                RoundedRectangle(cornerRadius: 24, style: .continuous)
                                    .stroke(Color.white.opacity(0.25), lineWidth: 1)
                            )
                    )
                }
            }
        }
    }
}

extension CameraOverlayHost where BottomLeading == EmptyView, BottomTrailing == EmptyView {
    public init(
        profile: OverlayProfile,
        session: CameraSession,
        guidanceState: GuidanceState,
        metrics: QualityMetrics,
        isAnalyzing: Bool,
        statusMessageOverride: String? = nil,
        deviceRollDeg: Float? = nil,
        onManualCapture: @escaping () -> Void,
        showsShutter: Bool = true,
        @ViewBuilder overlayContent: () -> OverlayContent
    ) {
        self.init(
            profile: profile,
            session: session,
            guidanceState: guidanceState,
            metrics: metrics,
            isAnalyzing: isAnalyzing,
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
