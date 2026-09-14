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
/// • Face alignment sweet-spot oval guide
/// • Real-time distance meter (38–55 cm target range)
/// • Head pose gate (|yaw| < 20°)
/// • Auto-advance ring: fills over 1.5 s while in the sweet spot, then calls `onReady`
public struct GazePositioningStepView: View {
    @StateObject private var controller = GazeSessionController()
    @State private var loopTimer: Timer?
    @State private var steadySec: Double = 0.0
    @State private var ovalPulseScale: CGFloat = 1.0
    @State private var didAutoAdvance: Bool = false

    public let onReady: () -> Void
    public var onClose: (() -> Void)?

    private let requiredHoldSec: Double = 1.5

    public init(onReady: @escaping () -> Void, onClose: (() -> Void)? = nil) {
        self.onReady = onReady
        self.onClose = onClose
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

    // MARK: - Top Bar

    private var topBar: some View {
        HStack {
            if let onClose {
                Button(action: onClose) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundColor(Color.white.opacity(0.75))
                        .padding(14)
                }
            }
            Spacer()
            VStack(spacing: 2) {
                Text(Localizer.tr("wizard_step1_title"))
                    .font(.system(size: 17, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
                Text(Localizer.tr("wizard_step1_subtitle"))
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(Color.white.opacity(0.65))
            }
            .padding(.leading, onClose != nil ? 0 : 16)
            Spacer()
            stepPill.padding(.trailing, 14)
        }
        .padding(.top, 8)
        .background(.ultraThinMaterial.opacity(0.7))
    }

    private var stepPill: some View {
        HStack(spacing: 3) {
            ForEach(1..<4) { step in
                RoundedRectangle(cornerRadius: 3, style: .continuous)
                    .fill(step == 1 ? LiquidGlass.celestialTeal : Color.white.opacity(0.3))
                    .frame(width: step == 1 ? 18 : 9, height: 5)
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
                // Dashed guide oval
                Ellipse()
                    .strokeBorder(
                        distanceState.color.opacity(0.65),
                        style: StrokeStyle(lineWidth: 2.5, dash: [10, 6])
                    )
                    .frame(width: ovalW, height: ovalH)
                    .scaleEffect(ovalPulseScale)
                    .position(x: w * 0.5, y: h * 0.5)

                // Auto-advance progress ring centred below the oval
                if distanceState == .sweetSpot {
                    Circle()
                        .trim(from: 0, to: CGFloat(min(steadySec / requiredHoldSec, 1.0)))
                        .stroke(
                            LiquidGlass.emeraldMint,
                            style: StrokeStyle(lineWidth: 4, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .frame(width: 36, height: 36)
                        .position(x: w * 0.5, y: h * 0.5 + ovalH * 0.46)
                        .animation(.linear(duration: 0.05), value: steadySec)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Bottom Status Bar

    private var bottomStatusBar: some View {
        VStack(spacing: 10) {
            // Distance status pill
            HStack(spacing: 8) {
                Image(systemName: distanceState.icon)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundColor(distanceState.color)
                Text(distanceState.label())
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
                    .foregroundColor(.white)
                    .lineLimit(1)
                Spacer()
                Text(String(format: "%.0f cm", controller.currentDistanceCm))
                    .font(.system(size: 14, weight: .bold, design: .monospaced))
                    .foregroundColor(distanceState.color)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 10)
            .background(.ultraThinMaterial.opacity(0.75))
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))

            // Hold-steady countdown
            if distanceState == .sweetSpot && steadySec > 0.4 && steadySec < requiredHoldSec {
                Text("\(Localizer.tr("wizard_hold_steady")) \(String(format: "%.1f", requiredHoldSec - steadySec))s…")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(LiquidGlass.emeraldMint.opacity(0.9))
            }

            // Manual advance button
            Button(action: advance) {
                HStack(spacing: 8) {
                    Image(systemName: "arrow.right.circle.fill")
                        .font(.system(size: 16, weight: .semibold))
                    Text(Localizer.tr("wizard_ready_calibrate"))
                        .font(.system(size: 15, weight: .semibold, design: .rounded))
                }
                .foregroundColor(.black)
                .padding(.vertical, 13)
                .frame(maxWidth: .infinity)
                .background(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .fill(distanceState == .sweetSpot ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal)
                )
                .shadow(
                    color: LiquidGlass.emeraldMint.opacity(distanceState == .sweetSpot ? 0.45 : 0),
                    radius: 10, y: 4
                )
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 20)
        .padding(.bottom, 28)
        .background(.ultraThinMaterial.opacity(0.5))
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
        loopTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { _ in
            MainActor.assumeIsolated {
                if distanceState == .sweetSpot {
                    steadySec += 1.0 / 30.0
                    if !didAutoAdvance && steadySec >= requiredHoldSec {
                        didAutoAdvance = true
                        advance()
                    }
                    withAnimation(.easeInOut(duration: 0.8)) { ovalPulseScale = 1.025 }
                } else {
                    steadySec = max(0, steadySec - (1.0 / 30.0) * 3.0)
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
        stopTracking()
        onReady()
    }
}
