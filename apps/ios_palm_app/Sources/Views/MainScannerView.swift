import SwiftUI
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Live guidance camera with iOS Camera-style last-capture thumbnail (bottom-left).
 * - Reticle contains center-top HandSideToggleView and dynamic silhouette.
 * - Tap thumbnail to open the crease overlay review and precision editor.
 * - Camera feed is automatically stopped during review and reading to conserve battery.
 */
public struct MainScannerView: View {
    @StateObject private var viewModel = PalmAnalysisViewModel()

    /// Drives the post-capture "hero" flight: a large centered flash of the
    /// still shrinks into the bottom-left filmstrip slot, matching the shared
    /// `captureHero` id below. See CaptureFilmstripThumbnail at file bottom.
    @Namespace private var captureHeroNamespace
    @State private var showCaptureFlash = false
    @State private var flashImage: UIImage?
    @State private var filmstripShakeTicks: CGFloat = 0

    public init() {}

    /// Kicks off the capture -> filmstrip flight for the newest shot. Held
    /// briefly at full size (so the capture visibly registers) before the
    /// slow, emphasized shrink into the thumbnail well; the filmstrip shakes
    /// once it lands so a fast multi-shot session still reads as "added".
    private func animateNewCapture() {
        guard let newest = viewModel.capturedShots.last else { return }
        flashImage = newest
        showCaptureFlash = true
        let flightDuration = 0.68
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
            withAnimation(.spring(response: flightDuration, dampingFraction: 0.78)) {
                showCaptureFlash = false
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15 + flightDuration) {
            flashImage = nil
            withAnimation(.default) {
                filmstripShakeTicks += 1
            }
        }
    }

    /// Live landmarks, or the geometry scaffold on simulator (no camera).
    private var regionLandmarks: [CGPoint] {
        if !viewModel.detectedLandmarks.isEmpty { return viewModel.detectedLandmarks }
        #if targetEnvironment(simulator)
        return PalmRegionGeometry.scaffoldLandmarks()
        #else
        return []
        #endif
    }

