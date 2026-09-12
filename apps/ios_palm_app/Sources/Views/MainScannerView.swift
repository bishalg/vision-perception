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

    public init() {}

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

                // Bottom-left thumbnail (like stock iOS Camera)
                if let thumb = viewModel.capturedImage, !viewModel.showResultViewer {
                    VStack {
                        Spacer()
                        HStack {
                            Button(action: { viewModel.openLastCapture() }) {
                                Image(uiImage: thumb)
                                    .resizable()
                                    .scaledToFill()
                                    .frame(width: 54, height: 54)
                                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                                            .stroke(Color.white.opacity(0.85), lineWidth: 1.5)
                                    )
                                    .shadow(color: .black.opacity(0.45), radius: 8, y: 3)
                            }
                            .padding(.leading, 22)
                            .padding(.bottom, 46)
                            Spacer()
                        }
                    }
                    .transition(.scale.combined(with: .opacity))
                    .zIndex(2)
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
        .animation(.spring(response: 0.35, dampingFraction: 0.8), value: viewModel.capturedImage != nil)
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
