import SwiftUI
import CurvAsdGazeKit

public enum AppWorkflowState {
    case dashboard
    case calibrating
    case screening
}

@main
struct AsdGazeApp: App {
    @State private var workflowState: AppWorkflowState = .dashboard

    var body: some Scene {
        WindowGroup {
            Group {
                switch workflowState {
                case .dashboard:
                    CaregiverDashboardView {
                        workflowState = .calibrating
                    }
                case .calibrating:
                    BubbleCalibrationView {
                        workflowState = .screening
                    }
                case .screening:
                    StimulusScreeningView {
                        workflowState = .dashboard
                    }
                }
            }
            .preferredColorScheme(.dark)
        }
    }
}
