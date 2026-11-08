import SwiftUI
import AsdCore
import CurvAsdGazeKit

// MARK: - Wizard Step Enum

public enum ScreeningWizardStep: Equatable, Sendable {
    /// "Before you begin" (S-13): disclaimer, checklist and what is recorded. Shown before
    /// positioning; not part of `captureOrder` because no capture session runs yet.
    case beforeYouBegin
    case positioning
    case gate
    /// Calibration and its validation (CAL-7 .. CAL-9, VAL-1) in the run's capture session.
    case calibrating
    case stimulus
    #if ASD_RESEARCH_CLASSIFIER
    case summary(GazeSessionReportSummary, [GazeFrame], EngagementClassificationResult?)
    #else
    case summary(GazeSessionReportSummary, [GazeFrame])
    #endif

    public static func == (lhs: ScreeningWizardStep, rhs: ScreeningWizardStep) -> Bool {
        switch (lhs, rhs) {
        case (.beforeYouBegin, .beforeYouBegin),
             (.positioning, .positioning),
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

    /// Capture order of one run (WP-G.10): position the camera, make sure the stimuli are on
    /// the device, then calibration → validation → battery in one capture session, so the
    /// session records its own calibration and the calibration is never older than needed.
    public static let captureOrder: [ScreeningWizardStep] = [.positioning, .gate, .calibrating, .stimulus]

    /// The step after this one in `captureOrder`; `nil` after the battery (the summary follows).
    public var nextCaptureStep: ScreeningWizardStep? {
        guard let i = Self.captureOrder.firstIndex(of: self), i + 1 < Self.captureOrder.count else { return nil }
        return Self.captureOrder[i + 1]
    }
}

// MARK: - Wizard Orchestrator

/// The top-level Gaze Screening Wizard (`ScreeningWizardStep.captureOrder`):
///   Step 0 → `BeforeYouBeginView` (disclaimer, checklist; camera primer before the system prompt)
///   Step 1 → `GazePositioningStepView` (Face Alignment & Distance Sweet Spot)
///   Step 2 → `StimulusDownloadGateView` (stimuli on the device)
///   Step 3 → `BubbleCalibrationView` (calibration + validation in the capture session)
///   Step 4 → `GazeStimulusScreeningView` (clips with drift checks, same capture session)
///   Result → `GazeSessionResultView` (Engagement Report & CareKit Sync)
public struct GazeScreeningWizardView: View {
    @State private var step: ScreeningWizardStep = .beforeYouBegin
    @StateObject private var camera = CameraAccessCoordinator()
    /// Observed so the entry gate re-evaluates when the guardian agrees (S-10).
    @ObservedObject private var consentRecords = ConsentRecordStore.shared
    /// S-11: camera primer before the system prompt, only while iOS has not asked yet.
    @State private var permissionPrimer: PermissionKind?
    /// CAL-1: one capture session per wizard run. Calibration, validation, the clips and the
    /// drift checks all run in this controller's session pipeline, which is new for every run
    /// (every start creates a new pipeline), so a calibration is never applied to another
    /// session or child and there is no "skip, use saved calibration" path.
    @StateObject private var captureController = GazeSessionController()

    public var onOpenHistory: (() -> Void)?

    public init(onOpenHistory: (() -> Void)? = nil) {
        self.onOpenHistory = onOpenHistory
    }

    public var body: some View {
        ZStack {
            // SIM-7 / CNS-1 entry gate: unsupported hardware (or missing consent) shows an
            // explanation and offers no way to start a session.
            if case .blocked(let reason) = CaptureEntryGate.evaluate(consentRecords: consentRecords) {
                if reason == .consentOutdated {
                    // S-10: a changed or withdrawn consent is reviewed here, not a dead end.
                    ConsentOutdatedView(store: consentRecords)
                } else {
                    UnsupportedDeviceView(reason: reason)
                }
            } else if camera.state.showsDeniedView {
                // CAM-1: denied or restricted camera access shows the explanation and Settings link.
                CameraDeniedView(state: camera.state)
            } else if step == .beforeYouBegin {
                // S-13: shown before positioning, before any camera prompt.
                BeforeYouBeginView(onBegin: beginTapped, onCancel: nil)
                    .transition(.opacity)
            } else if camera.canStartSession {
                wizardSteps
            } else {
                // Undetermined: the system prompt is (or is about to be) on screen.
                ProgressView()
            }
        }
        .task {
            // SIM-7: no camera prompt on hardware the gate above already blocks.
            guard CaptureEntryGate.deviceSupportsFaceTracking else { return }
            // S-11: an undetermined camera is asked only after "Begin" and the in-app primer.
            guard !camera.needsSystemPrompt else { return }
            await camera.ensureAccess(consentStore: .shared)
        }
        .cameraScenePhase(camera) { _ in }
        .permissionPrimer(
            $permissionPrimer,
            request: { _ in _ = await camera.ensureAccess(consentStore: .shared) },
            onFinish: { _ in
                // "Not now" leaves the camera undetermined: stay on "Before you begin".
                if camera.canStartSession { advanceToPositioning() }
            }
        )
    }

    /// "Begin": camera primer first when iOS has not asked yet (S-11), then positioning.
    private func beginTapped() {
        guard CaptureEntryGate.deviceSupportsFaceTracking else { return }
        if camera.needsSystemPrompt {
            permissionPrimer = .camera
            return
        }
        Task { @MainActor in
            await camera.ensureAccess(consentStore: .shared)
            if camera.canStartSession { advanceToPositioning() }
        }
    }

    private func advanceToPositioning() {
        withAnimation(.easeInOut(duration: 0.3)) { step = .positioning }
    }

    @ViewBuilder
    private var wizardSteps: some View {
        ZStack {
            switch step {
            case .beforeYouBegin:
                // Rendered by `body` before the camera is ready; never reached here.
                EmptyView()

            case .positioning:
                GazePositioningStepView(
                    onReady: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .gate }
                    }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity),
                    removal: .move(edge: .leading).combined(with: .opacity)
                ))

            case .calibrating:
                BubbleCalibrationView(
                    controller: captureController,
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

            case .gate:
                let profile = ChildProfileStore.shared.profile
                StimulusDownloadGateView(
                    childAgeMonths: profile.ageBracket.midpointMonths,
                    localeCode: profile.preferredLanguage,
                    onReady: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .calibrating }
                    },
                    onFallback: {
                        withAnimation(.easeInOut(duration: 0.3)) { step = .calibrating }
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
                        controller: captureController,
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
                        controller: captureController,
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
                        // The next observation starts with "Before you begin" again (S-13).
                        withAnimation(.easeInOut(duration: 0.3)) { step = .beforeYouBegin }
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
                        // The next observation starts with "Before you begin" again (S-13).
                        withAnimation(.easeInOut(duration: 0.3)) { step = .beforeYouBegin }
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
        // Orientation stays fixed from positioning until the results screen.
        .captureOrientationLocked()
        .onChange(of: step) { _, newStep in
            // A new run (retake, abort, dismiss) never inherits the previous calibration: the
            // run's session ends here and the next calibration starts a new pipeline.
            if newStep == .positioning || newStep == .beforeYouBegin, captureController.isRecording {
                Task { _ = await captureController.stopSession() }
            }
        }
        .toolbar(step == .stimulus ? .hidden : .visible, for: .tabBar)
    }
}
