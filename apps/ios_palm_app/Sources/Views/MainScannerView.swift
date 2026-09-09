import SwiftUI

/**
 * Main Scanner screen orchestrating the camera preview, HUD guidance overlay,
 * spline drawing layer, and Samudrika reading presentation.
 */
public struct MainScannerView: View {
    @StateObject private var viewModel = PalmAnalysisViewModel()

    public init() {}

    public var body: some View {
        ZStack {
            // 1. Fullscreen Camera Preview
            CameraPreviewView(session: viewModel.cameraManager.captureSession)
                .ignoresSafeArea()

            // 2. Continuous Crease Spline Vector Layer
            if !viewModel.detectedSplines.isEmpty {
                CreaseSplineOverlayView(splines: viewModel.detectedSplines)
                    .ignoresSafeArea()
            }

            // 3. Liquid Glass Guidance HUD
            LiquidGlassHUDView(
                guidanceState: viewModel.guidanceState,
                metrics: viewModel.qualityMetrics,
                onManualCapture: {
                    viewModel.manualCapture()
                }
            )

            // 4. Breadcrumb Header (Liquid Glass style)
            VStack {
                HStack(spacing: 6) {
                    Image(systemName: "sparkles")
                        .foregroundColor(Color(red: 0.20, green: 0.84, blue: 0.29))
                    Text("AstroFusion")
                        .font(.system(size: 13, weight: .regular))
                        .foregroundColor(.white.opacity(0.7))
                    Text("/")
                        .foregroundColor(.white.opacity(0.4))
                    Text("Palm Perception")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.white)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(
                    Capsule()
                        .fill(.ultraThinMaterial)
                        .overlay(Capsule().stroke(Color.white.opacity(0.2), lineWidth: 1))
                )
                .padding(.top, 10)

                Spacer()
            }

            // 5. Analyzing Overlay (Active processing indicator)
            if viewModel.isAnalyzing {
                ZStack {
                    Color.black.opacity(0.6).ignoresSafeArea()
                    VStack(spacing: 16) {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle(tint: .white))
                            .scaleEffect(1.5)

                        Text(LocalizedStrings.tr("status.processing", lang: viewModel.currentLanguage))
                            .font(.system(size: 15, weight: .semibold, design: .rounded))
                            .foregroundColor(.white)
                    }
                    .padding(28)
                    .background(
                        RoundedRectangle(cornerRadius: 24, style: .continuous)
                            .fill(.ultraThinMaterial)
                            .overlay(
                                RoundedRectangle(cornerRadius: 24, style: .continuous)
                                    .stroke(Color.white.opacity(0.25), lineWidth: 1)
                            )
                    )
                }
            }
        }
        .sheet(isPresented: $viewModel.showResultSheet) {
            SamudrikaReadingSheet(
                result: viewModel.analysisResult,
                splines: viewModel.detectedSplines,
                selectedLanguage: viewModel.currentLanguage,
                onLanguageChange: { newLang in
                    viewModel.currentLanguage = newLang
                },
                onRetake: {
                    viewModel.retake()
                }
            )
            .presentationDetents([.fraction(0.45), .fraction(0.85)])
            .presentationDragIndicator(.visible)
        }
        .onAppear {
            viewModel.start()
        }
        .onDisappear {
            viewModel.stop()
        }
    }
}
