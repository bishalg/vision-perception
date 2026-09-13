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
    /// Corridor guides ship default-off (spec §4); persists across launches.
    @AppStorage("showPalmCorridors") private var showCorridors = false

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
                    onManualCapture: { viewModel.manualCapture() }
                ) {
                    // Inside the reticle box:
                    ZStack(alignment: .top) {
                        // Dynamic Palm Silhouette (anatomical thumb + fades out when hand is detected)
                        PalmSilhouetteView(
                            isReady: viewModel.guidanceState.isReady,
                            handSide: viewModel.selectedHandSide,
                            detectedIsLeft: viewModel.detectedHandSide == .left,
                            isHandDetected: !viewModel.detectedLandmarks.isEmpty
                        )

                        // Center-top inside the camera box reticle frame
                        HandSideToggleView(
                            selection: $viewModel.selectedHandSide,
                            autoDetected: viewModel.detectedHandSide,
                            isHandDetected: !viewModel.detectedLandmarks.isEmpty
                        )
                        .padding(.top, 14)
                    }
                }

                // Optional corridors toggle (bottom left, above telemetry chips)
                if !viewModel.showResultViewer && !regionLandmarks.isEmpty {
                    VStack {
                        Spacer()
                        HStack {
                            Button(action: { showCorridors.toggle() }) {
                                HStack(spacing: 4) {
                                    Image(systemName: showCorridors ? "dot.scope" : "circle.dotted")
                                    Text(LocalizedStrings.tr("camera.corridors", lang: viewModel.currentLanguage))
                                        .font(.footnote)
                                        .bold()
                                }
                                .foregroundColor(.white.opacity(showCorridors ? 0.95 : 0.6))
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(
                                    Capsule()
                                        .fill(.ultraThinMaterial)
                                        .overlay(Capsule().stroke(Color.white.opacity(showCorridors ? 0.5 : 0.2), lineWidth: 0.8))
                                )
                            }
                            .accessibilityLabel(LocalizedStrings.tr("camera.corridors", lang: viewModel.currentLanguage))
                            .accessibilityValue(showCorridors ? Text("On") : Text("Off"))
                            .accessibilityAddTraits(showCorridors ? [.isButton, .isSelected] : .isButton)
                            Spacer()
                        }
                        .padding(.horizontal, 22)
                        .padding(.bottom, 134)
                    }
                    .zIndex(1)
                }

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

                // Live sci-fi region overlay (mounts + optional corridors).
                if !regionLandmarks.isEmpty && !viewModel.showResultViewer {
                    RegionOverlayView(
                        landmarks: regionLandmarks,
                        isReady: viewModel.guidanceState.isReady,
                        showCorridors: showCorridors,
                        mirror: shouldMirror,
                        imageRect: imageRect
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
                        onConfirmAdjustedLines: { viewModel.confirmAdjustedLinesForPrediction() },
                        onDismiss: { viewModel.dismissResultViewer() },
                        onRetake: { viewModel.retake() },
                        onProceedToAnalysis: { viewModel.proceedToAnalysis() }
                    )
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .zIndex(3)
                }
            }
            // The viewfinder is a media surface: always dark, like the native Camera app.
            .preferredColorScheme(.dark)
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
