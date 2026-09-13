import SwiftUI
import CurvAsdGazeKit

public enum ScannerDisplayMode: String, CaseIterable, Identifiable {
    case sideBySide = "Side-by-Side"
    case pictureInPicture = "PIP"
    case stimulusOnly = "Stimulus"

    public var id: String { rawValue }
}

/// Live interactive gaze & facial affect scanning view mirroring the Palm Scanner UX.
/// Features side-by-side cartoon/stimulus player + TrueDepth front camera alignment HUD.
public struct MainGazeScannerView: View {
    @StateObject private var controller = GazeSessionController()
    @State private var displayMode: ScannerDisplayMode = .sideBySide
    @State private var selectedLocale: AppLocale = .english
    @State private var sessionTimer: Timer?
    @State private var elapsedSeconds: Double = 0.0
    @State private var cartoonBlink: Bool = false
    @State private var cartoonBounce: CGFloat = 0.0

    public let onComplete: (GazeSessionReportSummary, [GazeFrame]) -> Void
    public let onOpenHistory: () -> Void

    public init(
        onComplete: @escaping (GazeSessionReportSummary, [GazeFrame]) -> Void,
        onOpenHistory: @escaping () -> Void
    ) {
        self.onComplete = onComplete
        self.onOpenHistory = onOpenHistory
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                Color.black.ignoresSafeArea()

                VStack(spacing: 8) {
                    topToolbarHUD
                    mainContentPane
                    bottomControlBar
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button(action: onOpenHistory) {
                        HStack(spacing: 4) {
                            Image(systemName: "chart.line.uptrend.xyaxis")
                            Text("History")
                                .font(.system(size: 14, weight: .semibold, design: .rounded))
                        }
                        .foregroundColor(LiquidGlass.celestialTeal)
                    }
                }
                ToolbarItem(placement: .principal) {
                    HStack(spacing: 6) {
                        Circle()
                            .fill(controller.currentQuality.isValid ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                            .frame(width: 8, height: 8)
                        Text(controller.isSimulationMode ? "Simulation 60Hz" : "TrueDepth 60Hz")
                            .font(.system(size: 12, weight: .bold, design: .monospaced))
                            .foregroundColor(.white)
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Picker("Language", selection: $selectedLocale) {
                        ForEach(AppLocale.allCases) { locale in
                            Text(locale.displayName).tag(locale)
                        }
                    }
                    .pickerStyle(.menu)
                    .tint(LiquidGlass.celestialTeal)
                }
            }
        }
        .onAppear {
            startSessionLoop()
        }
        .onDisappear {
            sessionTimer?.invalidate()
        }
    }

    // MARK: - Top Toolbar HUD

    private var topToolbarHUD: some View {
        HStack(spacing: 12) {
            Picker("Mode", selection: $displayMode) {
                ForEach(ScannerDisplayMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 260)

            Spacer()

            // Active AOI badge
            if let aoi = controller.activeAOIs.first {
                Text(aoi)
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.2)))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }

            // Timer display
            Text(String(format: "%02d:%02d", Int(elapsedSeconds) / 60, Int(elapsedSeconds) % 60))
                .font(.system(size: 13, weight: .bold, design: .monospaced))
                .foregroundColor(.white.opacity(0.8))
        }
        .glassCard(cornerRadius: 12, padding: 8)
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
                        colors: [Color(red: 0.10, green: 0.16, blue: 0.28), Color(red: 0.05, green: 0.08, blue: 0.16)],
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
                            .foregroundColor(.white)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)

                // Right: Geometric Kaleidoscope (Non-Social)
                ZStack {
                    LinearGradient(
                        colors: [Color(red: 0.20, green: 0.08, blue: 0.24), Color(red: 0.08, green: 0.04, blue: 0.12)],
                        startPoint: .top, endPoint: .bottom
                    )

                    VStack(spacing: 10) {
                        Image(systemName: "sparkles")
                            .font(.system(size: 40))
                            .foregroundColor(LiquidGlass.celestialTeal.opacity(0.85))
                            .rotationEffect(.degrees(elapsedSeconds * 45))

                        Text("Dynamic Patterns")
                            .font(.system(size: 13, weight: .bold, design: .rounded))
                            .foregroundColor(.white.opacity(0.8))
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
                .stroke(Color.white.opacity(0.12), lineWidth: 1)
        )
    }

    // MARK: - Pane B: Live Front Camera / Face HUD

    private func faceAlignmentHUDPane(size: CGSize) -> some View {
        VStack(spacing: 8) {
            ZStack {
                Color.black.opacity(0.6)

                // Face Alignment Frame Silhouette
                RoundedRectangle(cornerRadius: 24, style: .continuous)
                    .stroke(
                        controller.currentQuality.isValid ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                        lineWidth: 2
                    )
                    .frame(width: min(size.width * 0.75, 110), height: min(size.height * 0.55, 130))

                // Facial Affect / Smile Indicator
                VStack {
                    Spacer()
                    HStack(spacing: 4) {
                        Image(systemName: "face.smiling.fill")
                            .foregroundColor(controller.currentAffect.socialSmileIntensity > 0.40 ? LiquidGlass.emeraldMint : .gray)
                            .font(.system(size: 14))
                        Text(String(format: "Smile: %d%%", Int(controller.currentAffect.socialSmileIntensity * 100)))
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                            .foregroundColor(.white)
                    }
                    .padding(.bottom, 6)
                }
            }
            .frame(maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))

            // Distance & Pose Telemetry Bars
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text("Dist:")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(Color.white.opacity(0.6))
                    Spacer()
                    Text("\(Int(controller.currentDistanceCm)) cm")
                        .font(.system(size: 11, weight: .bold, design: .monospaced))
                        .foregroundColor(
                            (controller.currentDistanceCm >= 35 && controller.currentDistanceCm <= 65)
                                ? LiquidGlass.emeraldMint
                                : LiquidGlass.warmAmber
                        )
                }

                HStack {
                    Text("Yaw / Pitch:")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(Color.white.opacity(0.6))
                    Spacer()
                    Text(String(format: "%+.0f° / %+.0f°", controller.headYawDeg, controller.headPitchDeg))
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundColor(.white)
                }
            }
            .padding(.horizontal, 4)
        }
        .glassCard(cornerRadius: 14, padding: 10)
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
                    .foregroundColor(.white)
                    .frame(width: 44, height: 44)
                    .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(LiquidGlass.cardBackground))
            }

            GlassButton(
                title: Localizer.tr("complete_observation", lang: selectedLocale),
                systemImage: "checkmark.circle.fill",
                isPrimary: true
            ) {
                sessionTimer?.invalidate()
                let summary = controller.stopSession()
                onComplete(summary, controller.recordedFrames)
            }
        }
    }

    // MARK: - Session Loop

    private func startSessionLoop() {
        controller.startSession()
        sessionTimer?.invalidate()
        sessionTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
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
