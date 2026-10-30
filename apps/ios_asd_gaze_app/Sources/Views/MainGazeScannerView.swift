import SwiftUI
import CurvAsdGazeKit

public enum ScannerDisplayMode: String, CaseIterable, Identifiable {
    case sideBySide = "Split"
    case pictureInPicture = "PiP"
    case stimulusOnly = "Full"

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .sideBySide: return "Split"
        case .pictureInPicture: return "PiP"
        case .stimulusOnly: return "Full"
        }
    }

    public var icon: String {
        switch self {
        case .sideBySide: return "rectangle.split.2x1.fill"
        case .pictureInPicture: return "pip.fill"
        case .stimulusOnly: return "rectangle.fill"
        }
    }
}

/// Live interactive gaze & facial affect scanning view mirroring the Palm Scanner UX.
/// Features side-by-side cartoon/stimulus player + TrueDepth front camera alignment HUD.
public struct MainGazeScannerView: View {
    @Environment(\.dismiss) private var dismiss
    @StateObject private var controller = GazeSessionController()
    @State private var displayMode: ScannerDisplayMode = .sideBySide
    @State private var sessionTimer: Timer?
    @State private var elapsedSeconds: Double = 0.0
    @State private var cartoonBlink: Bool = false
    @State private var cartoonBounce: CGFloat = 0.0
    #if ASD_RESEARCH_CLASSIFIER
    @State private var presentedReport: (summary: GazeSessionReportSummary, frames: [GazeFrame], classification: BiomarkerClassificationResult)?
    #else
    @State private var presentedReport: (summary: GazeSessionReportSummary, frames: [GazeFrame])?
    #endif
    @State private var showCalibrationSheet: Bool = false
    @State private var entryBlock: DeviceBlockReason?
    @StateObject private var camera = CameraAccessCoordinator()
    /// SIM-2/SIM-7: why capture cannot run right now, if it cannot.
    private var activeBlock: DeviceBlockReason? { entryBlock ?? controller.blockReason }

    public var onClose: (() -> Void)?
    public let onComplete: ((GazeSessionReportSummary, [GazeFrame]) -> Void)?
    public let onOpenHistory: (() -> Void)?
    public let onNavigateToCalibration: (() -> Void)?

