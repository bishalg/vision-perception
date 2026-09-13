import SwiftUI
import CurvAsdGazeKit

public enum AppWorkflowState {
    case scanner
    case result(GazeSessionReportSummary, [GazeFrame])
    case history
    case calibrating
}

@main
struct AsdGazeApp: App {
    @State private var workflowState: AppWorkflowState = .scanner

    var body: some Scene {
        WindowGroup {
            Group {
                switch workflowState {
                case .scanner:
                    MainGazeScannerView(
                        onComplete: { summary, frames in
                            workflowState = .result(summary, frames)
                        },
                        onOpenHistory: {
                            workflowState = .history
                        }
                    )
                case .result(let summary, let frames):
                    GazeSessionResultView(
                        summary: summary,
                        recordedFrames: frames,
                        onDismiss: {
                            workflowState = .scanner
                        },
                        onRetake: {
                            workflowState = .scanner
                        }
                    )
                case .history:
                    CaregiverDashboardView {
                        workflowState = .calibrating
                    }
                case .calibrating:
                    BubbleCalibrationView {
                        workflowState = .scanner
                    }
                }
            }
            .preferredColorScheme(.dark)
        }
    }
}
