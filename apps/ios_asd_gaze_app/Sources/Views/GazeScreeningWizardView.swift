import SwiftUI
import CurvAsdGazeKit

// MARK: - Wizard Step Enum

public enum ScreeningWizardStep: Equatable {
    case positioning
    case calibrating
    case stimulus
    case summary(GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult)

    public static func == (lhs: ScreeningWizardStep, rhs: ScreeningWizardStep) -> Bool {
        switch (lhs, rhs) {
        case (.positioning, .positioning),
             (.calibrating, .calibrating),
             (.stimulus, .stimulus):
            return true
        case (.summary, .summary):
            return true
        default:
            return false
        }
    }
}

// MARK: - Wizard Orchestrator

/// The top-level Gaze Screening Wizard orchestrating 3 sequential clinical steps:
///   Step 1 → `GazePositioningStepView` (Face Alignment & Distance Sweet Spot)
///   Step 2 → `BubbleCalibrationView` (5-Point Interactive Bubble Calibration)
///   Step 3 → `GazeStimulusScreeningView` (Full-Screen Stimulus + 1:1 Affect Logging)
///   Result → `GazeSessionResultView` (Biomarker Report & CareKit Sync)
public struct GazeScreeningWizardView: View {
    @State private var step: ScreeningWizardStep = .positioning

    public var onOpenHistory: (() -> Void)?

    public init(onOpenHistory: (() -> Void)? = nil) {
        self.onOpenHistory = onOpenHistory
    }

    public var body: some View {
        ZStack {
            switch step {
            case .positioning:
                GazePositioningStepView(
                    onReady: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .calibrating }
                    }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            case .calibrating:
                BubbleCalibrationView(
                    onClose: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
                    },
                    onCalibrationComplete: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .stimulus }
                    }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            case .stimulus:
                GazeStimulusScreeningView(
                    onComplete: { summary, frames, classification in
                        withAnimation(.easeInOut(duration: 0.25)) {
                            step = .summary(summary, frames, classification)
                        }
                    },
                    onAbort: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
                    }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            case .summary(let summary, let frames, let classification):
                GazeSessionResultView(
                    summary: summary,
                    recordedFrames: frames,
                    classification: classification,
                    onDismiss: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
                    },
                    onRetake: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
                    }
                )
                .transition(.asymmetric(
                    insertion: .opacity,
                    removal: .opacity
                ))
            }
        }
        .animation(.easeInOut(duration: 0.3), value: step)
    }
}
