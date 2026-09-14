import SwiftUI
import CurvAsdGazeKit

// MARK: - Step 3: Full-Screen Stimulus Screening View

/// Wizard Step 3 — Full-screen clinical stimulus with invisible 60 Hz TrueDepth tracking.
///
/// Layout:
/// • Edge-to-edge full screen stimulus (zero child mirror preview, ETHICS_PROTOCOL §2)
/// • Top-center: Sleek continuous screening progress bar
/// • Top-right: Real-time session elapsed timer
/// • Bottom: Dedicated, expansive Real-Time Emotion & Facial Affect HUD
public struct GazeStimulusScreeningView: View {
    @StateObject private var controller = GazeSessionController()
    @State private var sessionTimer: Timer?
    @State private var elapsedSeconds: Double = 0.0
    @State private var cartoonBlink: Bool = false
    @State private var cartoonBounce: CGFloat = 0.0
    @State private var showAbortConfirm: Bool = false

    private let sessionDurationSec: Double = 30.0

    // Callbacks
    public let onComplete: (GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult) -> Void
    public let onAbort: () -> Void

    public init(
        onComplete: @escaping (GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult) -> Void,
        onAbort: @escaping () -> Void
    ) {
        self.onComplete = onComplete
        self.onAbort = onAbort
    }

    public var body: some View {
        ZStack {
            // Full-screen deep canvas
            Color.black.ignoresSafeArea()

            GeometryReader { geo in
                ZStack {
                    // 1. Full-Screen Clinical Stimulus
                    stimulusContent()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    // 2. Invisible TrueDepth camera tracking session kept alive
                    ARFaceCameraView(
                        session: controller.arSession,
                        isSimulation: controller.isSimulationMode,
                        headYawDeg: controller.headYawDeg,
                        headPitchDeg: controller.headPitchDeg,
                        smileIntensity: controller.currentAffect.socialSmileIntensity,
                        surpriseScore: controller.currentAffect.surpriseScore,
                        distressScore: controller.currentAffect.distressScore,
                        dominantEmotion: controller.currentAffect.dominantEmotion,
                        isValid: controller.currentQuality.isValid
                    )
                    .frame(width: 1, height: 1)
                    .opacity(0.001)
                    .allowsHitTesting(false)

                    // 3. Floating HUD Overlays (Top Controls + Bottom Emotion HUD)
                    VStack {
                        topControlsBar
                            .padding(.horizontal, 16)
                            .padding(.top, 14)

                        Spacer()

                        bottomEmotionHUD
                            .padding(.horizontal, 16)
                            .padding(.bottom, 22)
                    }
                }
            }
        }
        .ignoresSafeArea()
        .onAppear(perform: startSession)
        .onDisappear(perform: stopSession)
        .confirmationDialog(
            "Abort Screening?",
            isPresented: $showAbortConfirm,
            titleVisibility: .visible
        ) {
            Button("Abort & Return to Start", role: .destructive) {
                stopSession()
                onAbort()
            }
            Button("Continue Screening", role: .cancel) {}
        } message: {
            Text("Session data collected so far will not be saved.")
        }
    }

    // MARK: - Top Controls Bar (Timer at Top-Right, Progress Bar at Top-Center)

