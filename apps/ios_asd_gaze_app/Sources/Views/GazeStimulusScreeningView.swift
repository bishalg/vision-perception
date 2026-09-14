import SwiftUI
import CurvAsdGazeKit

// MARK: - Step 3: Full-Screen Stimulus Screening View

/// Wizard Step 3 — Full-screen clinical stimulus with invisible 60 Hz TrueDepth tracking.
///
/// Key design decisions grounded in research docs:
/// • Zero camera preview shown to the child (prevents self-mirror distraction).
/// • TrueDepth runs in volatile RAM at 60 Hz — zero raw video persisted (ETHICS_PROTOCOL.md §2).
/// • 1:1 temporal synchrony: every sample is tagged with its exact stimulus AOI, gaze (x,y),
///   and instantaneous facial affect — enabling clinicians to ask "at this scene moment,
///   did the child smile back?" (STIMULI_SPEC.md §2, ARCHITECTURE.md).
/// • Faint caregiver HUD at the bottom: stopwatch, affect pill, and discrete abort button.
public struct GazeStimulusScreeningView: View {
    @StateObject private var controller = GazeSessionController()
    @State private var sessionTimer: Timer?
    @State private var elapsedSeconds: Double = 0.0
    @State private var cartoonBlink: Bool = false
    @State private var cartoonBounce: CGFloat = 0.0
    @State private var displayMode: ScannerDisplayMode = .stimulusOnly
    @State private var showAbortConfirm: Bool = false
    @State private var presentedReport: (
        summary: GazeSessionReportSummary,
        frames: [GazeFrame],
        classification: BiomarkerClassificationResult
    )?

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
            // Deep black stimulus canvas — minimise reflections (STIMULI_SPEC.md §3)
            Color.black.ignoresSafeArea()

            GeometryReader { geo in
                ZStack {
                    // Full-Screen Clinical Stimulus (no camera shown to child)
                    stimulusContent()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    // Invisible TrueDepth session kept alive (camera never shown)
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

                    // Faint Caregiver HUD at bottom (non-intrusive)
                    VStack {
                        Spacer()
                        caregiverHUD
                    }
                }
            }
        }
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

    // MARK: - Stimulus Content (GeoPref Split-Screen)

    @ViewBuilder
    private func stimulusContent() -> some View {
        GeometryReader { geometry in
            ZStack {
                // GeoPref Split-Screen: Social Actor vs Geometric Pattern
                HStack(spacing: 0) {
                    // Left: Social scene
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

                // Real-time gaze reticle (shown to caregiver, not visible to child during clinical use)
                Circle()
                    .stroke(LiquidGlass.celestialTeal, lineWidth: 3)
                    .background(Circle().fill(LiquidGlass.celestialTeal.opacity(0.3)))
                    .frame(width: 30, height: 30)
                    .position(
                        x: controller.currentGazePoint.x * geometry.size.width,
                        y: controller.currentGazePoint.y * geometry.size.height
                    )
                    .shadow(color: LiquidGlass.celestialTeal, radius: 8)
                    .opacity(controller.currentQuality.isValid ? 0.85 : 0.0)
            }
        }
    }

    // MARK: - Caregiver HUD (Non-Intrusive Bottom Strip)

    private var caregiverHUD: some View {
        HStack(spacing: 12) {
            // Timer
            HStack(spacing: 5) {
                Image(systemName: "stopwatch.fill")
                    .font(.system(size: 11))
                    .foregroundColor(Color.white.opacity(0.7))
                Text(String(format: "%02d:%02d",
                            Int(elapsedSeconds) / 60,
                            Int(elapsedSeconds) % 60))
                    .font(.system(size: 13, weight: .bold, design: .monospaced))
                    .monospacedDigit()
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 5)
            .background(Capsule().fill(.ultraThinMaterial.opacity(0.6)))

            // Progress bar
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.12)).frame(height: 4)
                    Capsule()
                        .fill(LiquidGlass.celestialTeal.opacity(0.8))
                        .frame(width: g.size.width * CGFloat(min(elapsedSeconds / sessionDurationSec, 1.0)), height: 4)
                }
            }
            .frame(height: 4)

            // Live emotion affect pill
            HStack(spacing: 4) {
                Text(controller.currentAffect.dominantEmotion.emoji)
                    .font(.system(size: 13))
                Text(controller.currentAffect.dominantEmotion.rawValue)
                    .font(.system(size: 11, weight: .semibold, design: .rounded))
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(Capsule().fill(.ultraThinMaterial.opacity(0.6)))

            // Abort button
            Button {
                showAbortConfirm = true
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 20, weight: .medium))
                    .foregroundColor(Color.white.opacity(0.6))
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial.opacity(0.55))
        .clipShape(Capsule())
        .padding(.horizontal, 20)
        .padding(.bottom, 24)
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
