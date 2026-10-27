import SwiftUI
import CurvAsdGazeKit

// MARK: - Wizard Step Enum

public enum ScreeningWizardStep: Equatable {
    case positioning
    case calibrating
    case gate
    case stimulus
    #if ASD_RESEARCH_CLASSIFIER
    case summary(GazeSessionReportSummary, [GazeFrame], BiomarkerClassificationResult?)
    #else
    case summary(GazeSessionReportSummary, [GazeFrame])
    #endif

    public static func == (lhs: ScreeningWizardStep, rhs: ScreeningWizardStep) -> Bool {
        switch (lhs, rhs) {
        case (.positioning, .positioning),
             (.calibrating, .calibrating),
             (.gate, .gate),
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
    @StateObject private var camera = CameraAccessCoordinator()

    public var onOpenHistory: (() -> Void)?

    public init(onOpenHistory: (() -> Void)? = nil) {
        self.onOpenHistory = onOpenHistory
    }

    public var body: some View {
        ZStack {
            // SIM-7 / CNS-1 entry gate: unsupported hardware (or missing consent) shows an
            // explanation and offers no way to start a session.
            if case .blocked(let reason) = CaptureEntryGate.evaluate() {
                UnsupportedDeviceView(reason: reason)
            } else if camera.state.showsDeniedView {
                // CAM-1: denied or restricted camera access shows the explanation and Settings link.
                CameraDeniedView(state: camera.state)
            } else if camera.canStartSession {
                wizardSteps
            } else {
                // Undetermined: the system prompt is (or is about to be) on screen.
                ProgressView()
            }
        }
        .task { await camera.ensureAccess(consentStore: .shared) }
        .cameraScenePhase(camera) { _ in }
    }

    @ViewBuilder
    private var wizardSteps: some View {
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
                    onClose: nil,
                    onCalibrationComplete: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .gate }
                    }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            case .gate:
                let profile = ChildProfileStore.shared.profile
                StimulusDownloadGateView(
                    childAgeMonths: profile.ageBracket.midpointMonths,
                    localeCode: profile.preferredLanguage,
                    onReady: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .stimulus }
                    },
                    onFallback: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .stimulus }
                    }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            case .stimulus:
                let profile = ChildProfileStore.shared.profile
                let durationSec = Double(profile.sessionDurationSec)
                let activePlaylist = StimulusPlaylistBattery.playlist(
                    forTargetDuration: durationSec,
                    ageMonths: profile.ageBracket.midpointMonths
                )
                Group {
                    #if ASD_RESEARCH_CLASSIFIER
                    GazeStimulusScreeningView(
                        playlist: activePlaylist,
                        onComplete: { summary, frames, classification in
                            withAnimation(.easeInOut(duration: 0.25)) {
                                step = .summary(summary, frames, classification)
                            }
                        },
                        onAbort: {
                            withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
                        }
                    )
                    #else
                    GazeStimulusScreeningView(
                        playlist: activePlaylist,
                        onComplete: { summary, frames in
                            withAnimation(.easeInOut(duration: 0.25)) {
                                step = .summary(summary, frames)
                            }
                        },
                        onAbort: {
                            withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
                        }
                    )
                    #endif
                }
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            #if ASD_RESEARCH_CLASSIFIER
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
            #else
            case .summary(let summary, let frames):
                GazeSessionResultView(
                    summary: summary,
                    recordedFrames: frames,
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
            #endif
            }
        }
        .animation(.easeInOut(duration: 0.3), value: step)
        .toolbar(step == .stimulus ? .hidden : .visible, for: .tabBar)
    }
}
