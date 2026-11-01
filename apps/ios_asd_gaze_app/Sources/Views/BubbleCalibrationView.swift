import SwiftUI
import AsdCore
import CurvAsdGazeKit

// MARK: - Balloon Shapes

/// Teardrop balloon body with realistic curvature and bottom knot.
public struct BalloonShape: Shape {
    public func path(in rect: CGRect) -> Path {
        var path = Path()
        let w = rect.width
        let h = rect.height
        let knotHeight: CGFloat = h * 0.12
        let bodyHeight: CGFloat = h - knotHeight

        // Teardrop balloon bulb
        path.move(to: CGPoint(x: w * 0.5, y: bodyHeight))
        path.addCurve(
            to: CGPoint(x: w * 0.5, y: 0),
            control1: CGPoint(x: -w * 0.18, y: bodyHeight * 0.65),
            control2: CGPoint(x: 0, y: 0)
        )
        path.addCurve(
            to: CGPoint(x: w * 0.5, y: bodyHeight),
            control1: CGPoint(x: w, y: 0),
            control2: CGPoint(x: w * 1.18, y: bodyHeight * 0.65)
        )
        path.closeSubpath()

        // Knot triangle
        path.move(to: CGPoint(x: w * 0.44, y: bodyHeight))
        path.addLine(to: CGPoint(x: w * 0.36, y: h))
        path.addLine(to: CGPoint(x: w * 0.64, y: h))
        path.addLine(to: CGPoint(x: w * 0.56, y: bodyHeight))
        path.closeSubpath()

        return path
    }
}

/// Dynamic wavy balloon string.
public struct BalloonStringShape: Shape {
    public var sway: CGFloat

    public var animatableData: CGFloat {
        get { sway }
        set { sway = newValue }
    }

    public func path(in rect: CGRect) -> Path {
        var path = Path()
        let w = rect.width
        let h = rect.height
        path.move(to: CGPoint(x: w * 0.5, y: 0))
        path.addCurve(
            to: CGPoint(x: w * 0.5 + sway * 10, y: h),
            control1: CGPoint(x: w * 0.5 - 8 + sway * 6, y: h * 0.4),
            control2: CGPoint(x: w * 0.5 + 10 - sway * 5, y: h * 0.75)
        )
        return path
    }
}

// MARK: - Calibration screen (WP-G.10)

/// Hands-free calibration with shrinking balloon characters (CAL-7, CAL-8), followed by the
/// validation step (VAL-1), run through the session's Rust gaze pipeline.
///
/// The pipeline owns every decision: which target to show next (including the CAL-3 re-show of
/// a starved target), when a window has enough frame time, the robust fit, model selection,
/// the quality band (CAL-5) and the advice (CAL-9). Nothing is fitted or stored in Swift, and
/// samples are never gated on the distance between raw gaze and the target (CAL-2).
///
/// Two modes:
/// - With a `controller` (the wizard, and re-calibration inside the battery): the calibration
///   runs in that controller's session, so the session record carries this calibration and
///   its validation (QS-1). The session is started here (without a clip) only if it is not
///   running yet; it is never stopped here.
/// - Without one (Settings, the free scanner): a calibration check in a session of its own,
///   stopped when the view closes. The calibration belongs to that session only (CAL-1) and is
///   never applied to another one.
public struct BubbleCalibrationView: View {
    @Environment(\.dismiss) private var dismiss
    @StateObject private var controller: GazeSessionController
    private let ownsSession: Bool
    /// Why this calibration runs (recorded for a re-calibration, QS-1).
    public let reason: GazeRecalReason?
    public let onCalibrationComplete: () -> Void
    public var onClose: (() -> Void)?

    @StateObject private var camera = CameraAccessCoordinator()
    @State private var entryBlock: DeviceBlockReason?
    @State private var isCapturing = false
    /// A new run (after an interruption) starts the calibration over in a fresh view.
    @State private var runId = UUID()
    /// SIM-2/SIM-7: why capture cannot run right now, if it cannot.
    private var activeBlock: DeviceBlockReason? { entryBlock ?? controller.blockReason }

    public init(
        controller: GazeSessionController? = nil,
        reason: GazeRecalReason? = nil,
        onClose: (() -> Void)? = nil,
        onCalibrationComplete: @escaping () -> Void
    ) {
        _controller = StateObject(wrappedValue: controller ?? GazeSessionController())
        self.ownsSession = controller == nil
        self.reason = reason
        self.onClose = onClose
        self.onCalibrationComplete = onCalibrationComplete
    }

    public var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            if isCapturing {
                CalibrationRoundView(controller: controller, reason: reason) { _ in
                    finish()
                }
                .id(runId)
            } else {
                ProgressView()
                    .tint(.white)
            }

