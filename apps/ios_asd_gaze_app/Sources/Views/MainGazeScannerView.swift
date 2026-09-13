import SwiftUI
import CurvAsdGazeKit

public enum ScannerDisplayMode: String, CaseIterable, Identifiable {
    case sideBySide = "Side-by-Side"
    case pictureInPicture = "Picture-in-Picture"
    case stimulusOnly = "Full Screen"

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .sideBySide: return "Side-by-Side (Split View)"
        case .pictureInPicture: return "Picture-in-Picture (Floating)"
        case .stimulusOnly: return "Full Screen (Stimulus Only)"
        }
    }

    public var description: String {
        switch self {
        case .sideBySide: return "Stimulus & Face Tracking together"
        case .pictureInPicture: return "Maximized video with floating camera"
        case .stimulusOnly: return "Full video only without camera HUD"
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
    @State private var presentedReport: (summary: GazeSessionReportSummary, frames: [GazeFrame])?

    public var onClose: (() -> Void)?
    public let onComplete: ((GazeSessionReportSummary, [GazeFrame]) -> Void)?
    public let onOpenHistory: (() -> Void)?

    public init(
        onClose: (() -> Void)? = nil,
        onComplete: ((GazeSessionReportSummary, [GazeFrame]) -> Void)? = nil,
        onOpenHistory: (() -> Void)? = nil
    ) {
        self.onClose = onClose
        self.onComplete = onComplete
        self.onOpenHistory = onOpenHistory
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
                    topStatusBar
                    mainContentPane
                    bottomControlBar
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
            }
            .navigationTitle(Localizer.tr("dev_gaze_title"))
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

                ToolbarItem(placement: .principal) {
                    HStack(spacing: 6) {
                        Text(Localizer.tr("dev_gaze_title"))
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textPrimary)
                        Image(systemName: controller.currentQuality.isValid ? "eye.fill" : "eye.slash")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(controller.currentQuality.isValid ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                    }
                }

                ToolbarItem(placement: .topBarTrailing) {
                    HStack(spacing: 8) {
                        // Intuitive Layout Selector Menu with active layout label & chevron
                        Menu {
                            Section("SWITCH SCREEN LAYOUT") {
                                ForEach(ScannerDisplayMode.allCases) { mode in
                                    Button {
                                        withAnimation(.spring(response: 0.35, dampingFraction: 0.8)) {
                                            displayMode = mode
                                        }
                                    } label: {
                                        HStack {
                                            Label(mode.title, systemImage: mode.icon)
                                            if displayMode == mode {
                                                Image(systemName: "checkmark")
                                            }
                                        }
                                    }
                                }
                            }
                        } label: {
                            HStack(spacing: 4) {
                                Image(systemName: displayMode.icon)
                                    .font(.system(size: 13, weight: .semibold))
                                Text(displayMode.rawValue)
                                    .font(.system(size: 12, weight: .semibold, design: .rounded))
                                Image(systemName: "chevron.down")
                                    .font(.system(size: 8, weight: .bold))
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 5)
                            .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.18)))
                            .foregroundColor(LiquidGlass.celestialTeal)
                        }

                        Menu {
                            Button(action: {
                                controller.isSimulationMode.toggle()
                            }) {
                                Label(
                                    controller.isSimulationMode ? "Switch to TrueDepth" : "Switch to Simulation",
                                    systemImage: controller.isSimulationMode ? "camera.viewfinder" : "cpu"
                                )
                            }

                            Button(action: {
                                elapsedSeconds = 0.0
                                controller.startSession()
                            }) {
                                Label("Reset Observation", systemImage: "arrow.counterclockwise")
                            }

                            if let onOpenHistory {
                                Button(action: onOpenHistory) {
                                    Label("Caregiver History", systemImage: "chart.line.uptrend.xyaxis")
                                }
                            }
                        } label: {
                            Image(systemName: "ellipsis.circle")
                                .font(.system(size: 17))
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
                    GazeSessionResultView(
                        summary: rep.summary,
                        recordedFrames: rep.frames,
                        onDismiss: { presentedReport = nil },
                        onRetake: {
                            presentedReport = nil
                            startSessionLoop()
                        }
                    )
                }
            }
        }
        .onAppear {
            startSessionLoop()
        }
        .onDisappear {
            sessionTimer?.invalidate()
            _ = controller.stopSession()
        }
    }

    // MARK: - Top Status Bar (Stable, Non-Flickering)

    private var topStatusBar: some View {
        HStack(spacing: 10) {
            // Quality status
            HStack(spacing: 5) {
                Circle()
                    .fill(controller.currentQuality.isValid ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                    .frame(width: 7, height: 7)
                Text(controller.currentQuality.isValid ? "Tracking" : "Searching...")
                    .font(.system(size: 12, weight: .semibold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)
            }

            Spacer()

            // Active visual focus pill (fixed layout)
            HStack(spacing: 4) {
                Image(systemName: "eye.fill")
                    .font(.system(size: 10))
                    .foregroundColor(LiquidGlass.celestialTeal)
                Text(controller.activeAOIs.contains("Social_Actor") ? "Focus: Social Scene" : "Focus: Dynamic Patterns")
                    .font(.system(size: 11, weight: .medium, design: .rounded))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.16)))

            Spacer()

            // Timer with tabular monospaced digits (stable width)
            HStack(spacing: 4) {
                Image(systemName: "stopwatch.fill")
                    .font(.system(size: 10))
                    .foregroundColor(LiquidGlass.textSecondary)
                Text(String(format: "%02d:%02d", Int(elapsedSeconds) / 60, Int(elapsedSeconds) % 60))
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .monospacedDigit()
                    .foregroundColor(LiquidGlass.textPrimary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .glassCard(cornerRadius: 12, padding: 4)
    }

    private func cycleDisplayMode() {
        withAnimation(.easeInOut(duration: 0.25)) {
            switch displayMode {
            case .sideBySide: displayMode = .pictureInPicture
            case .pictureInPicture: displayMode = .stimulusOnly
            case .stimulusOnly: displayMode = .sideBySide
            }
        }
    }

    // MARK: - Main Content Split Pane

    @ViewBuilder
    private var mainContentPane: some View {
        GeometryReader { geometry in
            switch displayMode {
            case .sideBySide:
                HStack(spacing: 8) {
                    stimulusPlayerPane(size: CGSize(width: geometry.size.width * 0.58, height: geometry.size.height))
                    faceAlignmentHUDPane(size: CGSize(width: geometry.size.width * 0.40, height: geometry.size.height))
                }
            case .pictureInPicture:
                ZStack(alignment: .bottomTrailing) {
                    stimulusPlayerPane(size: geometry.size)
                    faceAlignmentHUDPane(size: CGSize(width: 140, height: 190))
                        .padding(12)
                }
            case .stimulusOnly:
                stimulusPlayerPane(size: geometry.size)
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
                            .foregroundColor(LiquidGlass.celestialTeal.opacity(0.85))
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
        }
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(LiquidGlass.cardBorder, lineWidth: 1)
        )
    }

    // MARK: - Pane B: Live Front Camera / Face HUD

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
            ZStack {
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
            }
            .frame(maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))

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
        .glassCard(cornerRadius: 14, padding: 8)
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
                .fill(isActive ? activeColor.opacity(0.18) : LiquidGlass.cardBackground)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(isActive ? activeColor.opacity(0.35) : LiquidGlass.cardBorder, lineWidth: 0.5)
        )
    }

    // MARK: - Bottom Control Bar

    private var bottomControlBar: some View {
        HStack(spacing: 12) {
            Button(action: {
                elapsedSeconds = 0.0
                controller.startSession()
            }) {
                Image(systemName: "arrow.counterclockwise")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundColor(LiquidGlass.textPrimary)
                    .frame(width: 44, height: 44)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(LiquidGlass.cardBackground)
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .stroke(LiquidGlass.cardBorder, lineWidth: 1)
                    )
            }

            GlassButton(
                title: Localizer.tr("complete_observation"),
                systemImage: "checkmark.circle.fill",
                isPrimary: true
            ) {
                sessionTimer?.invalidate()
                let summary = controller.stopSession()
                CarePlanStore.shared.markScreeningCompleted(socialScore: summary.socialPreferenceScore)
                if let onComplete {
                    onComplete(summary, controller.recordedFrames)
                } else {
                    presentedReport = (summary, controller.recordedFrames)
                }
            }
        }
    }

    // MARK: - Session Loop & Actions

    private func handleClose() {
        sessionTimer?.invalidate()
        _ = controller.stopSession()
        if let onClose {
            onClose()
        } else {
            dismiss()
        }
    }

    private func startSessionLoop() {
        controller.startSession()
        sessionTimer?.invalidate()
        sessionTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
            MainActor.assumeIsolated {
                elapsedSeconds += 1.0 / 60.0
                if controller.isSimulationMode {
                    controller.tickSimulatedFrame(timeSec: elapsedSeconds)
                }

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
