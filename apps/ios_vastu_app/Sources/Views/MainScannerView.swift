import SwiftUI
import CurvCameraOverlay
import CurvVastuOverlay

public struct MainScannerView: View {
    @StateObject private var viewModel = VastuAnalysisViewModel()

    public init() {}

    public var body: some View {
        CameraOverlayHost(
            profile: viewModel.profile,
            session: viewModel.cameraSession,
            guidanceState: viewModel.guidanceState,
            metrics: viewModel.qualityMetrics,
            isAnalyzing: viewModel.isAnalyzing,
            onManualCapture: { viewModel.manualCapture() }
        ) {
            VastuMandalaView(isReady: viewModel.guidanceState.isReady)
        }
        .sheet(isPresented: $viewModel.showResultSheet) {
            VastuResultSheet(
                json: viewModel.analysisJSON,
                onRetake: { viewModel.retake() }
            )
            .presentationDetents([.fraction(0.45), .fraction(0.85)])
            .presentationDragIndicator(.visible)
        }
        .onAppear { viewModel.start() }
        .onDisappear { viewModel.stop() }
    }
}

public struct VastuResultSheet: View {
    public let json: String?
    public let onRetake: () -> Void

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Capsule()
                    .fill(Color.white.opacity(0.3))
                    .frame(width: 44, height: 5)
                    .frame(maxWidth: .infinity)
                    .padding(.top, 10)

                Text("Vastu Mandala Analysis")
                    .font(.system(size: 20, weight: .bold, design: .rounded))
                    .foregroundColor(.white)

                Text("Floor-plan ridges mapped to the 9-pad Purusha Mandala via CurvEngine domain `floor_plan`.")
                    .font(.system(size: 13))
                    .foregroundColor(.white.opacity(0.7))

                Text(json ?? "{\"status\":\"no_evidence\"}")
                    .font(.system(size: 11, weight: .regular, design: .monospaced))
                    .foregroundColor(.white.opacity(0.85))
                    .padding(12)
                    .background(
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .fill(Color.white.opacity(0.06))
                    )

                Button(action: onRetake) {
                    HStack {
                        Image(systemName: "arrow.counterclockwise")
                        Text("Retake")
                    }
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .fill(.ultraThinMaterial)
                    )
                }
                .padding(.bottom, 24)
            }
            .padding(.horizontal)
        }
        .background(Color.black.opacity(0.85).background(.ultraThinMaterial).ignoresSafeArea())
    }
}