    public var body: some View {
        GeometryReader { geometry in
            let cameraFeedSize = CGSize(width: 1080, height: 1920)
            let imageRect = ImageCoordinateMapper.imageRect(
                imageSize: cameraFeedSize,
                viewSize: geometry.size,
                mode: .fill
            )
            let isScaffold = viewModel.detectedLandmarks.isEmpty
            // Only mirror synthetic scaffold hand on simulator if Left Hand is selected.
            // Live landmarks from camera are already in image space and anatomically invariant.
            let shouldMirror = isScaffold && (viewModel.selectedHandSide == .left || viewModel.detectedHandSide == .left)

            ZStack {
                CameraOverlayHost(
                    profile: viewModel.profile,
                    session: viewModel.cameraSession,
                    guidanceState: viewModel.guidanceState,
                    metrics: viewModel.qualityMetrics,
                    isAnalyzing: viewModel.isAnalyzing,
                    statusMessageOverride: viewModel.statusMessageOverride,
                    onManualCapture: { viewModel.manualCapture() },
                    // Orbit mode auto-captures keyframes: the still shutter would
                    // fire a single-photo analysis mid-orbit, so it is replaced
                    // by the OrbitScanBottomCard below.
                    showsShutter: viewModel.scanMode != .orbitScan,
                    bottomLeading: {
                        LeftHandSideButton(
                            selection: $viewModel.selectedHandSide,
                            autoDetected: viewModel.detectedHandSide
                        )
                    },
                    bottomTrailing: {
                        RightHandSideButton(
                            selection: $viewModel.selectedHandSide,
                            autoDetected: viewModel.detectedHandSide
                        )
                    }
                ) {
                    // Inside the reticle box: silhouette artwork only.
                    // NOTE: this subtree renders under allowsHitTesting(false)
                    // (see LiquidGlassHUD), so no tappable controls may live here.
                    PalmSilhouetteView(
                        isReady: viewModel.guidanceState.isReady,
                        handSide: viewModel.selectedHandSide,
                        detectedIsLeft: viewModel.detectedHandSide == .left,
                        isHandDetected: !viewModel.detectedLandmarks.isEmpty
                    )
                }

                // 3D Orbit Scan Guidance HUD (animated arrows, % progress, angle pips, coaching).
                // Purely visual — hit testing stays off so it can never cover the mode capsule.
                if viewModel.scanMode == .orbitScan && !viewModel.showResultViewer {
                    OrbitGuidanceOverlayView(
                        session: viewModel.orbitSession,
                        state: viewModel.orbitState
                    )
                    .allowsHitTesting(false)
                    .zIndex(2)
                }

                // Capture-mode capsule (Still / 3D Orbit / Close-Up / Fingertips).
                // Deliberately a sibling of CameraOverlayHost, floating over the
                // reticle's top edge: inside the reticle it would be visible but
                // untappable because the HUD reticle subtree is allowsHitTesting(false).
                VStack(spacing: 8) {
                    CameraModeSelectorView(
                        selectedMode: viewModel.scanMode,
                        onSelect: { viewModel.setScanMode($0) }
                    )
                    // Orbit direction chip: the center arrow sits behind the
                    // user's hand, so the "which way" cue lives here at top.
                    if viewModel.scanMode == .orbitScan && !viewModel.showResultViewer {
                        OrbitDirectionChip(session: viewModel.orbitSession, state: viewModel.orbitState)
                    }
                    Spacer()
                }
                .padding(.top, geometry.size.height * (1 - viewModel.profile.reticleHeightFraction) / 2 + 12)
                .zIndex(2)

                // Live landmark skeleton while hand is detected
                if !viewModel.detectedLandmarks.isEmpty && !viewModel.showResultViewer {
                    HandOutlineOverlay(
                        landmarks: viewModel.detectedLandmarks,
                        imageRect: imageRect
                    )
                    .ignoresSafeArea()
                    .allowsHitTesting(false)
                    .zIndex(1)
                }

                // Live sci-fi region overlay (mounts).
                if !regionLandmarks.isEmpty && !viewModel.showResultViewer {
                    RegionOverlayView(
                        landmarks: regionLandmarks,
                        isReady: viewModel.scanMode == .stillPhoto ? viewModel.guidanceState.isReady : viewModel.orbitSession.coverageFraction > 0.3,
                        showCorridors: false,
                        mirror: shouldMirror,
                        imageRect: imageRect,
                        mountReliefHeat: viewModel.mountReliefHeat
                    )
                    .ignoresSafeArea()
                    .zIndex(1)
                }

                // Bottom-left filmstrip thumbnail (like stock iOS Camera), with a
                // session badge (e.g. 1/2 for left+right palm) that turns green
                // once the required shot count is met.
                if let thumb = viewModel.capturedShots.last, !viewModel.showResultViewer, !showCaptureFlash {
                    VStack {
                        Spacer()
                        HStack {
                            Button(action: { viewModel.openLastCapture() }) {
                                CaptureFilmstripThumbnail(
                                    image: thumb,
                                    count: viewModel.capturedShots.count,
                                    requiredCount: viewModel.requiredShotCount,
                                    shakeTicks: filmstripShakeTicks
                                )
                            }
                            .matchedGeometryEffect(id: "captureHero", in: captureHeroNamespace)
                            .padding(.leading, 22)
                            .padding(.bottom, 46)
                            Spacer()
                        }
                    }
                    .transition(.scale.combined(with: .opacity))
                    .zIndex(2)
                }

                // Bottom 3D-scan card: replaces the still shutter while in orbit
                // mode. One plain instruction at a time — no palmistry jargon.
                if viewModel.scanMode == .orbitScan && !viewModel.showResultViewer {
                    VStack {
                        Spacer()
                        OrbitScanBottomCard(
                            session: viewModel.orbitSession,
                            state: viewModel.orbitState,
                            onRestart: { viewModel.setScanMode(.orbitScan) }
                        )
                        .padding(.horizontal, 16)
                        .padding(.bottom, 34)
                    }
                    .zIndex(2)
                }

                // Post-capture "hero" flash: briefly full-size, then shrinks
                // (slowly, deliberately — like the stock Camera app) into the
                // filmstrip slot above via the shared matchedGeometryEffect id.
                if showCaptureFlash, let flashImage {
                    Image(uiImage: flashImage)
                        .resizable()
                        .scaledToFill()
                        .frame(
                            width: showCaptureFlash ? geometry.size.width * 0.62 : 54,
                            height: showCaptureFlash ? geometry.size.width * 0.62 : 54
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 18, style: .continuous)
                                .stroke(Color.white.opacity(0.9), lineWidth: 2)
                        )
                        .shadow(color: .black.opacity(0.5), radius: 20, y: 6)
                        .matchedGeometryEffect(id: "captureHero", in: captureHeroNamespace)
                        .position(x: geometry.size.width / 2, y: geometry.size.height / 2)
                        .allowsHitTesting(false)
                        .zIndex(4)
                }

                // Phase 2: Crease Line Verification Screen
                if viewModel.showResultViewer {
                    CaptureResultView(
                        image: viewModel.capturedImage,
                        landmarks: viewModel.captureLandmarks,
                        splines: $viewModel.detectedSplines,
                        selectedLanguage: viewModel.currentLanguage,
                        overlaySource: viewModel.overlaySource,
                        handSide: viewModel.selectedHandSide == .auto ? viewModel.detectedHandSide : viewModel.selectedHandSide,
                        padDepths: viewModel.padDepths,
                        onConfirmAdjustedLines: { viewModel.confirmAdjustedLinesForPrediction() },
                        onDismiss: { viewModel.dismissResultViewer() },
                        onRetake: { viewModel.retake() },
                        onProceedToAnalysis: { viewModel.proceedToAnalysis() }
                    )
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .zIndex(3)
                }
            }
            // The viewfinder canvas is a media surface (stays video-dark);
            // all chrome around it follows the system theme on launch.
        }
        .ignoresSafeArea()
        .animation(.easeInOut(duration: 0.22), value: viewModel.showResultViewer)
        .onChange(of: viewModel.captureTick) { _ in animateNewCapture() }
        // Phase 3: Dedicated Samudrika Reading Sheet (native sheet chrome)
        .sheet(isPresented: $viewModel.showAnalysisReading) {
            SamudrikaReadingSheet(
                result: viewModel.analysisResult,
                splines: viewModel.predictionReadySplines.isEmpty ? viewModel.detectedSplines : viewModel.predictionReadySplines,
                handSide: viewModel.selectedHandSide == .auto ? viewModel.detectedHandSide : viewModel.selectedHandSide,
                selectedLanguage: viewModel.currentLanguage,
                onLanguageChange: { viewModel.currentLanguage = $0 },
                onRetake: { viewModel.retake() }
            )
            .presentationDetents([.medium, .large])
            .presentationDragIndicator(.visible)
        }
        // Camera Session Lifecycle Management (Stop feed to conserve battery during review & reading)
        .onChange(of: viewModel.showResultViewer) { showing in
            if showing {
                viewModel.cameraSession.stop()
            } else if !viewModel.showAnalysisReading {
                viewModel.cameraSession.start()
            }
        }
        .onChange(of: viewModel.showAnalysisReading) { showing in
            if showing {
                viewModel.cameraSession.stop()
            } else if !viewModel.showResultViewer {
                viewModel.cameraSession.start()
            }
        }
        .onAppear { viewModel.start() }
        .onDisappear { viewModel.stop() }
    }
}