            VStack {
                captureControlsRow
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.top, 52)
        }
        // GEO: the layout covers the whole screen, so normalised target positions are screen
        // positions, and there is no navigation bar during capture.
        .ignoresSafeArea()
        .toolbar(.hidden, for: .navigationBar)
        .mediaDark()
        .overlay {
            if camera.state.showsDeniedView {
                CameraDeniedView(state: camera.state, onClose: onClose)
            } else if let reason = activeBlock {
                UnsupportedDeviceView(reason: reason, onClose: onClose)
            }
        }
        .task { await startCapture() }
        .onDisappear {
            if ownsSession { stopCapture() }
        }
        .captureOrientationLocked()
        .cameraScenePhase(camera) { action in
            switch action {
            case .stopSession:
                stopCapture()
            case .backgroundInterruptionEnded:
                // The calibration starts over in a new session; nothing partial is kept.
                Task { await startCapture() }
            case .none:
                break
            }
        }
    }

    // MARK: Capture

    private func startCapture() async {
        // SIM-7: hardware first, so a device without TrueDepth (or Apple Vision Pro)
        // never sees a camera prompt for capture it cannot run.
        guard CaptureEntryGate.deviceSupportsFaceTracking else {
            entryBlock = .faceTrackingUnsupported
            return
        }
        // CAM-1: no session until camera access is granted.
        guard await camera.ensureAccess(consentStore: .shared) == .granted, !Task.isCancelled else {
            if camera.state == .unknown { entryBlock = .consentRequired }
            return
        }
        if ownsSession || !controller.isRecording {
            // CNS-1: consent token required. No clip yet: calibration frames never count as
            // stimulus frames (G.3 `set_recording`).
            guard controller.startSessionWithConsent(beginsClip: false) else {
                entryBlock = .consentRequired
                return
            }
        }
        entryBlock = nil
        guard controller.blockReason == nil else { return }
        camera.sessionDidStart()
        runId = UUID()
        isCapturing = true
    }

    private func stopCapture() {
        isCapturing = false
        guard camera.isSessionActive else { return }
        camera.sessionDidEnd()
        // Also in the wizard's session: a background or a close ends this run, and a new
        // start makes a new pipeline (CAL-1), so nothing partial carries over.
        Task { _ = await controller.stopSession() }
    }

    private func finish() {
        isCapturing = false
        if ownsSession {
            camera.sessionDidEnd()
            let stopping = Task { _ = await controller.stopSession() }
            Task {
                await stopping.value
                onCalibrationComplete()
            }
        } else {
            // The session goes on into the battery; this view only hands over.
            camera.sessionDidEnd()
            onCalibrationComplete()
        }
    }

    // MARK: Controls

    private var captureControlsRow: some View {
        HStack {
            if onClose != nil {
                Button(action: handleClose) {
                    HStack(spacing: 4) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 16))
                        Text(Localizer.tr("close"))
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
                .accessibilityIdentifier("calibration_close")
            }
            Spacer()
        }
    }

    private func handleClose() {
        stopCapture()
        if let onClose {
            onClose()
        } else {
            dismiss()
        }
    }
}

// MARK: - One calibration round: calibration, result, validation, result

/// Holds the round's `GazeCalibrationFlow` and shows the screen for its phase. Used by the
/// wizard (through `BubbleCalibrationView`) and by a re-calibration inside the battery.
struct CalibrationRoundView: View {
    @ObservedObject var controller: GazeSessionController
    @State private var flow: GazeCalibrationFlow
    /// Called once when the round is ready; `true` when the operator went on against advice.
    let onReady: (Bool) -> Void

    init(controller: GazeSessionController, reason: GazeRecalReason?, onReady: @escaping (Bool) -> Void) {
        self.controller = controller
        self._flow = State(initialValue: GazeCalibrationFlow(reason: reason))
        self.onReady = onReady
    }

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            switch flow.phase {
            case let .calibrating(reason):
                CalibrationTargetsView(controller: controller, reason: reason) { result in
                    switch result {
                    case let .finished(quality, advice):
                        flow.calibrationFinished(quality, advice: advice)
                    case .unavailable:
                        flow.calibrationUnavailable()
                    }
                }
                .id("calibration-\(flow.attempts)")
            case let .calibrationResult(quality, canRetry):
                CalibrationResultCard(
                    quality: quality,
                    canRetry: canRetry,
                    onRetry: { flow.retry() },
                    onContinue: { flow.continueAnyway() }
                )
            case .validating:
                ValidationStepView(controller: controller) { advice, accuracyDeg in
                    flow.validationFinished(advice: advice, accuracyDeg: accuracyDeg)
                }
                .id("validation-\(flow.attempts)")
            case let .validationResult(advice, accuracyDeg):
                ValidationResultCard(
                    advice: advice,
                    accuracyDeg: accuracyDeg,
                    canRetry: flow.canRetry,
                    onRetry: { flow.retry() },
                    onContinue: { flow.continueAnyway() }
                )
            case .ready:
                ProgressView().tint(.white)
            }
        }
        .ignoresSafeArea()
        .onChange(of: flow.isReady) { _, ready in
            if ready { onReady(flow.continuedAnyway) }
        }
    }
}

