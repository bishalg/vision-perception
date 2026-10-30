import SwiftUI
import ARKit
import CurvAsdGazeKit

/// Onboarding Step 2 optional card: a short on-camera check that estimates the
/// subject's coarse age band and softly cross-checks it against the bracket the
/// caregiver just picked.
///
/// Assistive by design — it never blocks onboarding, never stores imagery, and
/// gracefully degrades on devices without a TrueDepth camera or without an
/// installed age model.
struct SubjectAgeQuickCheckCard: View {
    let selectedBracket: PediatricAgeBracket

    @StateObject private var checkController = SubjectAgeCheckController()
    @StateObject private var gazeController = GazeSessionController()
    @State private var isChecking = false
    @StateObject private var camera = CameraAccessCoordinator()

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            // Header
            HStack(spacing: 10) {
                ZStack {
                    Circle()
                        .fill(LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity))
                        .frame(width: 36, height: 36)
                    Image(systemName: "person.crop.square.badge.camera")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text("Optional: Quick Camera Check")
                        .font(.system(size: 15, weight: .bold, design: .rounded))
                        .foregroundColor(LiquidGlass.textPrimary)
                    Text("Confirms the right person is in front of the camera before sessions begin.")
                        .font(.system(size: 11))
                        .foregroundColor(LiquidGlass.textSecondary)
                }
                Spacer(minLength: 0)
            }

            checkBody

            if camera.state.showsDeniedView {
                CameraDeniedInlineNotice(state: camera.state)
            }

            // Privacy reassurance line
            HStack(spacing: 5) {
                Image(systemName: "lock.shield.fill")
                    .font(.system(size: 10))
                    .foregroundColor(LiquidGlass.emeraldMint)
                Text("Runs entirely on this device. No photos or videos are ever captured or stored — only an age-group estimate.")
                    .font(.system(size: 10))
                    .foregroundColor(LiquidGlass.textTertiary)
                    .lineLimit(2)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(cornerRadius: 16, padding: 16)
        .onChange(of: gazeController.subjectAgeSignal) { _, signal in
            checkController.observe(signal)
        }
        .onDisappear(perform: stopCheck)
        .cameraScenePhase(camera) { action in
            if action == .stopSession { stopCheck() }
        }
    }

    // MARK: - Body States

    @ViewBuilder
    private var checkBody: some View {
        if !ARFaceTrackingConfiguration.isSupported {
            Label("This check needs a Face ID camera (TrueDepth). Setup continues fine without it.",
                  systemImage: "info.circle")
                .font(.system(size: 12))
                .foregroundColor(LiquidGlass.textTertiary)
        } else if !gazeController.subjectAgeEstimator.isActive {
            Label("Age model not installed on this device — skipping the check.",
                  systemImage: "info.circle")
                .font(.system(size: 12))
                .foregroundColor(LiquidGlass.textTertiary)
        } else if isChecking {
            liveCheckSection
        } else {
            startButton
        }
    }

    private var startButton: some View {
        Button {
            Task { await startCheck() }
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "camera.viewfinder")
                    .font(.system(size: 14, weight: .semibold))
                Text("Start 5-second check")
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 11)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(LiquidGlass.celestialTeal.opacity(0.5), lineWidth: 1)
            )
            .foregroundColor(LiquidGlass.celestialTeal)
        }
        .buttonStyle(.plain)
    }

    private var liveCheckSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            // Live signal readout
            HStack(spacing: 10) {
                if let line = SubjectAgeCheckController.statusLine(for: checkController.lastSignal) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(LiquidGlass.emeraldMint)
                    Text(line)
                        .font(.system(size: 13, weight: .semibold, design: .rounded))
                        .foregroundColor(LiquidGlass.textPrimary)
                } else {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .tint(LiquidGlass.celestialTeal)
                        .frame(width: 18, height: 18)
                    Text("Looking for a stable face reading… hold the device like you would during a session.")
                        .font(.system(size: 12))
                        .foregroundColor(LiquidGlass.textSecondary)
                }
                Spacer(minLength: 0)
            }

            verdictSection

            Button {
                stopCheck()
            } label: {
                Text("Done")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 9)
                    .background(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(LiquidGlass.cardInset)
                    )
                    .foregroundColor(LiquidGlass.textSecondary)
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder
    private var verdictSection: some View {
        switch checkController.currentVerdict {
        case .insufficientData:
            EmptyView()
        case .consistent:
            Label("The face in view matches the selected age group (\(selectedBracket.rawValue)).",
                  systemImage: "hand.thumbsup.fill")
                .font(.system(size: 12, weight: .medium))
                .foregroundColor(LiquidGlass.emeraldMint)
        case .softConfirm:
            if !checkController.dismissedSoftConfirm {
                VStack(alignment: .leading, spacing: 8) {
                    Label("This looks like an adult face. Sessions are designed for your child — keep your child in front of the camera during the observation.",
                          systemImage: "exclamationmark.triangle.fill")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(LiquidGlass.warmAmber)
                    Button {
                        checkController.dismissSoftConfirm()
                    } label: {
                        Text("Thanks — noted")
                            .font(.system(size: 12, weight: .semibold, design: .rounded))
                            .padding(.horizontal, 14)
                            .padding(.vertical, 7)
                            .background(Capsule().fill(LiquidGlass.warmAmber.opacity(LiquidGlass.tintOpacity)))
                            .foregroundColor(LiquidGlass.warmAmber)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    // MARK: - Actions

    private func startCheck() async {
        isChecking = true
        checkController.reset()
        // CAM-1: no session until camera access is granted; denied ends the check.
        guard await camera.ensureAccess(consentStore: .shared) == .granted else {
            isChecking = false
            return
        }
        // CNS-1: consent-gated start; if consent is missing nothing starts and the check ends.
        if gazeController.startSessionWithConsent() {
            camera.sessionDidStart()
        } else {
            isChecking = false
        }
    }

    private func stopCheck() {
        guard isChecking else { return }
        isChecking = false
        camera.sessionDidEnd()
        Task { _ = await gazeController.stopSession() }
    }
}