/// Bottom-left "last capture" thumbnail with a session badge — like the stock
/// iOS Camera roll well, but tracking a multi-shot session (left/right palm
/// today; a future angle, e.g. back of hand, just bumps `requiredCount`).
/// The badge and border both go green once `count >= requiredCount`.
private struct CaptureFilmstripThumbnail: View {
    let image: UIImage
    let count: Int
    let requiredCount: Int
    /// Bumped by the parent once the capture "lands"; drives the shake.
    let shakeTicks: CGFloat

    private var isComplete: Bool { count >= requiredCount }
    private var boxColor: Color { isComplete ? OverlayPalette.emerald : Color.white.opacity(0.85) }
    private var badgeColor: Color { isComplete ? OverlayPalette.emerald : Color.black.opacity(0.78) }

    var body: some View {
        Image(uiImage: image)
            .resizable()
            .scaledToFill()
            .frame(width: 54, height: 54)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(boxColor, lineWidth: isComplete ? 2.2 : 1.5)
            )
            .shadow(color: (isComplete ? OverlayPalette.emerald : .black).opacity(isComplete ? 0.55 : 0.45), radius: 8, y: 3)
            .overlay(alignment: .topTrailing) {
                Text("\(count)")
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
                    .frame(minWidth: 18, minHeight: 18)
                    .background(Circle().fill(badgeColor))
                    .overlay(Circle().stroke(Color.white.opacity(0.9), lineWidth: 1))
                    .offset(x: 8, y: -8)
            }
            .modifier(ShakeEffect(animatableData: shakeTicks))
            .animation(.easeInOut(duration: 0.25), value: isComplete)
    }
}