/// Runs the calibration targets of one attempt through the pipeline.
struct CalibrationTargetsView: View {
    enum Result {
        case finished(GazeCalibrationQuality, GazeRecalAdviceRecord?)
        /// No live pipeline (Debug simulation): the session goes on without a calibration.
        case unavailable
    }

    @ObservedObject var controller: GazeSessionController
    let reason: GazeRecalReason?
    let onFinished: (Result) -> Void
    @StateObject private var stage = GazeTargetStage()
    @State private var celebrating = false

    var body: some View {
        ZStack {
            GazeTargetStageLayer(stage: stage)

            if celebrating {
                VStack(spacing: 10) {
                    Image(systemName: "sparkles")
                        .font(.system(size: 54))
                        .foregroundColor(LiquidGlass.warmAmber)
                    Text(Localizer.tr("calibrate_result_great"))
                        .font(.system(size: 26, weight: .bold, design: .rounded))
                        .foregroundColor(.white)
                }
                .transition(.scale.combined(with: .opacity))
            }

            VStack {
                Spacer()
                TargetHintPill(
                    text: Localizer.tr("target_calibration_hint"),
                    progress: stage.current.map { "\($0.index + 1) / \($0.count)" }
                )
                .padding(.bottom, 40)
            }
        }
        .task { await run() }
    }

    private func run() async {
        guard let schedule = await controller.targetSchedule() else {
            onFinished(.unavailable)
            return
        }
        let runner = stage.runner(for: controller, schedule: schedule)
        do {
            let outcome = try await runner.runCalibration(reason: reason)
            let advice = await controller.recalibrationAdvice()
            if outcome.quality == .good || outcome.quality == .fair {
                withAnimation(.spring(response: 0.4, dampingFraction: 0.7)) { celebrating = true }
                try? await Task.sleep(nanoseconds: 900_000_000)
            }
            onFinished(.finished(outcome.quality, advice))
        } catch is CancellationError {
            stage.clear()
        } catch GazeProcessingActor.TargetError.noPipeline {
            onFinished(.unavailable)
        } catch {
            // The camera stopped delivering frames, or the pipeline refused the calibration:
            // no fit, shown as a failed calibration with "try again" (CAL-3).
            onFinished(.finished(.failed, await controller.recalibrationAdvice()))
        }
    }
}

/// Result of a `poor` or `failed` calibration: "try again" (while the limit allows) or an
/// explicit "continue anyway"; the session record keeps the calibration quality (CAL-9.1).
struct CalibrationResultCard: View {
    let quality: GazeCalibrationQuality
    let canRetry: Bool
    let onRetry: () -> Void
    let onContinue: () -> Void

    var body: some View {
        OperatorCard(
            icon: "exclamationmark.triangle.fill",
            title: Localizer.tr(quality == .poor ? "calibrate_result_poor_title" : "calibrate_failed_title"),
            message: Localizer.tr(quality == .poor ? "calibrate_result_poor_body" : "calibrate_failed_body"),
            note: canRetry ? nil : Localizer.tr("calibrate_limit_reached"),
            primary: canRetry ? (Localizer.tr("calibrate_try_again"), "calibration_try_again", onRetry) : nil,
            secondary: (Localizer.tr("calibrate_continue_anyway"), "calibration_continue_anyway", onContinue)
        )
    }
}

/// Operator-facing card used by the result screens and the re-calibration prompt.
struct OperatorCard: View {
    let icon: String
    let title: String
    let message: String
    var note: String?
    let primary: (String, String, () -> Void)?
    let secondary: (String, String, () -> Void)?

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: icon)
                .font(.system(size: 30))
                .foregroundColor(LiquidGlass.warmAmber)
            Text(title)
                .font(.system(size: 18, weight: .bold))
                .multilineTextAlignment(.center)
            Text(message)
                .font(.system(size: 14))
                .multilineTextAlignment(.center)
            if let note {
                Text(note)
                    .font(.system(size: 12))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .multilineTextAlignment(.center)
            }
            if let primary {
                Button(action: primary.2) {
                    Text(primary.0)
                        .font(.system(size: 15, weight: .semibold))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 10)
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier(primary.1)
            }
            if let secondary {
                Button(action: secondary.2) {
                    Text(secondary.0)
                        .font(.system(size: 15, weight: .semibold))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 10)
                }
                .buttonStyle(.bordered)
                .accessibilityIdentifier(secondary.1)
            }
        }
        .padding(24)
        .glassCard(cornerRadius: 20, padding: 20)
        .padding(.horizontal, 24)
    }
}