    public init(
        onClose: (() -> Void)? = nil,
        onComplete: ((GazeSessionReportSummary, [GazeFrame]) -> Void)? = nil,
        onOpenHistory: (() -> Void)? = nil,
        onNavigateToCalibration: (() -> Void)? = nil
    ) {
        self.onClose = onClose
        self.onComplete = onComplete
        self.onOpenHistory = onOpenHistory
        self.onNavigateToCalibration = onNavigateToCalibration
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                // Adaptive Liquid Glass canvas (dark twilight in dark mode, airy porcelain in light mode)
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .ignoresSafeArea()

                VStack(spacing: 8) {
                    topHeaderBar
                    mainContentPane
                    bottomControlBar
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
            }
            .overlay {
                if camera.state.showsDeniedView {
                    CameraDeniedView(state: camera.state, onClose: onClose)
                } else if let reason = activeBlock {
                    UnsupportedDeviceView(reason: reason, onClose: onClose)
                }
            }
            .navigationTitle("")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                if onClose != nil {
                    ToolbarItem(placement: .topBarLeading) {
                        Button(action: handleClose) {
                            HStack(spacing: 4) {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.system(size: 16))
                                Text(Localizer.tr("close"))
                                    .font(.system(size: 15, weight: .medium))
                            }
                            .foregroundColor(LiquidGlass.celestialTeal)
                        }
                    }
                }
            }
            .sheet(isPresented: Binding(
                get: { presentedReport != nil },
                set: { if !$0 { presentedReport = nil } }
            )) {
                if let rep = presentedReport {
                    #if ASD_RESEARCH_CLASSIFIER
                    GazeSessionResultView(
                        summary: rep.summary,
                        recordedFrames: rep.frames,
                        classification: rep.classification,
                        onDismiss: { presentedReport = nil },
                        onRetake: {
                            presentedReport = nil
                            startSessionLoop()
                        }
                    )
                    #else
                    GazeSessionResultView(
                        summary: rep.summary,
                        recordedFrames: rep.frames,
                        onDismiss: { presentedReport = nil },
                        onRetake: {
                            presentedReport = nil
                            startSessionLoop()
                        }
                    )
                    #endif
                }
            }
            .fullScreenCover(isPresented: $showCalibrationSheet) {
                BubbleCalibrationView(
                    onClose: {
                        showCalibrationSheet = false
                        controller.reloadCalibrationProfile()
                    },
                    onCalibrationComplete: {
                        showCalibrationSheet = false
                        controller.reloadCalibrationProfile()
                    }
                )
            }
        }
        .task {
            controller.reloadCalibrationProfile()
            await runSessionLoop()
        }
        .onDisappear {
            endCapture()
        }
        .cameraScenePhase(camera) { action in
            // CAM-3: backgrounding stops the session; the observation ends honestly as
            // "interrupted" and the parent chooses to close or start again.
            if action == .stopSession {
                endCapture()
                entryBlock = .interrupted
            }
        }
    }

    // MARK: - Top Header Bar (Clean 3-Mode Toggle Only)

    private var topHeaderBar: some View {
        HStack {
            Spacer()
            modeToggleSelector
            Spacer()
        }
        .padding(.vertical, 4)
    }

    private var modeToggleSelector: some View {
        HStack(spacing: 2) {
            ForEach(ScannerDisplayMode.allCases) { mode in
                Button {
                    withAnimation(.spring(response: 0.32, dampingFraction: 0.82)) {
                        displayMode = mode
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: mode.icon)
                            .font(.system(size: 11, weight: .semibold))
                        Text(mode.title)
                            .font(.system(size: 11, weight: .semibold, design: .rounded))
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(
                        Capsule()
                            .fill(displayMode == mode ? LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity) : Color.clear)
                    )
                    .overlay(
                        Capsule()
                            .stroke(
                                displayMode == mode ? LiquidGlass.celestialTeal.opacity(0.85) : Color.clear,
                                lineWidth: 1
                            )
                    )
                    .foregroundColor(displayMode == mode ? LiquidGlass.celestialTeal : LiquidGlass.textSecondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .background(Capsule().fill(LiquidGlass.cardBackground))
        .overlay(Capsule().stroke(LiquidGlass.cardBorder, lineWidth: 0.8))
    }

    // MARK: - Faint Transparent HUD Floating Indicators

    private var faintEyeStatusIndicator: some View {
        let isActive = controller.trackingStage.isGazeActive && controller.currentQuality.isValid
        let isAcquiring = controller.trackingStage.hasFace
        let eyeColor: Color = isActive
            ? LiquidGlass.emeraldMint
            : (isAcquiring ? LiquidGlass.celestialTeal : LiquidGlass.deactivatedBrown)
        let iconName = isActive ? "eye.fill" : (isAcquiring ? "eye" : "eye.slash.fill")

        return ZStack {
            Circle()
                .fill(.ultraThinMaterial.opacity(0.65))
                .frame(width: 28, height: 28)
            Circle()
                .stroke(eyeColor.opacity(0.45), lineWidth: 0.9)
                .frame(width: 28, height: 28)
            Image(systemName: iconName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundColor(eyeColor)
        }
        .shadow(color: Color.black.opacity(0.12), radius: 4, x: 0, y: 2)
        .accessibilityLabel(isActive ? "Tracking Active" : "Tracking Inactive")
    }

    private var faintCalibrationIndicator: some View {
        let isCalibrated = controller.calibrationProfile.isCalibrated
        let targetColor: Color = isCalibrated
            ? LiquidGlass.emeraldMint
            : LiquidGlass.deactivatedBrown

        return Button(action: handleNavigateToCalibration) {
            ZStack {
                Circle()
                    .fill(.ultraThinMaterial.opacity(0.65))
                    .frame(width: 28, height: 28)
                Circle()
                    .stroke(targetColor.opacity(0.45), lineWidth: 0.9)
                    .frame(width: 28, height: 28)
                Image(systemName: "target")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(targetColor)
            }
            .shadow(color: Color.black.opacity(0.12), radius: 4, x: 0, y: 2)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(isCalibrated ? "Calibrated (Tap to recalibrate)" : "Uncalibrated (Tap to calibrate)")
    }

    private var faintTimerBadge: some View {
        HStack(spacing: 4) {
            Image(systemName: "stopwatch.fill")
                .font(.system(size: 9))
                .foregroundColor(LiquidGlass.textSecondary)
            Text(String(format: "%02d:%02d", Int(elapsedSeconds) / 60, Int(elapsedSeconds) % 60))
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .monospacedDigit()
                .foregroundColor(LiquidGlass.textPrimary)
        }
        .padding(.horizontal, 7)
        .padding(.vertical, 4)
        .background(
            Capsule()
                .fill(.ultraThinMaterial.opacity(0.65))
        )
        .overlay(
            Capsule()
                .stroke(Color.white.opacity(0.15), lineWidth: 0.8)
        )
        .shadow(color: Color.black.opacity(0.12), radius: 4, x: 0, y: 2)
    }

    // MARK: - Main Content Split Pane

    @ViewBuilder
    private var mainContentPane: some View {
        GeometryReader { geometry in
            let totalWidth = geometry.size.width
            let totalHeight = geometry.size.height

            ZStack(alignment: .bottomTrailing) {
                switch displayMode {
                case .sideBySide:
                    HStack(spacing: 8) {
                        stimulusPlayerPane(size: CGSize(width: totalWidth * 0.58, height: totalHeight))
                            .frame(width: totalWidth * 0.58, height: totalHeight)

                        faceAlignmentHUDPane(size: CGSize(width: totalWidth * 0.40, height: totalHeight))
                            .frame(width: totalWidth * 0.40, height: totalHeight)
                    }
                    .frame(width: totalWidth, height: totalHeight)

                case .pictureInPicture:
                    ZStack(alignment: .bottomTrailing) {
                        stimulusPlayerPane(size: geometry.size)
                            .frame(width: totalWidth, height: totalHeight)

                        compactFaceFloatingHUD
                            .frame(width: 135, height: 175)
                            .shadow(color: Color.black.opacity(0.45), radius: 10, x: 0, y: 5)
                            .padding(.trailing, 10)
                            .padding(.bottom, 10)
                    }
                    .frame(width: totalWidth, height: totalHeight)

                case .stimulusOnly:
                    ZStack {
                        stimulusPlayerPane(size: geometry.size)
                            .frame(width: totalWidth, height: totalHeight)

                        // Keep camera session alive invisibly so ARSession is uninterrupted
                        ARFaceCameraView(
                            session: controller.arSession,
                            isSimulation: controller.isSimulationMode,
                            headYawDeg: controller.headYawDeg,
                            headPitchDeg: controller.headPitchDeg,
                            isValid: controller.currentQuality.isValid
                        )
                        .frame(width: 1, height: 1)
                        .opacity(0.001)
                        .allowsHitTesting(false)
                    }
                    .frame(width: totalWidth, height: totalHeight)
                }
            }
        }
    }

    // MARK: - Pane A: Cartoon Stimulus Player

    private func stimulusPlayerPane(size: CGSize) -> some View {
        ZStack {
            // Split-Screen Clinical Stimulus (Cartoon Social Scene vs Dynamic Geometry)
            HStack(spacing: 1) {
                // Left: Cartoon Actor Scene (Social)
                ZStack {
                    LinearGradient(
                        colors: [LiquidGlass.stimulusSocialTop, LiquidGlass.stimulusSocialBottom],
                        startPoint: .top, endPoint: .bottom
                    )

                    VStack(spacing: 12) {
                        // Engaging Cartoon Sun Character
                        ZStack {
                            Circle()
                                .fill(
                                    RadialGradient(
                                        colors: [Color.yellow, Color.orange],
                                        center: .center, startRadius: 10, endRadius: 45
                                    )
                                )
                                .frame(width: 90, height: 90)
                                .shadow(color: Color.orange.opacity(0.6), radius: 15)

                            // Cartoon Face
                            VStack(spacing: 8) {
                                HStack(spacing: 18) {
                                    Circle()
                                        .fill(Color.black)
                                        .frame(width: 8, height: cartoonBlink ? 2 : 12)
                                    Circle()
                                        .fill(Color.black)
                                        .frame(width: 8, height: cartoonBlink ? 2 : 12)
                                }
                                Capsule()
                                    .fill(Color.red)
                                    .frame(width: 22, height: 8)
                            }
                        }
                        .offset(y: cartoonBounce)

                        Text("Sunny & Friends (Social)")
                            .font(.system(size: 13, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textPrimary)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)

                // Right: Geometric Kaleidoscope (Non-Social)
                ZStack {
                    LinearGradient(
                        colors: [LiquidGlass.stimulusGeometryTop, LiquidGlass.stimulusGeometryBottom],
                        startPoint: .top, endPoint: .bottom
                    )

                    VStack(spacing: 10) {
                        Image(systemName: "sparkles")
                            .font(.system(size: 40))
                            .foregroundColor(LiquidGlass.celestialTeal)
                            .rotationEffect(.degrees(elapsedSeconds * 45))

                        Text("Dynamic Patterns")
                            .font(.system(size: 13, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textPrimary)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }

            // Real-Time Gaze Reticle Overlay
            Circle()
                .stroke(LiquidGlass.celestialTeal, lineWidth: 3)
                .background(Circle().fill(LiquidGlass.celestialTeal.opacity(0.35)))
                .frame(width: 32, height: 32)
                .position(
                    x: controller.currentGazePoint.x * size.width,
                    y: controller.currentGazePoint.y * size.height
                )
                .shadow(color: LiquidGlass.celestialTeal, radius: 10)

            // Faint Floating HUD Overlays
            VStack {
                // Top-Right: Monospaced Stopwatch Timer
                HStack {
                    Spacer()
                    faintTimerBadge
                }
                .padding(.top, 8)
                .padding(.trailing, 8)

                Spacer()

                // Bottom: Eye Indicator (Left) & Calibration Indicator (Right if Full Screen mode)
                HStack(alignment: .bottom) {
                    faintEyeStatusIndicator
                        .padding(.leading, 8)

                    Spacer()

                    if displayMode == .stimulusOnly {
                        faintCalibrationIndicator
                            .padding(.trailing, 8)
                    }
                }
                .padding(.bottom, 8)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(LiquidGlass.cardBorder, lineWidth: 1)
        )
    }

    // MARK: - Pane B: Live Front Camera / Face HUD (Full)

    private func faceAlignmentHUDPane(size: CGSize) -> some View {
        let affect = controller.currentAffect
        let emotion = affect.dominantEmotion
        let emotionColor: Color = {
            switch emotion {
            case .joy, .excited: return LiquidGlass.emeraldMint
            case .surprise: return LiquidGlass.celestialTeal
            case .distress: return LiquidGlass.warmAmber
            case .calm: return LiquidGlass.textPrimary
            }
        }()

        return VStack(spacing: 8) {
            ZStack(alignment: .bottomTrailing) {
                ARFaceCameraView(
                    session: controller.arSession,
                    isSimulation: controller.isSimulationMode,
                    headYawDeg: controller.headYawDeg,
                    headPitchDeg: controller.headPitchDeg,
                    smileIntensity: affect.socialSmileIntensity,
                    surpriseScore: affect.surpriseScore,
                    distressScore: affect.distressScore,
                    dominantEmotion: emotion,
                    isValid: controller.currentQuality.isValid
                )

                // Dominant Emotional State Overlay Badge
                VStack {
                    Spacer()
                    HStack(spacing: 5) {
                        Text(emotion.emoji)
                            .font(.system(size: 13))
                        Text(emotion.rawValue)
                            .font(.system(size: 11, weight: .bold, design: .rounded))
                            .foregroundColor(emotionColor)
                    }
                    .padding(.horizontal, 9)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(.ultraThinMaterial))
                    .overlay(Capsule().stroke(LiquidGlass.cardBorder, lineWidth: 0.8))
                    .padding(.bottom, 6)
                }

                // Faint Calibration Indicator docked at bottom-right inside camera
                faintCalibrationIndicator
                    .padding(.trailing, 8)
                    .padding(.bottom, 6)
            }
            .frame(maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            // THM-6: the emotion badge and calibration indicator sit on the live camera,
            // so this camera sub-area resolves dark (bright accents over the feed).
            .mediaDark()

            // Distance, Head Pose, & Emotion Telemetry
            VStack(alignment: .leading, spacing: 5) {
                HStack {
                    Text("Dist:")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(LiquidGlass.textSecondary)
                    Spacer()
                    Text("\(Int(controller.currentDistanceCm)) cm")
                        .font(.system(size: 11, weight: .bold, design: .monospaced))
                        .monospacedDigit()
                        .foregroundColor(
                            (controller.currentDistanceCm >= 38 && controller.currentDistanceCm <= 55)
                                ? LiquidGlass.emeraldMint
                                : LiquidGlass.warmAmber
                        )
                }

                HStack {
                    Text("Pose:")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(LiquidGlass.textSecondary)
                    Spacer()
                    Text(String(format: "%+.0f° / %+.0f°", controller.headYawDeg, controller.headPitchDeg))
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .monospacedDigit()
                        .foregroundColor(LiquidGlass.textPrimary)
                }

                GlassDivider()

                // Real-Time Emotion Affect Spectrum Chips
                HStack(spacing: 4) {
                    emotionMicroChip(
                        emoji: "😊",
                        label: "Joy",
                        score: affect.socialSmileIntensity,
                        activeColor: LiquidGlass.emeraldMint
                    )
                    emotionMicroChip(
                        emoji: "😲",
                        label: "Surprise",
                        score: affect.surpriseScore,
                        activeColor: LiquidGlass.celestialTeal
                    )
                    emotionMicroChip(
                        emoji: "😟",
                        label: "Distress",
                        score: affect.distressScore,
                        activeColor: LiquidGlass.warmAmber
                    )
                }
            }
            .padding(.horizontal, 4)
        }
        .frame(width: size.width, height: size.height)
        .glassCard(cornerRadius: 14, padding: 8)
    }

    // MARK: - Pane C: Floating Picture-in-Picture Face HUD

    private var compactFaceFloatingHUD: some View {
        let affect = controller.currentAffect
        let emotion = affect.dominantEmotion

        return ZStack(alignment: .bottom) {
            ARFaceCameraView(
                session: controller.arSession,
                isSimulation: controller.isSimulationMode,
                headYawDeg: controller.headYawDeg,
                headPitchDeg: controller.headPitchDeg,
                smileIntensity: affect.socialSmileIntensity,
                surpriseScore: affect.surpriseScore,
                distressScore: affect.distressScore,
                dominantEmotion: emotion,
                isValid: controller.currentQuality.isValid
            )

            // Compact Floating Controls & Telemetry
            HStack(spacing: 4) {
                // Telemetry Badge on bottom-leading
                HStack(spacing: 3) {
                    Text(emotion.emoji)
                        .font(.system(size: 10))
                    Text("\(Int(controller.currentDistanceCm))cm")
                        .font(.system(size: 9, weight: .bold, design: .monospaced))
                        .monospacedDigit()
                        .foregroundColor(
                            (controller.currentDistanceCm >= 38 && controller.currentDistanceCm <= 55)
                                ? LiquidGlass.emeraldMint
                                : LiquidGlass.warmAmber
                        )
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(Capsule().fill(.ultraThinMaterial))
                .overlay(Capsule().stroke(LiquidGlass.cardBorder, lineWidth: 0.8))

                Spacer()

                // Faint Calibration Indicator on bottom-trailing inside camera
                faintCalibrationIndicator
            }
            .padding(.horizontal, 6)
            .padding(.bottom, 6)
        }
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        // THM-6: telemetry badge and calibration indicator sit on the live camera.
        .mediaDark()
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(LiquidGlass.celestialTeal.opacity(0.65), lineWidth: 1.2)
        )
    }

    private func emotionMicroChip(emoji: String, label: String, score: Double, activeColor: Color) -> some View {
        let isActive = score > 0.25
        return VStack(spacing: 2) {
            HStack(spacing: 2) {
                Text(emoji)
                    .font(.system(size: 10))
                Text("\(Int(score * 100))%")
                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                    .foregroundColor(isActive ? activeColor : LiquidGlass.textSecondary)
            }
            Text(label)
                .font(.system(size: 9, weight: .medium))
                .foregroundColor(LiquidGlass.textTertiary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 3)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(isActive ? activeColor.opacity(LiquidGlass.tintOpacity) : LiquidGlass.cardBackground)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(isActive ? activeColor.opacity(0.35) : LiquidGlass.cardBorder, lineWidth: 0.5)
        )
    }

    // MARK: - Bottom Control Bar

    private var bottomControlBar: some View {
        HStack(spacing: 12) {
            GlassButton(
                title: Localizer.tr("complete_observation"),
                systemImage: "checkmark.circle.fill",
                isPrimary: true
            ) {
                Task { await completeSession() }
            }
            .frame(maxWidth: .infinity)

            // Three-dot Options Menu on bottom right
            threeDotsMenuButton
        }
    }

    private var threeDotsMenuButton: some View {
        Menu {
            Button(action: {
                elapsedSeconds = 0.0
                Task { await beginConsentedCapture() }
            }) {
                Label("Reset Observation", systemImage: "arrow.counterclockwise")
            }

            #if ASD_SIMULATION_ENABLED
            // Debug-only (SIM-3/SIM-6): simulation is entered explicitly, never as a fallback.
            Button(action: {
                elapsedSeconds = 0.0
                if controller.isSimulationMode {
                    Task { await beginConsentedCapture() }
                } else {
                    entryBlock = nil
                    controller.startSimulatedSession()
                }
            }) {
                Label(
                    controller.isSimulationMode ? "Switch to TrueDepth" : "Switch to Simulation",
                    systemImage: controller.isSimulationMode ? "camera.viewfinder" : "cpu"
                )
            }
            #endif

            Button(action: handleNavigateToCalibration) {
                Label("Calibrate Gaze", systemImage: "target")
            }

            if let onOpenHistory {
                Button(action: onOpenHistory) {
                    Label("Caregiver History", systemImage: "chart.line.uptrend.xyaxis")
                }
            }
        } label: {
            ZStack {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(LiquidGlass.cardBackground)
                    .frame(width: 44, height: 44)
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(LiquidGlass.cardBorder, lineWidth: 1)
                    .frame(width: 44, height: 44)
                Image(systemName: "ellipsis")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }
        }
    }

    // MARK: - Actions

    private func handleNavigateToCalibration() {
        if let onNavigateToCalibration {
            onNavigateToCalibration()
        } else {
            showCalibrationSheet = true
        }
    }

    /// Stops the timer and, if a capture is running, the controller (idle timer restored).
    private func endCapture() {
        sessionTimer?.invalidate()
        sessionTimer = nil
        guard camera.isSessionActive else { return }
        camera.sessionDidEnd()
        Task { _ = await controller.stopSession() }
    }

    private func handleClose() {
        endCapture()
        if let onClose {
            onClose()
        } else {
            dismiss()
        }
    }

    private func completeSession() async {
        sessionTimer?.invalidate()
        sessionTimer = nil
        camera.sessionDidEnd()
        // `nil` when nothing recorded (live tracking unavailable): no sink receives
        // the empty summary in that case.
        let recordedSource = controller.captureSource
        let summary = await controller.stopSession()
        let frames = controller.recordedFrames

        #if ASD_RESEARCH_CLASSIFIER
        // 1. Research builds only (CLM-3): heuristic classifier.
        let classification = BiomarkerClassificationEngine.shared.classifySync(summary: summary)
        #endif

        if recordedSource != nil {
            // 1b. Build the de-identified session record and persist it to the
            // on-device history (dashboard trends, weekly badges, doctor report).
            // The history store ignores simulated sessions (SIM-5).
            let participantToken = GazeSessionRecord.participantToken(
                enrollment: ResearchKitStudyStore.shared.fetchEnrollment(),
                sessionId: summary.sessionId
            )
            let sessionRecord = GazeSessionRecord(summary: summary, participantToken: participantToken)
            GazeSessionHistoryStore.shared.record(sessionRecord)

            // 2. Batch encrypt & persist volatile RAM frames to disk atomically off main thread
            Task.detached(priority: .userInitiated) {
                let vault = SessionVaultManager()
                let header = SessionHeader(
                    sessionId: summary.sessionId,
                    participantToken: participantToken,
                    stimulusBattery: "GeoPref_v1",
                    stimulusDurationSec: summary.durationSec,
                    captureSource: summary.captureSource
                )
                #if ASD_RESEARCH_CLASSIFIER
                let framesFile = try? vault.saveSession(
                    header: header,
                    frames: frames,
                    report: summary,
                    classification: classification
                )
                #else
                let framesFile = try? vault.saveSession(header: header, frames: frames, report: summary)
                #endif
                // WP-A.4: queue for /v1/sessions (consented, configured, live only).
                // No per-video matrix here, so no attention flag is claimed.
                await MainActor.run {
                    _ = ResearchKitSyncService.shared?.enqueueSession(sessionRecord, framesFile: framesFile)
                }
            }

            // 3. Mark in CarePlan (which mirrors the outcome into CareKit and compiles
            // the ResearchKit step result from the full summary). Both ignore
            // simulated sessions (SIM-5).
            #if ASD_RESEARCH_CLASSIFIER
            CarePlanStore.shared.markScreeningCompleted(
                captureSource: summary.captureSource,
                socialScore: summary.socialPreferenceScore,
                validFramePercentage: summary.validRatio * 100.0,
                summary: summary,
                riskScore: classification.riskScore,
                riskStratification: classification.riskStratification.rawValue
            )
            #else
            CarePlanStore.shared.markScreeningCompleted(
                captureSource: summary.captureSource,
                socialScore: summary.socialPreferenceScore,
                validFramePercentage: summary.validRatio * 100.0,
                summary: summary
            )
            #endif
        }

        if let onComplete {
            onComplete(summary, frames)
        } else {
            #if ASD_RESEARCH_CLASSIFIER
            presentedReport = (summary, frames, classification)
            #else
            presentedReport = (summary, frames)
            #endif
        }
    }

    /// CNS-1: live capture only starts with a consent token minted from the profile store.
    /// CAM-1: nothing starts until camera access is granted.
    private func beginConsentedCapture() async {
        // SIM-7: hardware first, so a device without TrueDepth (or Apple Vision Pro)
        // never sees a camera prompt for capture it cannot run.
        guard CaptureEntryGate.deviceSupportsFaceTracking else {
            entryBlock = .faceTrackingUnsupported
            return
        }
        guard await camera.ensureAccess(consentStore: .shared) == .granted, !Task.isCancelled else {
            if camera.state == .unknown { entryBlock = .consentRequired }
            return
        }
        entryBlock = controller.startSessionWithConsent() ? nil : .consentRequired
        if entryBlock == nil { camera.sessionDidStart() }
    }

    private func startSessionLoop() {
        Task { await runSessionLoop() }
    }

    private func runSessionLoop() async {
        await beginConsentedCapture()
        guard camera.canStartSession, !Task.isCancelled else { return }
        sessionTimer?.invalidate()
        sessionTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
            MainActor.assumeIsolated {
                elapsedSeconds += 1.0 / 60.0
                #if ASD_SIMULATION_ENABLED
                if controller.isSimulationMode {
                    controller.tickSimulatedFrame(timeSec: elapsedSeconds)
                }
                #endif

                // Gentle cartoon bounce and blink animation
                cartoonBounce = sin(elapsedSeconds * 4.0) * 8.0
                if fmod(elapsedSeconds, 3.5) < 0.15 {
                    cartoonBlink = true
                } else {
                    cartoonBlink = false
                }
            }
        }
    }
}