/// Classic damped horizontal shake driven entirely by SwiftUI's animatable-
/// data interpolation (no Timer, no manual step tracking). Caller bumps an
/// integer-valued `CGFloat` inside `withAnimation` to trigger one shake.
private struct ShakeEffect: GeometryEffect {
    var travel: CGFloat = 6
    var oscillations: Double = 3
    var animatableData: CGFloat

    func effectValue(size: CGSize) -> ProjectionTransform {
        let dx = travel * sin(Double(animatableData) * .pi * oscillations)
        return ProjectionTransform(CGAffineTransform(translationX: dx, y: 0))
    }
}

/**
 * Bottom card shown instead of the still shutter during 3D Orbit mode.
 * Top HUD keeps progress + angle pips; the center keeps the direction
 * arrow; this card owns the single current instruction in plain words.
 */
struct OrbitScanBottomCard: View {
    let session: OrbitCaptureSession
    let state: OrbitGuidanceState
    let onRestart: () -> Void

    private var stepNumber: Int { min(session.filledBins.count + 1, OrbitBin.allCases.count) }

    private var instruction: String {
        switch state {
        case .idle, .searching:
            return "Show your open palm"
        case .lockPose:
            return "Move your palm to the center"
        case .fitHand:
            return "Show your whole hand"
        case .moveCloser:
            return "Move your hand closer"
        case .addLight:
            return "Add more light"
        case .coach(let bin), .holdSteady(let bin, _), .capturing(let bin):
            return bin.displayName
        case .rejected(let reason):
            return reason
        case .complete:
            return "Scan complete"
        }
    }

    private var statusLine: String {
        switch state {
        case .idle:
            return "Get ready…"
        case .searching:
            return "Looking for your hand…"
        case .lockPose:
            return "Center it in the frame"
        case .fitHand:
            return "Pull back — keep every fingertip visible"
        case .moveCloser:
            return "Hold it nearer the camera"
        case .addLight:
            return "Brighter spot, less glare"
        case .coach:
            return "Move slowly into position"
        case .holdSteady(_, let p):
            return "Hold steady… \(Int(p * 100))%"
        case .capturing:
            return "Saving this angle…"
        case .rejected:
            return "Adjust and hold still"
        case .complete:
            return "Computing your 3D map…"
        }
    }

    private var statusColor: Color {
        switch state {
        case .holdSteady, .capturing, .complete:
            return OverlayPalette.emerald
        case .rejected:
            return .yellow
        default:
            return .white.opacity(0.85)
        }
    }

    var body: some View {
        VStack(spacing: 8) {
            // Progress + angle pips, moved down from the top of the screen.
            OrbitProgressHeaderView(session: session, state: state)

            Text("Step \(stepNumber) of \(OrbitBin.allCases.count) — \(instruction)")
                .font(.system(size: 16, weight: .semibold, design: .rounded))
                .foregroundColor(.white)
                .multilineTextAlignment(.center)
                .contentTransition(.opacity)
                .animation(.easeInOut(duration: 0.2), value: instruction)

            Text(statusLine)
                .font(.system(size: 13, weight: .medium, design: .rounded))
                .foregroundColor(statusColor)

            Button(action: onRestart) {
                Text("Restart scan")
                    .font(.system(size: 12, weight: .semibold, design: .rounded))
                    .foregroundColor(.white.opacity(0.75))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 6)
                    .background(
                        Capsule()
                            .stroke(Color.white.opacity(0.3), lineWidth: 1)
                    )
            }
            .buttonStyle(.plain)
            .padding(.top, 2)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.black.opacity(0.72))
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(Color.white.opacity(0.2), lineWidth: 0.8)
                )
        )
        .shadow(color: .black.opacity(0.4), radius: 12, y: 4)
    }
}

/**
 * Compact "which way do I move" chip pinned below the mode capsule during
 * orbit mode. The big center arrow is hidden behind the user's own hand,
 * so this is the glanceable direction cue.
 */
struct OrbitDirectionChip: View {
    let session: OrbitCaptureSession
    let state: OrbitGuidanceState

    @State private var nudge = false

    private var activeBin: OrbitBin? {
        switch state {
        case .coach(let bin), .holdSteady(let bin, _), .capturing(let bin):
            return bin
        default:
            return OrbitBin.allCases.first(where: { !session.filledBins.contains($0) })
        }
    }

