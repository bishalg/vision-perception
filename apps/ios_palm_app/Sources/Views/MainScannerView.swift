import SwiftUI
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Live guidance camera with iOS Camera-style last-capture thumbnail (bottom-left).
 * Tap thumbnail to open the crease overlay review (edit lines).
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
                PalmSilhouetteView(
                    isReady: viewModel.guidanceState.isReady,
                    handSide: viewModel.selectedHandSide,
                    detectedIsLeft: viewModel.detectedHandSide == .left
                )
            }

            // Top hand toggle (Auto / Left / Right) — algorithm auto-detects, user can lock
            VStack {
                HandSideToggleView(
                    selection: $viewModel.selectedHandSide,
                    autoDetected: viewModel.detectedHandSide
                )
                .padding(.top, 54)
                // Corridor guide toggle — only while a hand is on screen.
                if !regionLandmarks.isEmpty && !viewModel.showResultViewer {
                    Button(action: { showCorridors.toggle() }) {
                        HStack(spacing: 5) {
                            Image(systemName: showCorridors ? "dot.scope" : "circle.dotted")
                            Text("Corridors")
                                .font(.system(size: 12, weight: .semibold))
                        }
                        .foregroundColor(.white.opacity(showCorridors ? 0.95 : 0.6))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(
                            Capsule()
                                .fill(.ultraThinMaterial)
                                .overlay(Capsule().stroke(Color.white.opacity(showCorridors ? 0.5 : 0.2), lineWidth: 1))
                        )
                    }
                    .padding(.top, 6)
                }
                Spacer()
            }
            .zIndex(1)

            // Live landmark skeleton while hand is detected (same dots as result view).
            if !viewModel.detectedLandmarks.isEmpty && !viewModel.showResultViewer {
                HandOutlineOverlay(landmarks: viewModel.detectedLandmarks)
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
                    mirror: viewModel.detectedHandSide == .left
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

            if viewModel.showResultViewer {
                CaptureResultView(
                    image: viewModel.capturedImage,
                    landmarks: viewModel.captureLandmarks,
                    splines: $viewModel.detectedSplines,
                    selectedLanguage: viewModel.currentLanguage,
                    overlaySource: viewModel.overlaySource,
                    handSide: viewModel.selectedHandSide == .auto ? viewModel.detectedHandSide : viewModel.selectedHandSide,
                    onLanguageChange: { viewModel.currentLanguage = $0 },
                    onRetake: { viewModel.retake() },
                    onConfirmAdjustedLines: { viewModel.confirmAdjustedLinesForPrediction() }
                )
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .zIndex(3)
                .overlay(alignment: .topLeading) {
                    Button(action: { viewModel.dismissResultViewer() }) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 28))
                            .symbolRenderingMode(.hierarchical)
                            .foregroundStyle(.white.opacity(0.9))
                            .padding(.top, 16)
                            .padding(.leading, 16)
                    }
                }
            }
        }
        .animation(.easeInOut(duration: 0.22), value: viewModel.showResultViewer)
        .animation(.spring(response: 0.35, dampingFraction: 0.8), value: viewModel.capturedImage != nil)
        .onAppear { viewModel.start() }
        .onDisappear { viewModel.stop() }
    }
}
