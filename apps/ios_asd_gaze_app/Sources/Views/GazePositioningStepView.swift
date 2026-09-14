import SwiftUI
import CurvAsdGazeKit

// MARK: - Distance State

private enum DistanceState {
    case noFace
    case tooClose
    case sweetSpot
    case tooFar

    var color: Color {
        switch self {
        case .noFace:    return LiquidGlass.textSecondary
        case .tooClose:  return LiquidGlass.warmAmber
        case .sweetSpot: return LiquidGlass.emeraldMint
        case .tooFar:    return LiquidGlass.warmAmber
        }
    }

    var icon: String {
        switch self {
        case .noFace:    return "viewfinder"
        case .tooClose:  return "arrow.backward.circle.fill"
        case .sweetSpot: return "checkmark.circle.fill"
        case .tooFar:    return "arrow.forward.circle.fill"
        }
    }

    func label() -> String {
        switch self {
        case .noFace:    return Localizer.tr("wizard_no_face")
        case .tooClose:  return Localizer.tr("wizard_distance_close")
        case .sweetSpot: return Localizer.tr("wizard_distance_good")
        case .tooFar:    return Localizer.tr("wizard_distance_far")
        }
    }
}

// MARK: - Step 1: Gaze Positioning View

/// Wizard Step 1 — Full-screen front camera preview with:
/// • Face alignment sweet-spot oval guide (emerald when verified)
/// • Minimum 5s face preview observation + 3s continuous sweet-spot detection
/// • Manual "Next: Start Calibration" button, enabled ONLY once the verification is complete
/// • Clean top bar (no close button)
public struct GazePositioningStepView: View {
    @StateObject private var controller = GazeSessionController()
    @State private var loopTimer: Timer?
    @State private var elapsedPreviewSec: Double = 0.0
    @State private var continuousDetectionSec: Double = 0.0
    @State private var isVerified: Bool = false
    @State private var ovalPulseScale: CGFloat = 1.0

    public let onReady: () -> Void

    private let minPreviewSec: Double = 5.0
    private let requiredContinuousSec: Double = 3.0

    public init(onReady: @escaping () -> Void) {
        self.onReady = onReady
    }

