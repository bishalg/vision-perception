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
    @State private var currentClipElapsedSec: Double = 0.0
    /// Authoritative AVPlayer media clock for the active clip (from the 10 Hz periodic observer).
    /// Falls back to timer accumulation when no video is playing (procedural/fallback stimuli).
    @State private var currentClipMediaTimeSec: Double = 0.0
    @State private var currentClipIndex: Int = 0
    @State private var cartoonBlink: Bool = false
    @State private var cartoonBounce: CGFloat = 0.0
    @State private var showAbortConfirm: Bool = false
    @State private var entryBlock: DeviceBlockReason?
    @StateObject private var camera = CameraAccessCoordinator()
    /// SIM-2/SIM-7: why capture cannot run right now, if it cannot.
    private var activeBlock: DeviceBlockReason? { entryBlock ?? controller.blockReason }

    public let playlist: [StimulusPlaylistItem]
    public let proceduralMode: ProceduralStimulusMode?

    // Callbacks
    #if ASD_RESEARCH_CLASSIFIER
    public let onComplete: (GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult?) -> Void
    #else
    public let onComplete: (GazeSessionReportSummary, [GazeFrame]) -> Void
    #endif
    public let onAbort: () -> Void

    public var sessionDurationSec: Double {
        let total = playlist.reduce(0.0) { $0 + $1.targetDurationSec }
        return total > 0 ? total : 30.0
    }

    private var currentClip: StimulusPlaylistItem? {
        guard currentClipIndex >= 0 && currentClipIndex < playlist.count else { return nil }
        return playlist[currentClipIndex]
    }

    #if ASD_RESEARCH_CLASSIFIER
    public init(
        playlist: [StimulusPlaylistItem] = StimulusPlaylistBattery.standard30s,
        proceduralMode: ProceduralStimulusMode? = nil,
        onComplete: @escaping (GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult?) -> Void,
        onAbort: @escaping () -> Void
    ) {
        self.playlist = playlist.isEmpty ? StimulusPlaylistBattery.standard30s : playlist
        self.proceduralMode = proceduralMode
        self.onComplete = onComplete
        self.onAbort = onAbort
    }

    public init(
        videoName: String = "stim_05_geopref_split_10s",
        proceduralMode: ProceduralStimulusMode? = nil,
        onComplete: @escaping (GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult?) -> Void,
        onAbort: @escaping () -> Void
    ) {
        self.playlist = [
            StimulusPlaylistItem(
                videoName: videoName,
                title: videoName.replacingOccurrences(of: "_", with: " ").capitalized,
                category: "Clinical Stimulus",
                targetDurationSec: 30.0
            )
        ]
        self.proceduralMode = proceduralMode
        self.onComplete = onComplete
        self.onAbort = onAbort
    }
    #else
    public init(
        playlist: [StimulusPlaylistItem] = StimulusPlaylistBattery.standard30s,
        proceduralMode: ProceduralStimulusMode? = nil,
        onComplete: @escaping (GazeSessionReportSummary, [GazeFrame]) -> Void,
        onAbort: @escaping () -> Void
    ) {
        self.playlist = playlist.isEmpty ? StimulusPlaylistBattery.standard30s : playlist
        self.proceduralMode = proceduralMode
        self.onComplete = onComplete
        self.onAbort = onAbort
    }

    public init(
        videoName: String = "stim_05_geopref_split_10s",
        proceduralMode: ProceduralStimulusMode? = nil,
        onComplete: @escaping (GazeSessionReportSummary, [GazeFrame]) -> Void,
        onAbort: @escaping () -> Void
    ) {
        self.playlist = [
            StimulusPlaylistItem(
                videoName: videoName,
                title: videoName.replacingOccurrences(of: "_", with: " ").capitalized,
                category: "Clinical Stimulus",
                targetDurationSec: 30.0
            )
        ]
        self.proceduralMode = proceduralMode
        self.onComplete = onComplete
        self.onAbort = onAbort
    }
    #endif

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
                    let topSafeInset = max(geo.safeAreaInsets.top, 44.0)
                    let bottomSafeInset = max(geo.safeAreaInsets.bottom, 16.0)

                    VStack(spacing: 0) {
                        topControlsBar
                            .padding(.horizontal, 16)
                            .padding(.top, topSafeInset + 8)

                        Spacer()

                        bottomEmotionHUD
                            .padding(.horizontal, 16)
                            .padding(.bottom, bottomSafeInset + 10)
                    }
                }
            }
        }
        .ignoresSafeArea()
        .overlay {
            if camera.state.showsDeniedView {
                CameraDeniedView(state: camera.state, onClose: onAbort)
            } else if let reason = activeBlock {
                UnsupportedDeviceView(reason: reason, onClose: onAbort)
            }
        }
        .task { await startSession() }
        .onDisappear { endCapture() }
        .cameraScenePhase(camera) { action in
            // CAM-3: backgrounding stops the session. An observation cannot be resumed
            // mid-stimulus, so it ends honestly as "interrupted" instead of continuing.
            if action == .stopSession {
                endCapture()
                entryBlock = .interrupted
            }
        }
        .confirmationDialog(
            "Stop Observation?",
            isPresented: $showAbortConfirm,
            titleVisibility: .visible
        ) {
            Button("Abort & Return to Start", role: .destructive) {
                endCapture()
                onAbort()
            }
            Button("Continue Observation", role: .cancel) {}
        } message: {
            Text("Session data collected so far will not be saved.")
        }
    }

    // MARK: - Top Controls Bar (Timer, Progress Bar, & Sequence Dots)

    private var topControlsBar: some View {
        HStack(alignment: .top, spacing: 8) {
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
                .foregroundColor(Color.white.opacity(0.85))
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .background(Capsule().fill(.ultraThinMaterial.opacity(0.75)))
                .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 0.8))
            }
            .buttonStyle(.plain)

            Spacer(minLength: 2)

            // Top-Center: Continuous Progress Bar + Dots for Multiple Videos
            VStack(spacing: 5) {
                // Continuous Session Progress Bar
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(Color.white.opacity(0.2))
                        .frame(width: 130, height: 5)
                    Capsule()
                        .fill(
                            LinearGradient(
                                colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                                startPoint: .leading,
                                endPoint: .trailing
                            )
                        )
                        .frame(
                            width: max(5, 130 * CGFloat(min(elapsedSeconds / max(1.0, sessionDurationSec), 1.0))),
                            height: 5
                        )
                        .shadow(color: LiquidGlass.emeraldMint.opacity(0.5), radius: 3)
                }

                // Numeric Timer Progress Display
                Text("\(Int(min(elapsedSeconds, sessionDurationSec)))s / \(Int(sessionDurationSec))s")
                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                    .foregroundColor(Color.white.opacity(0.85))

                // Progress Indicator Dots for Multiple Videos (Directly below timer progress)
                HStack(spacing: 6) {
                    ForEach(0..<playlist.count, id: \.self) { idx in
                        if idx < currentClipIndex {
                            // Completed clip: Solid Emerald Mint dot
                            Circle()
                                .fill(LiquidGlass.emeraldMint)
                                .frame(width: 6, height: 6)
                                .shadow(color: LiquidGlass.emeraldMint.opacity(0.6), radius: 2)
                        } else if idx == currentClipIndex {
                            // Current Active clip: Glowing elongated Celestial Teal capsule
                            Capsule()
                                .fill(LiquidGlass.celestialTeal)
                                .frame(width: 16, height: 6)
                                .shadow(color: LiquidGlass.celestialTeal.opacity(0.8), radius: 4)
                        } else {
                            // Upcoming clip: Translucent dot
                            Circle()
                                .fill(Color.white.opacity(0.3))
                                .frame(width: 6, height: 6)
                        }
                    }
                }

                // Active Clip Title / Category Tag
                if let clip = currentClip {
                    Text("\(currentClipIndex + 1)/\(playlist.count) • \(clip.title)")
                        .font(.system(size: 9, weight: .medium, design: .rounded))
                        .foregroundColor(Color.white.opacity(0.75))
                        .lineLimit(1)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(.ultraThinMaterial.opacity(0.65)))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.15), lineWidth: 0.8))

            Spacer(minLength: 2)

            // Top-Right: Time Value Badge
            HStack(spacing: 5) {
                Image(systemName: "stopwatch.fill")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundColor(LiquidGlass.emeraldMint)
                Text(String(format: "%02d:%02d",
                            Int(elapsedSeconds) / 60,
                            Int(elapsedSeconds) % 60))
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .monospacedDigit()
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(Capsule().fill(.ultraThinMaterial.opacity(0.75)))
            .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 0.8))
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

    // MARK: - Stimulus Content (Video / Procedural / Fallback)

    @ViewBuilder
    private func stimulusContent() -> some View {
        GeometryReader { geometry in
            ZStack {
                if let proceduralMode = proceduralMode {
                    ProceduralOculomotorStimulusView(
                        mode: proceduralMode,
                        elapsedSec: elapsedSeconds
                    )
                } else if let clip = currentClip,
                          let videoURL = OnDemandStimulusManager.shared.resolveVideoURL(name: clip.videoName) {
                    StimulusVideoPlayerView(
                        videoURL: videoURL,
                        isLooping: true,
                        onEnded: {
                            advanceToNextClipIfNeeded()
                        },
                        onMediaUpdate: { mediaTime in
                            currentClipMediaTimeSec = mediaTime
                            controller.updateClipMediaTime(mediaTime)
                        }
                    )
                    .id(clip.videoName)
                    .transition(.opacity.animation(.easeInOut(duration: 0.35)))
                } else {
                    // Fallback GeoPref Split-Screen: Social Actor vs Geometric Pattern
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

    private func startSession() async {
        // CAM-1: no session until camera access is granted.
        guard await camera.ensureAccess(consentStore: .shared) == .granted, !Task.isCancelled else {
            if camera.state == .unknown { entryBlock = .consentRequired }
            return
        }
        controller.reloadCalibrationProfile()
        currentClipIndex = 0
        currentClipElapsedSec = 0.0
        currentClipMediaTimeSec = 0.0
        elapsedSeconds = 0.0

        let initialBattery: StimulusAOITracker
        if let clip = currentClip {
            initialBattery = StimulusAOITracker.tracker(forVideoNamed: clip.videoName)
        } else {
            initialBattery = StimulusAOITracker.makeGeoPrefBattery()
        }

        // CNS-1: consent token required; SIM-2: no simulation fallback on unsupported hardware.
        guard controller.startSessionWithConsent(battery: initialBattery, stimulusId: currentClip?.videoName) else {
            entryBlock = .consentRequired
            return
        }
        entryBlock = nil
        guard controller.blockReason == nil else { return }
        camera.sessionDidStart()

        sessionTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
            MainActor.assumeIsolated {
                elapsedSeconds += 1.0 / 60.0
                currentClipElapsedSec += 1.0 / 60.0

                // Prefer the AVPlayer media clock once the periodic observer is live; the
                // wall-clock accumulation stays as fallback for procedural/fallback stimuli
                // and stalled playback (advance on whichever reports the clip duration first).
                let mediaClockActive = currentClipMediaTimeSec > 0.01
                let effectiveClipTime = mediaClockActive ? currentClipMediaTimeSec : currentClipElapsedSec

                #if ASD_SIMULATION_ENABLED
                if controller.isSimulationMode {
                    controller.tickSimulatedFrame(
                        timeSec: elapsedSeconds,
                        clipTimeSec: effectiveClipTime,
                        stimulusId: currentClip?.videoName,
                        clipMediaTimeSec: mediaClockActive ? currentClipMediaTimeSec : nil
                    )
                }
                #endif

                // Cartoon animation
                cartoonBounce = sin(elapsedSeconds * 4.2) * 9.0
                cartoonBlink = fmod(elapsedSeconds, 3.5) < 0.15

                // Advance to next video in playlist if target clip duration reached
                if let clip = currentClip,
                   currentClipElapsedSec >= clip.targetDurationSec || effectiveClipTime >= clip.targetDurationSec {
                    advanceToNextClipIfNeeded()
                }

                // Auto-complete when total session duration is reached
                if elapsedSeconds >= sessionDurationSec {
                    finishSession()
                }
            }
        }
    }

    private func advanceToNextClipIfNeeded() {
        guard currentClipIndex + 1 < playlist.count else {
            if elapsedSeconds >= sessionDurationSec {
                finishSession()
            }
            return
        }

        withAnimation(.easeInOut(duration: 0.3)) {
            currentClipIndex += 1
            currentClipElapsedSec = 0.0
            currentClipMediaTimeSec = 0.0
        }

        if let nextClip = currentClip {
            let nextBattery = StimulusAOITracker.tracker(forVideoNamed: nextClip.videoName)
            controller.switchStimulusClip(battery: nextBattery, stimulusId: nextClip.videoName)
        }
    }

    private func stopSession() {
        sessionTimer?.invalidate()
        sessionTimer = nil
    }

    /// Stops the timer and, if a capture is still running, the controller (idle timer restored).
    private func endCapture() {
        stopSession()
        guard camera.isSessionActive else { return }
        camera.sessionDidEnd()
        Task { _ = await controller.stopSession() }
    }

    private func finishSession() {
        stopSession()
        camera.sessionDidEnd()
        Task { await finalizeSession() }
    }

    private func finalizeSession() async {
        // `nil` when nothing recorded (live tracking unavailable): no sink receives
        // the empty summary in that case.
        let recordedSource = controller.captureSource
        let summary = await controller.stopSession()
        let frames = controller.recordedFrames
        #if ASD_RESEARCH_CLASSIFIER
        let classification = BiomarkerClassificationEngine.shared.classifySync(summary: summary)
        #endif
        let activeBatteryName = proceduralMode?.rawValue ?? (playlist.count > 1 ? "MultiClipBattery_\(playlist.count)Clips" : (currentClip?.videoName ?? "SingleClip"))

        // Per-stimulus expected-vs-recorded scoring against the bundled provisional baseline.
        // Extraction is linear over frames and runs once here; the result view reuses the tagged frames.
        let childAgeMonths = ChildProfileStore.shared.profile.ageBracket.midpointMonths
        let perVideoScore: PerVideoSessionScore? = NormativeBaselineLibrary.loadBundled().map { baseline in
            PerVideoScoringEngine.score(
                metricSets: PerVideoMetricsExtractor.extract(from: frames),
                baseline: baseline,
                ageMonths: childAgeMonths
            )
        }

        if recordedSource != nil {
            let participantToken = GazeSessionRecord.participantToken(
                enrollment: ResearchKitStudyStore.shared.fetchEnrollment(),
                sessionId: summary.sessionId
            )
            // Norm-collection contribution: only for consented + opted-in live TrueDepth sessions.
            let collectionMetadata = NormCollectionGate.collectionMetadata(
                enrollment: ResearchKitStudyStore.shared.fetchEnrollment(),
                childAgeMonths: childAgeMonths,
                captureSource: summary.captureSource
            )

            // History record (schema v1.6 carries the per-video matrix for trend + export).
            // Pseudonymous token, never the child's name (CNS-7); simulated sessions are
            // ignored by the store (SIM-5).
            let sessionRecord = GazeSessionRecord(
                summary: summary,
                participantToken: participantToken,
                perVideoComposite: perVideoScore?.compositeScore,
                perVideo: perVideoScore?.videos
            )
            let attentionFlag = perVideoScore.map { AttentionFlagAlgorithm.assess($0).flag }

            // Persist encrypted vault off main thread (frames now carry per-clip stimulus attribution)
            Task.detached(priority: .userInitiated) {
                let vault = SessionVaultManager()
                let header = SessionHeader(
                    sessionId: summary.sessionId,
                    participantToken: participantToken,
                    stimulusBattery: activeBatteryName,
                    stimulusDurationSec: summary.durationSec,
                    captureSource: summary.captureSource
                )
                #if ASD_RESEARCH_CLASSIFIER
                let framesFile = try? vault.saveSession(
                    header: header,
                    frames: frames,
                    report: summary,
                    classification: classification,
                    perVideoScore: perVideoScore,
                    collectionMetadata: collectionMetadata
                )
                #else
                let framesFile = try? vault.saveSession(
                    header: header,
                    frames: frames,
                    report: summary,
                    perVideoScore: perVideoScore,
                    collectionMetadata: collectionMetadata
                )
                #endif
                // WP-A.4: queue for /v1/sessions (consented, configured, live only;
                // the service refuses everything else). The frames blob stays
                // encrypted with the device key.
                await MainActor.run {
                    _ = ResearchKitSyncService.shared?.enqueueSession(
                        sessionRecord,
                        framesFile: framesFile,
                        attentionFlag: attentionFlag
                    )
                }
            }

            GazeSessionHistoryStore.shared.record(sessionRecord)

            // CareKit sync (both ignore simulated sessions, SIM-5; CarePlanStore mirrors outcome to CareKitClinicalStore)
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

        #if ASD_RESEARCH_CLASSIFIER
        onComplete(summary, frames, classification)
        #else
        onComplete(summary, frames)
        #endif
    }
}
