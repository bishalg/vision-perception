import SwiftUI
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Live guidance camera with iOS Camera-style last-capture thumbnail (bottom-left).
 * Tap thumbnail to open the crease overlay review.
 */
public struct MainScannerView: View {
    @StateObject private var viewModel = PalmAnalysisViewModel()

    public init() {}

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
                PalmSilhouetteView(isReady: viewModel.guidanceState.isReady)
            }

            // Live landmark skeleton while hand is detected (same dots as result view).
            if !viewModel.detectedLandmarks.isEmpty && !viewModel.showResultViewer {
                HandOutlineOverlay(landmarks: viewModel.detectedLandmarks)
                    .ignoresSafeArea()
                    .allowsHitTesting(false)
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
                    splines: viewModel.detectedSplines,
                    selectedLanguage: viewModel.currentLanguage,
                    usedNativeEngine: viewModel.usedNativeEngine,
                    onLanguageChange: { viewModel.currentLanguage = $0 },
                    onRetake: { viewModel.retake() }
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