    public var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            ARFaceCameraView(
                session: controller.arSession,
                isSimulation: controller.isSimulationMode,
                headYawDeg: controller.headYawDeg,
                headPitchDeg: controller.headPitchDeg,
                smileIntensity: controller.currentAffect.socialSmileIntensity,
                surpriseScore: controller.currentAffect.surpriseScore,
                distressScore: controller.currentAffect.distressScore,
                dominantEmotion: controller.currentAffect.dominantEmotion,
                isValid: controller.currentQuality.isValid
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                topBar
                Spacer()
                sweetSpotGuide
                Spacer()
                bottomStatusBar
            }
        }
        .onAppear(perform: startTracking)
        .onDisappear(perform: stopTracking)
    }

    // MARK: - Top Bar (Clean, no close button)

    private var topBar: some View {
        HStack {
            Spacer()
            VStack(spacing: 2) {
                Text(Localizer.tr("wizard_step1_title"))
                    .font(.system(size: 17, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
                Text(Localizer.tr("wizard_step1_subtitle"))
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(Color.white.opacity(0.65))
            }
            .padding(.leading, 36)
            Spacer()
            stepPill.padding(.trailing, 16)
        }
        .padding(.vertical, 12)
        .background(.ultraThinMaterial.opacity(0.7))
    }

    private var stepPill: some View {
        HStack(spacing: 4) {
            ForEach(1..<4) { step in
                RoundedRectangle(cornerRadius: 3, style: .continuous)
                    .fill(step == 1 ? LiquidGlass.celestialTeal : Color.white.opacity(0.3))
                    .frame(width: step == 1 ? 20 : 8, height: 5)
            }
        }
        .accessibilityLabel("Step 1 of 3")
    }

    // MARK: - Sweet Spot Oval Guide

    private var sweetSpotGuide: some View {
        GeometryReader { geo in
            let w = geo.size.width
            let h = geo.size.height
            let ovalW = w * 0.62
            let ovalH = h * 0.68

            ZStack {
                // Guide oval: solid emerald when verified, animated dashed line during alignment
                Ellipse()
                    .strokeBorder(
                        isVerified ? LiquidGlass.emeraldMint : distanceState.color.opacity(0.7),
                        style: isVerified
                            ? StrokeStyle(lineWidth: 3.5)
                            : StrokeStyle(lineWidth: 2.5, dash: [10, 6])
                    )
                    .frame(width: ovalW, height: ovalH)
                    .scaleEffect(ovalPulseScale)
                    .position(x: w * 0.5, y: h * 0.5)
                    .shadow(
                        color: isVerified ? LiquidGlass.emeraldMint.opacity(0.6) : .clear,
                        radius: 12
                    )

                // Progress ring & checkmark indicator below the oval
                ZStack {
                    Circle()
                        .stroke(Color.white.opacity(0.2), lineWidth: 3.5)
                        .frame(width: 44, height: 44)

                    Circle()
                        .trim(from: 0, to: CGFloat(min(continuousDetectionSec / requiredContinuousSec, 1.0)))
                        .stroke(
                            LiquidGlass.emeraldMint,
                            style: StrokeStyle(lineWidth: 4, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .frame(width: 44, height: 44)
                        .animation(.linear(duration: 0.05), value: continuousDetectionSec)

                    if isVerified {
                        Image(systemName: "checkmark")
                            .font(.system(size: 18, weight: .bold))
                            .foregroundColor(LiquidGlass.emeraldMint)
                            .transition(.scale.combined(with: .opacity))
                    } else if distanceState == .sweetSpot {
                        Text(String(format: "%.0f", ceil(max(0, requiredContinuousSec - continuousDetectionSec))))
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.emeraldMint)
                    } else {
                        Image(systemName: "face.dashed")
                            .font(.system(size: 16))
                            .foregroundColor(Color.white.opacity(0.6))
                    }
                }
                .position(x: w * 0.5, y: h * 0.5 + ovalH * 0.46)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Bottom Status Bar

    private var bottomStatusBar: some View {
        VStack(spacing: 12) {
            // Distance status readout
            HStack(spacing: 8) {
                Image(systemName: isVerified ? "checkmark.circle.fill" : distanceState.icon)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundColor(isVerified ? LiquidGlass.emeraldMint : distanceState.color)
                Text(isVerified ? "Face Aligned & Verified" : distanceState.label())
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
                    .foregroundColor(.white)
                    .lineLimit(1)
                Spacer()
                Text(String(format: "%.0f cm", controller.currentDistanceCm))
                    .font(.system(size: 14, weight: .bold, design: .monospaced))
                    .foregroundColor(isVerified ? LiquidGlass.emeraldMint : distanceState.color)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 10)
            .background(.ultraThinMaterial.opacity(0.75))
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))

            // Manual Start Calibration Button — enabled only when verified
            Button(action: advance) {
                HStack(spacing: 8) {
                    if isVerified {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 16, weight: .bold))
                    }
                    Text(buttonTitle)
                        .font(.system(size: 15, weight: .semibold, design: .rounded))
                    if isVerified {
                        Image(systemName: "arrow.right")
                            .font(.system(size: 14, weight: .bold))
                    }
                }
                .foregroundColor(isVerified ? .black : Color.white.opacity(0.75))
                .padding(.vertical, 14)
                .frame(maxWidth: .infinity)
                .background(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .fill(isVerified ? LiquidGlass.emeraldMint : Color.white.opacity(0.15))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .stroke(isVerified ? Color.clear : LiquidGlass.cardBorder, lineWidth: 1)
                )
                .shadow(
                    color: isVerified ? LiquidGlass.emeraldMint.opacity(0.45) : .clear,
                    radius: 10, y: 4
                )
            }
            .buttonStyle(.plain)
            .disabled(!isVerified)
        }
        .padding(.horizontal, 20)
        .padding(.bottom, 28)
        .background(.ultraThinMaterial.opacity(0.55))
    }

    private var buttonTitle: String {
        if isVerified {
            return Localizer.tr("wizard_ready_calibrate")
        } else if distanceState != .sweetSpot {
            return "Align Face in Center Oval"
        } else if continuousDetectionSec < requiredContinuousSec {
            return "Hold Steady (\(String(format: "%.1f", continuousDetectionSec))s / 3.0s)"
        } else {
            return "Verifying Face Pose (\(Int(ceil(max(0, minPreviewSec - elapsedPreviewSec))))s)"
        }
    }

    // MARK: - Distance State

    private var distanceState: DistanceState {
        guard controller.trackingStage.hasFace else { return .noFace }
        let d = controller.currentDistanceCm
        if d < 32 { return .tooClose }
        if d > 60 { return .tooFar }
        return .sweetSpot
    }

    // MARK: - Actions

    private func startTracking() {
        controller.startSession()
        elapsedPreviewSec = 0.0
        continuousDetectionSec = 0.0
        isVerified = false

        loopTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
            MainActor.assumeIsolated {
                let dt = 1.0 / 60.0
                elapsedPreviewSec += dt

                if controller.isSimulationMode {
                    controller.tickSimulatedFrame(timeSec: elapsedPreviewSec)
                }

                if distanceState == .sweetSpot {
                    continuousDetectionSec += dt
                    if !isVerified && continuousDetectionSec >= requiredContinuousSec && elapsedPreviewSec >= minPreviewSec {
                        withAnimation(.spring(response: 0.4, dampingFraction: 0.7)) {
                            isVerified = true
                            ovalPulseScale = 1.0
                        }
                    } else if !isVerified {
                        withAnimation(.easeInOut(duration: 0.6)) {
                            ovalPulseScale = 1.02
                        }
                    }
                } else if !isVerified {
                    continuousDetectionSec = max(0, continuousDetectionSec - dt * 2.0)
                    ovalPulseScale = 1.0
                }
            }
        }
    }

    private func stopTracking() {
        loopTimer?.invalidate()
        loopTimer = nil
        _ = controller.stopSession()
    }

    private func advance() {
        guard isVerified else { return }
        stopTracking()
        onReady()
    }
}
