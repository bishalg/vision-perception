import SwiftUI
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Thin coordinator view — assembles the camera feed with the correct per-mode
 * overlay. Each scan mode (Still · Orbit · Close-Up · Fingertips) has its own
 * dedicated overlay file in `Sources/Views/Scanner/`.
 *
 * Responsibilities kept here:
 *  - `CameraOverlayHost` setup with mode-aware reticle content
 *  - Mode selector capsule + mode-specific top chips
 *  - Live landmark overlays (hand outline, region heat map)
 *  - Filmstrip thumbnail + capture hero animation
 *  - Phase 2 result viewer + Phase 3 reading sheet
 *  - Camera session lifecycle (stop during review, resume on dismiss)
 */
public struct MainScannerView: View {
    @StateObject private var viewModel = PalmAnalysisViewModel()

    /// Drives the post-capture "hero" flight: large flash shrinks into filmstrip.
    @Namespace private var captureHeroNamespace
    @State private var showCaptureFlash = false
    @State private var flashImage: UIImage?
    @State private var filmstripShakeTicks: CGFloat = 0

    public init() {}

    // MARK: - Helpers

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
            withAnimation(.default) { filmstripShakeTicks += 1 }
        }
    }

    private var regionLandmarks: [CGPoint] {
        if !viewModel.detectedLandmarks.isEmpty { return viewModel.detectedLandmarks }
        #if targetEnvironment(simulator)
        return PalmRegionGeometry.scaffoldLandmarks()
        #else
        return []
        #endif
    }

    // MARK: - Reticle Content (per-mode)

    @ViewBuilder
    private func reticleContent() -> some View {
        switch viewModel.scanMode {
        case .stillPhoto:
            PalmSilhouetteView(
                isReady: viewModel.guidanceState.isReady,
                handSide: viewModel.selectedHandSide,
                detectedIsLeft: viewModel.detectedHandSide == .left,
                isHandDetected: !viewModel.detectedLandmarks.isEmpty
            )
        case .orbitScan:
            // Orbit has its own full HUD — no silhouette bleeds into orbit mode
            Color.clear
        case .closeUpMosaic:
            CloseUpReticleView(dwellProgress: viewModel.closeUpDwellProgress)
        case .fingertipScan:
            FingertipMacroReticleView(
                isCapturing: viewModel.isCapturing,
                holdProgress: viewModel.fingertipHoldProgress
            )
        }
    }

    // MARK: - Body

    public var body: some View {
        GeometryReader { geometry in
            let cameraFeedSize = CGSize(width: 1080, height: 1920)
            let imageRect = ImageCoordinateMapper.imageRect(
                imageSize: cameraFeedSize,
                viewSize: geometry.size,
                mode: .fill
            )
            let isScaffold = viewModel.detectedLandmarks.isEmpty
            let shouldMirror = isScaffold &&
                (viewModel.selectedHandSide == .left || viewModel.detectedHandSide == .left)

            ZStack {
                // MARK: Camera + reticle host
                CameraOverlayHost(
                    profile: viewModel.profile,
                    session: viewModel.cameraSession,
                    guidanceState: viewModel.guidanceState,
                    metrics: viewModel.qualityMetrics,
                    isAnalyzing: viewModel.isAnalyzing,
                    statusMessageOverride: viewModel.statusMessageOverride,
                    onManualCapture: { viewModel.manualCapture() },
                    showsShutter: viewModel.scanMode == .stillPhoto,
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
                    reticleContent()
                }

                // MARK: 3D Orbit guidance HUD
                if viewModel.scanMode == .orbitScan && !viewModel.showResultViewer {
                    OrbitGuidanceOverlayView(
                        session: viewModel.orbitSession,
                        state: viewModel.orbitState
                    )
                    .allowsHitTesting(false)
                    .zIndex(2)
                }

                // MARK: Fingertip guidance HUD
                if viewModel.scanMode == .fingertipScan && !viewModel.showResultViewer {
                    FingertipGuidanceOverlayView(
                        session: viewModel.fingertipSession,
                        state: viewModel.fingertipState,
                        holdProgress: viewModel.fingertipHoldProgress,
                        isCapturing: viewModel.isCapturing
                    )
                    .zIndex(2)
                }

                // MARK: Close-Up guidance HUD
                if viewModel.scanMode == .closeUpMosaic && !viewModel.showResultViewer {
                    CloseUpGuidanceOverlayView(
                        currentTile: CloseUpTile(rawValue: viewModel.closeUpTileStatus),
                        statusMessage: viewModel.statusMessageOverride ?? "Move in close",
                        dwellProgress: viewModel.closeUpDwellProgress
                    )
                    .zIndex(2)
                }

                // MARK: Mode selector capsule + orbit direction chip
                VStack(spacing: 8) {
                    CameraModeSelectorView(
                        selectedMode: viewModel.scanMode,
                        completedModes: viewModel.completedScanModes,
                        onSelect: { viewModel.setScanMode($0) }
                    )
                    if viewModel.scanMode == .orbitScan && !viewModel.showResultViewer {
                        OrbitDirectionChip(
                            session: viewModel.orbitSession,
                            state: viewModel.orbitState
                        )
                    }
                    Spacer()
                }
                .padding(.top, max(geometry.safeAreaInsets.top, 8))
                .zIndex(2)

                // MARK: Live landmark overlays
                if !viewModel.detectedLandmarks.isEmpty && !viewModel.showResultViewer {
                    HandOutlineOverlay(
                        landmarks: viewModel.detectedLandmarks,
                        imageRect: imageRect
                    )
                    .ignoresSafeArea()
                    .allowsHitTesting(false)
                    .zIndex(1)
                }

                if !regionLandmarks.isEmpty && !viewModel.showResultViewer {
                    RegionOverlayView(
                        landmarks: regionLandmarks,
                        isReady: viewModel.scanMode == .stillPhoto
                            ? viewModel.guidanceState.isReady
                            : viewModel.orbitSession.coverageFraction > 0.3,
                        showCorridors: false,
                        mirror: shouldMirror,
                        imageRect: imageRect,
                        mountReliefHeat: viewModel.mountReliefHeat
                    )
                    .ignoresSafeArea()
                    .allowsHitTesting(false)
                    .zIndex(1)
                }

                // MARK: Filmstrip thumbnail (bottom-left, iOS Camera-style)
                if let thumb = viewModel.capturedShots.last,
                   !viewModel.showResultViewer,
                   !showCaptureFlash
                {
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
                            .padding(.bottom, max(geometry.safeAreaInsets.bottom, 16) + 20)
                            Spacer()
                        }
                    }
                    .transition(.scale.combined(with: .opacity))
                    .zIndex(2)
                }

                // MARK: Per-mode bottom cards
                switch viewModel.scanMode {
                case .stillPhoto:
                    EmptyView()  // shutter is inside CameraOverlayHost
                case .orbitScan:
                    if !viewModel.showResultViewer {
                        VStack {
                            Spacer()
                            OrbitScanBottomCard(
                                session: viewModel.orbitSession,
                                state: viewModel.orbitState,
                                onRestart: { viewModel.setScanMode(.orbitScan) },
                                onView3D: { viewModel.showResultViewer = true }
                            )
                            .padding(.horizontal, 16)
                            .padding(.bottom, max(geometry.safeAreaInsets.bottom, 16) + 12)
                        }
                        .zIndex(2)
                    }
                case .closeUpMosaic:
                    if !viewModel.showResultViewer {
                        VStack {
                            Spacer()
                            CloseUpScanBottomCard(
                                session: viewModel.closeUpSession,
                                statusMessage: viewModel.statusMessageOverride ?? "Move in close",
                                dwellProgress: viewModel.closeUpDwellProgress,
                                onRestart: { viewModel.setScanMode(.closeUpMosaic) },
                                onViewReading: {
                                    viewModel.mergeScanEnrichmentIntoAnalysisResult()
                                    viewModel.showAnalysisReading = true
                                }
                            )
                            .padding(.horizontal, 16)
                            .padding(.bottom, max(geometry.safeAreaInsets.bottom, 16) + 12)
                        }
                        .zIndex(2)
                    }
                case .fingertipScan:
                    if !viewModel.showResultViewer {
                        VStack {
                            Spacer()
                            FingertipScanBottomCard(
                                session: viewModel.fingertipSession,
                                isCapturing: viewModel.isCapturing,
                                statusMessage: viewModel.statusMessageOverride ?? "Align fingertip in circle",
                                onCapture: { viewModel.manualFingertipCapture() },
                                onReset: { viewModel.resetFingertipScan() },
                                onInspect: {
                                    viewModel.resultViewerInitialTab = .fingertips
                                    viewModel.showResultViewer = true
                                },
                                onViewReading: {
                                    viewModel.mergeScanEnrichmentIntoAnalysisResult()
                                    viewModel.showAnalysisReading = true
                                }
                            )
                            .padding(.horizontal, 16)
                            .padding(.bottom, max(geometry.safeAreaInsets.bottom, 16) + 12)
                        }
                        .zIndex(2)
                    }
                }

                // MARK: Capture hero flash (shrinks into filmstrip slot)
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

                // MARK: Phase 2 — Crease line review
                if viewModel.showResultViewer {
                    CaptureResultView(
                        image: viewModel.capturedImage,
                        landmarks: viewModel.captureLandmarks,
                        splines: $viewModel.detectedSplines,
                        selectedLanguage: viewModel.currentLanguage,
                        overlaySource: viewModel.overlaySource,
                        handSide: viewModel.selectedHandSide == .auto
                            ? viewModel.detectedHandSide
                            : viewModel.selectedHandSide,
                        padDepths: viewModel.padDepths,
                        fingertipSession: viewModel.fingertipSession,
                        initialTab: viewModel.resultViewerInitialTab,
                        onPatternOverride: { digit, pattern in
                            viewModel.overrideFingertipPattern(digit: digit, pattern: pattern)
                        },
                        onConfirmAdjustedLines: { viewModel.confirmAdjustedLinesForPrediction() },
                        onDismiss: { viewModel.dismissResultViewer() },
                        onRetake: { viewModel.retake() },
                        onProceedToAnalysis: { viewModel.proceedToAnalysis() }
                    )
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .zIndex(3)
                }
            }
        }
        .ignoresSafeArea()
        .animation(.easeInOut(duration: 0.22), value: viewModel.showResultViewer)
        .onChange(of: viewModel.captureTick) { _ in animateNewCapture() }
        // Phase 3 — Reading sheet
        .sheet(isPresented: $viewModel.showAnalysisReading) {
            SamudrikaReadingSheet(
                result: viewModel.analysisResult,
                splines: viewModel.predictionReadySplines.isEmpty
                    ? viewModel.detectedSplines
                    : viewModel.predictionReadySplines,
                handSide: viewModel.selectedHandSide == .auto
                    ? viewModel.detectedHandSide
                    : viewModel.selectedHandSide,
                selectedLanguage: viewModel.currentLanguage,
                onLanguageChange: { viewModel.currentLanguage = $0 },
                onRetake: { viewModel.retake() }
            )
            .presentationDetents([.medium, .large])
            .presentationDragIndicator(.visible)
        }
        // Camera session lifecycle
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

// MARK: - Filmstrip Thumbnail

/// Bottom-left capture roll thumbnail — like iOS Camera's, with a session badge.
/// Badge + border both go green once `count >= requiredCount`.
private struct CaptureFilmstripThumbnail: View {
    let image: UIImage
    let count: Int
    let requiredCount: Int
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
            .shadow(
                color: (isComplete ? OverlayPalette.emerald : .black)
                    .opacity(isComplete ? 0.55 : 0.45),
                radius: 8, y: 3
            )
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

// MARK: - Shake Effect

/// Classic damped horizontal shake — caller bumps `animatableData` inside
/// `withAnimation` to trigger one shake. No Timer, no manual step tracking.
private struct ShakeEffect: GeometryEffect {
    var travel: CGFloat = 6
    var oscillations: Double = 3
    var animatableData: CGFloat

    func effectValue(size: CGSize) -> ProjectionTransform {
        let dx = travel * sin(Double(animatableData) * .pi * oscillations)
        return ProjectionTransform(CGAffineTransform(translationX: dx, y: 0))
    }
}