    private var topControlsBar: some View {
        HStack(alignment: .center, spacing: 12) {
            // Top-Left: Discreet Caregiver Exit / Abort Button
            Button {
                showAbortConfirm = true
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "xmark")
                        .font(.system(size: 11, weight: .bold))
                    Text("Exit")
                        .font(.system(size: 11, weight: .semibold, design: .rounded))
                }
                .foregroundColor(Color.white.opacity(0.75))
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Capsule().fill(.ultraThinMaterial.opacity(0.7)))
                .overlay(Capsule().stroke(Color.white.opacity(0.15), lineWidth: 0.8))
            }
            .buttonStyle(.plain)

            Spacer()

            // Top-Center: Continuous Progress Bar
            VStack(spacing: 3) {
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(Color.white.opacity(0.2))
                        .frame(width: 130, height: 6)
                    Capsule()
                        .fill(
                            LinearGradient(
                                colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                                startPoint: .leading,
                                endPoint: .trailing
                            )
                        )
                        .frame(width: max(6, 130 * CGFloat(min(elapsedSeconds / sessionDurationSec, 1.0))), height: 6)
                        .shadow(color: LiquidGlass.emeraldMint.opacity(0.5), radius: 3)
                }

                Text("\(Int(min(elapsedSeconds, sessionDurationSec)))s / \(Int(sessionDurationSec))s")
                    .font(.system(size: 9, weight: .bold, design: .monospaced))
                    .foregroundColor(Color.white.opacity(0.7))
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Capsule().fill(.ultraThinMaterial.opacity(0.5)))

            Spacer()

            // Top-Right: Time Value Badge
            HStack(spacing: 5) {
                Image(systemName: "stopwatch.fill")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundColor(LiquidGlass.emeraldMint)
                Text(String(format: "%02d:%02d",
                            Int(elapsedSeconds) / 60,
                            Int(elapsedSeconds) % 60))
                    .font(.system(size: 13, weight: .bold, design: .monospaced))
                    .monospacedDigit()
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Capsule().fill(.ultraThinMaterial.opacity(0.7)))
            .overlay(Capsule().stroke(Color.white.opacity(0.15), lineWidth: 0.8))
        }
    }

    // MARK: - Bottom Emotion & Facial Affect HUD (Dedicated Spacious Area)

    private var bottomEmotionHUD: some View {
        HStack(spacing: 14) {
            // Emotion Avatar Icon with Dynamic Halo
            ZStack {
                Circle()
                    .fill(emotionAccentColor.opacity(0.2))
                    .frame(width: 48, height: 48)
                Circle()
                    .stroke(emotionAccentColor.opacity(0.6), lineWidth: 1.5)
                    .frame(width: 48, height: 48)
                Text(controller.currentAffect.dominantEmotion.emoji)
                    .font(.system(size: 26))
            }

            // Emotion Labels and Real-Time Biomarker Meters
            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 6) {
                    Text(controller.currentAffect.dominantEmotion.rawValue)
                        .font(.system(size: 14, weight: .bold, design: .rounded))
                        .foregroundColor(.white)

                    Circle()
                        .fill(controller.currentQuality.isValid ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                        .frame(width: 6, height: 6)

                    Spacer()

                    Text(controller.currentQuality.isValid ? "Tracked 60Hz" : "Aligning...")
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundColor(controller.currentQuality.isValid ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                }

                // Affect Breakdown Meters
                HStack(spacing: 16) {
                    // Smile Intensity Meter
                    HStack(spacing: 5) {
                        Text("Smile:")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(Color.white.opacity(0.65))

                        GeometryReader { g in
                            ZStack(alignment: .leading) {
                                Capsule().fill(Color.white.opacity(0.2))
                                Capsule()
                                    .fill(LiquidGlass.emeraldMint)
                                    .frame(width: g.size.width * CGFloat(min(controller.currentAffect.socialSmileIntensity, 1.0)))
                            }
                        }
                        .frame(width: 50, height: 4)

                        Text("\(Int(controller.currentAffect.socialSmileIntensity * 100))%")
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                            .foregroundColor(LiquidGlass.emeraldMint)
                    }

                    // Visual Engagement / Attention Meter
                    HStack(spacing: 5) {
                        Text("Attention:")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(Color.white.opacity(0.65))

                        GeometryReader { g in
                            ZStack(alignment: .leading) {
                                Capsule().fill(Color.white.opacity(0.2))
                                Capsule()
                                    .fill(LiquidGlass.celestialTeal)
                                    .frame(width: g.size.width * CGFloat(min(controller.currentAffect.surpriseScore, 1.0)))
                            }
                        }
                        .frame(width: 50, height: 4)

                        Text("\(Int(controller.currentAffect.surpriseScore * 100))%")
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(.ultraThinMaterial.opacity(0.72))
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(LiquidGlass.cardBorder, lineWidth: 0.8)
        )
        .shadow(color: Color.black.opacity(0.3), radius: 10, y: 5)
    }

    private var emotionAccentColor: Color {
        switch controller.currentAffect.dominantEmotion {
        case .joy, .excited:
            return LiquidGlass.emeraldMint
        case .surprise, .calm:
            return LiquidGlass.celestialTeal
        case .distress:
            return LiquidGlass.warmAmber
        }
    }

    // MARK: - Stimulus Content (GeoPref Split-Screen)

    @ViewBuilder
    private func stimulusContent() -> some View {
        GeometryReader { geometry in
            ZStack {
                // GeoPref Split-Screen: Social Actor vs Geometric Pattern
                HStack(spacing: 0) {
                    // Left: Social scene (Actor)
                    ZStack {
                        LinearGradient(
                            colors: [LiquidGlass.stimulusSocialTop, LiquidGlass.stimulusSocialBottom],
                            startPoint: .top, endPoint: .bottom
                        )
                        VStack(spacing: 14) {
                            // Animated cartoon character (child-facing stimulus)
                            ZStack {
                                Circle()
                                    .fill(RadialGradient(
                                        colors: [Color.yellow, Color.orange],
                                        center: .center, startRadius: 10, endRadius: 50
                                    ))
                                    .frame(width: 100, height: 100)
                                    .shadow(color: Color.orange.opacity(0.5), radius: 18)

                                VStack(spacing: 8) {
                                    HStack(spacing: 20) {
                                        Circle().fill(Color.black)
                                            .frame(width: 9, height: cartoonBlink ? 2 : 14)
                                        Circle().fill(Color.black)
                                            .frame(width: 9, height: cartoonBlink ? 2 : 14)
                                    }
                                    Capsule().fill(Color.red).frame(width: 26, height: 9)
                                }
                            }
                            .offset(y: cartoonBounce)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                    // Right: Dynamic Geometric Pattern
                    ZStack {
                        LinearGradient(
                            colors: [LiquidGlass.stimulusGeometryTop, LiquidGlass.stimulusGeometryBottom],
                            startPoint: .top, endPoint: .bottom
                        )
                        VStack(spacing: 14) {
                            Image(systemName: "hexagon.fill")
                                .font(.system(size: 54))
                                .foregroundColor(LiquidGlass.celestialTeal.opacity(0.8))
                                .rotationEffect(.degrees(elapsedSeconds * 60))

                            Image(systemName: "star.fill")
                                .font(.system(size: 28))
                                .foregroundColor(LiquidGlass.warmAmber.opacity(0.75))
                                .rotationEffect(.degrees(-elapsedSeconds * 90))
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }

                // Real-time gaze reticle (caregiver feedback)
                Circle()
                    .stroke(LiquidGlass.celestialTeal, lineWidth: 3)
                    .background(Circle().fill(LiquidGlass.celestialTeal.opacity(0.3)))
                    .frame(width: 28, height: 28)
                    .position(
                        x: controller.currentGazePoint.x * geometry.size.width,
                        y: controller.currentGazePoint.y * geometry.size.height
                    )
                    .shadow(color: LiquidGlass.celestialTeal, radius: 8)
                    .opacity(controller.currentQuality.isValid ? 0.85 : 0.0)
            }
        }
    }

    // MARK: - Session Management

    private func startSession() {
        controller.reloadCalibrationProfile()
        controller.startSession()
        elapsedSeconds = 0
        sessionTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
            MainActor.assumeIsolated {
                elapsedSeconds += 1.0 / 60.0

                if controller.isSimulationMode {
                    controller.tickSimulatedFrame(timeSec: elapsedSeconds)
                }

                // Cartoon animation
                cartoonBounce = sin(elapsedSeconds * 4.2) * 9.0
                cartoonBlink = fmod(elapsedSeconds, 3.5) < 0.15

                // Auto-complete at 30 s
                if elapsedSeconds >= sessionDurationSec {
                    finishSession()
                }
            }
        }
    }

    private func stopSession() {
        sessionTimer?.invalidate()
        sessionTimer = nil
    }

    private func finishSession() {
        stopSession()
        let summary = controller.stopSession()
        let frames = controller.recordedFrames
        let classification = BiomarkerClassificationEngine.shared.classifySync(summary: summary)

        // Persist encrypted vault off main thread
        Task.detached(priority: .userInitiated) {
            let vault = SessionVaultManager()
            let header = SessionHeader(
                sessionId: summary.sessionId,
                participantToken: "child-\(summary.sessionId.prefix(6))",
                stimulusBattery: "GeoPref_v1",
                stimulusDurationSec: summary.durationSec
            )
            _ = try? vault.saveSession(
                header: header,
                frames: frames,
                report: summary,
                classification: classification
            )
        }

        // CareKit sync
        CarePlanStore.shared.markScreeningCompleted(socialScore: summary.socialPreferenceScore)
        CareKitClinicalStore.shared.recordScreeningOutcome(
            socialScore: summary.socialPreferenceScore,
            validFramePercentage: summary.validRatio * 100.0,
            knownTasks: CarePlanStore.shared.tasks,
            riskScore: classification.riskScore,
            riskStratification: classification.riskStratification.rawValue
        )

        onComplete(summary, frames, classification)
    }
}