    private var isComplete: Bool {
        if case .complete = state { return true }
        return session.filledBins.count >= OrbitBin.allCases.count
    }

    /// Nudge offset pointing where the hand should go.
    private var nudgeOffset: CGSize {
        switch activeBin {
        case .tiltLeft: return CGSize(width: -9, height: 0)
        case .tiltRight: return CGSize(width: 9, height: 0)
        case .tiltForward: return CGSize(width: 0, height: -9)
        case .thumbToward: return CGSize(width: 7, height: 5)
        case .thumbAway: return CGSize(width: -7, height: 5)
        default: return .zero
        }
    }

    /// Setup hint shown instead of a direction while the hand itself isn't
    /// scannable yet (too far, cropped, dark…).
    private var setupHint: String? {
        switch state {
        case .idle, .searching: return "Show palm"
        case .lockPose: return "Center palm"
        case .fitHand: return "Whole hand"
        case .moveCloser: return "Come closer"
        case .addLight: return "More light"
        default: return nil
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            if isComplete {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 20, weight: .bold))
                    .foregroundColor(OverlayPalette.emerald)
                Text("Complete")
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            } else if let hint = setupHint {
                Image(systemName: "hand.raised.fill")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundColor(.yellow)
                Text(hint)
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            } else {
                Text(activeBin?.arrowSymbol ?? "✋")
                    .font(.system(size: 24, weight: .bold))
                    .foregroundColor(.cyan)
                    .offset(nudge ? nudgeOffset : .zero)
                    .animation(.easeInOut(duration: 0.7).repeatForever(autoreverses: true), value: nudge)
                    .onAppear { nudge = true }
                Text(activeBin?.shortLabel ?? "Get ready")
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.68))
                .overlay(Capsule().stroke(Color.cyan.opacity(0.5), lineWidth: 1))
        )
        .shadow(color: .black.opacity(0.4), radius: 8, y: 3)
        .allowsHitTesting(false)
    }
}

/**
 * Top Camera Mode Selector capsule.
 * Replaces the old "AUTO - L R" reticle header.
 * Allows instant switching between:
 * - Still Photo (classic single shot)
 * - 3D Orbit Scan (mount topography / metric relief)
 * - Close-up Lines (macro focus for deep creases)
 * - Fingertips (macro focus for dermatoglyphic patterns)
 */
struct CameraModeSelectorView: View {
    let selectedMode: PalmScanMode
    let onSelect: (PalmScanMode) -> Void

    var body: some View {
        HStack(spacing: 4) {
            ForEach(PalmScanMode.allCases) { mode in
                let isSelected = (mode == selectedMode)
                Button(action: {
                    withAnimation(.spring(response: 0.28, dampingFraction: 0.82)) {
                        onSelect(mode)
                    }
                }) {
                    HStack(spacing: 4) {
                        Image(systemName: mode.iconName)
                            .font(.system(size: 11, weight: isSelected ? .bold : .medium))
                            .foregroundColor(isSelected ? .white : .white.opacity(0.65))

                        Text(mode.shortTitle)
                            .font(.system(size: 11, weight: isSelected ? .bold : .medium, design: .rounded))
                            .foregroundColor(isSelected ? .white : .white.opacity(0.70))
                    }
                    .padding(.horizontal, 9)
                    .padding(.vertical, 6)
                    .background(
                        Capsule()
                            .fill(
                                isSelected
                                    ? (mode == .orbitScan ? Color.cyan.opacity(0.35) : (mode == .stillPhoto ? OverlayPalette.emerald.opacity(0.35) : Color.white.opacity(0.25)))
                                    : Color.clear
                            )
                            .overlay(
                                Capsule()
                                    .stroke(
                                        isSelected
                                            ? (mode == .orbitScan ? Color.cyan : (mode == .stillPhoto ? OverlayPalette.emerald : Color.white.opacity(0.7)))
                                            : Color.clear,
                                        lineWidth: 1
                                    )
                            )
                    )
                }
                .buttonStyle(.plain)
                .accessibilityLabel(mode.displayName)
                .accessibilityValue(isSelected ? "Selected" : "")
            }
        }
        .padding(3)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.65))
                .overlay(Capsule().stroke(Color.white.opacity(0.24), lineWidth: 0.8))
        )
        .shadow(color: Color.black.opacity(0.35), radius: 8, y: 3)
    }
}

