import SwiftUI
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

// MARK: - Main Interactive Calibration View

/// Fully hands-free 5-point child-friendly balloon popping calibration view.
///
/// Attention-guided timed calibration (not gaze-gated hit testing): raw TrueDepth
/// `lookAtPoint` gaze has an intrinsic 3-6 cm error on the glass — most of a phone's
/// 7.1 cm width — so requiring a raw gaze *hit* on a small target before advancing
/// was the actual defect here (chicken-and-egg: you need calibration to hit the
/// target, and you need to hit the target to calibrate). Instead: each balloon
/// holds attention for a fixed ~1.6s window regardless of where raw gaze lands,
/// raw samples are collected during that window, the balloon auto-pops, and after
/// all 5 targets a per-user affine correction (`GazeCalibrationProfile`) is fit and
/// persisted — applied to every gaze sample in every session from then on.
public struct BubbleCalibrationView: View {
    @Environment(\.dismiss) private var dismiss
    @StateObject private var controller = GazeSessionController()

    @State private var currentTargetIndex: Int = 0
    @State private var dwellProgress: Double = 0.0
    @State private var isPopping: Bool = false
    @State private var isLockedOnTarget: Bool = false
    @State private var stringSway: CGFloat = 0.0
    @State private var loopTimer: Timer?

    // Gaze smoothing (display only — no longer gates advancement, see updateFramePhysics)
    @State private var smoothedGaze: CGPoint = CGPoint(x: 0.5, y: 0.5)
    @State private var currentScreenSize: CGSize = CGSize(width: 393, height: 852)
    @State private var popShockwaveScale: CGFloat = 0.0
    @State private var showGazeReticle: Bool = false

    // Attention-guided calibration state
    @State private var windowElapsedSec: Double = 0.0
    /// Consecutive frames with no valid face/eye tracking. A short run is a
    /// tolerated blink (keeps the window ticking); a longer run freezes the
    /// window entirely — see `updateFramePhysics`.
    @State private var trackingDropoutFrames: Int = 0
    /// Raw (uncalibrated) gaze samples collected for the in-progress target.
    @State private var collectedRawSamples: [CGPoint] = []
    /// One robust (median) raw point per completed target, in target order —
    /// fit against `targetPoints` once all 5 are collected.
    @State private var fittedRawPoints: [CGPoint] = []

    public let onCalibrationComplete: () -> Void
    public var onClose: (() -> Void)?

    // 5 anchor points in normalized coordinates [x, y]
    private let targetPoints: [CGPoint] = [
        CGPoint(x: 0.50, y: 0.50), // 1. Center
        CGPoint(x: 0.22, y: 0.24), // 2. Top-Left
        CGPoint(x: 0.78, y: 0.24), // 3. Top-Right
        CGPoint(x: 0.22, y: 0.76), // 4. Bottom-Left
        CGPoint(x: 0.78, y: 0.76)  // 5. Bottom-Right
    ]

    // Balloon themes for visual engagement across points
    private let balloonColors: [[Color]] = [
        [LiquidGlass.celestialTeal, Color(red: 0.0, green: 0.6, blue: 0.9)],
        [Color(red: 1.0, green: 0.35, blue: 0.55), Color(red: 0.9, green: 0.1, blue: 0.4)],
        [Color(red: 0.3, green: 0.85, blue: 0.4), LiquidGlass.emeraldMint],
        [Color(red: 1.0, green: 0.65, blue: 0.2), LiquidGlass.warmAmber],
        [Color(red: 0.6, green: 0.4, blue: 0.95), Color(red: 0.4, green: 0.2, blue: 0.85)]
    ]

    // Attention-guided calibration timing (Option 1: timed, not gaze-hit-gated —
    // raw TrueDepth gaze has 3-6 cm intrinsic error, more than a phone is wide,
    // so requiring a raw hit before advancing was the actual defect).
    private let attentionWindowSec: Double = 1.6        // total time each balloon holds attention
    private let settleDelaySec: Double = 0.4            // ignore the initial saccade landing on the new target
    private let softFeedbackRadiusPoints: CGFloat = 160.0 // visual-only "looking close" glow, never gates progress

    public init(
        onClose: (() -> Void)? = nil,
        onCalibrationComplete: @escaping () -> Void
    ) {
        self.onClose = onClose
        self.onCalibrationComplete = onCalibrationComplete
    }

    public var body: some View {
        NavigationStack {
            GeometryReader { geometry in
                ZStack {
                    // Adaptive Liquid Glass canvas (dark twilight in dark mode, airy porcelain in light mode)
                    LinearGradient(
                        colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                    .ignoresSafeArea()

                    // Top distance and gaze guidance HUD
                    VStack(spacing: 8) {
                        topGuidanceHUD
                        Spacer()
                    }
                    .padding(.horizontal, 16)
                    .padding(.top, 8)
                    .zIndex(10)

                    // Active Target Balloon
                    if currentTargetIndex < targetPoints.count {
                        let target = targetPoints[currentTargetIndex]
                        let targetX = target.x * geometry.size.width
                        let targetY = target.y * geometry.size.height

                        activeBalloonView(targetIndex: currentTargetIndex)
                            .position(x: targetX, y: targetY)
                            .zIndex(5)
                    }

                    // Real-Time Gaze Reticle (visual feedback showing where the eyes are looking)
                    liveGazeReticle(in: geometry.size)
                        .zIndex(8)

                    // Bottom instruction hint
                    VStack {
                        Spacer()
                        bottomHintPill
                    }
                    .padding(.bottom, 20)
                    .zIndex(10)
                }
                .onAppear {
                    currentScreenSize = geometry.size
                }
                .onChange(of: geometry.size) { _, newSize in
                    currentScreenSize = newSize
                }
            }
            .navigationTitle(Localizer.tr("calibrate_title"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                if let onClose {
                    ToolbarItem(placement: .cancellationAction) {
                        Button(action: handleClose) {
                            HStack(spacing: 4) {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.system(size: 16))
                                Text(Localizer.tr("close"))
                            }
                            .foregroundColor(LiquidGlass.celestialTeal)
                        }
                    }
                }

                ToolbarItem(placement: .primaryAction) {
                    Button {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            showGazeReticle.toggle()
                        }
                    } label: {
                        Image(systemName: showGazeReticle ? "eye.fill" : "eye.slash")
                            .font(.system(size: 15))
                            .foregroundColor(showGazeReticle ? LiquidGlass.emeraldMint : LiquidGlass.textSecondary)
                    }
                    .accessibilityLabel("Toggle Gaze Cursor")
                }
            }
            .onAppear {
                startCalibrationSession()
            }
            .onDisappear {
                stopCalibrationSession()
            }
        }
    }

    // MARK: - Balloon View with Dwell Inflation

    private func activeBalloonView(targetIndex: Int) -> some View {
        let colors = balloonColors[targetIndex % balloonColors.count]
        // Scale expands from 1.0 up to 1.50 as eyes fixate
        let balloonScale: CGFloat = isPopping ? 1.65 : (1.0 + CGFloat(dwellProgress) * 0.48)

        return ZStack {
            if isPopping {
                // Expanding Shockwave rings
                Circle()
                    .stroke(colors[0].opacity(max(0, 1.0 - popShockwaveScale)), lineWidth: 3.5)
                    .frame(width: 60 + popShockwaveScale * 140, height: 60 + popShockwaveScale * 140)

                Circle()
                    .stroke(LiquidGlass.emeraldMint.opacity(max(0, 0.8 - popShockwaveScale)), lineWidth: 2)
                    .frame(width: 40 + popShockwaveScale * 90, height: 40 + popShockwaveScale * 90)

                // 12 Radial Confetti particles bursting outward
                ForEach(0..<12, id: \.self) { i in
                    let angle = Double(i) * (.pi / 6.0)
                    let distance = 25.0 + Double(popShockwaveScale) * (50.0 + Double(i % 3) * 22.0)
                    let particleColor: Color = {
                        switch i % 4 {
                        case 0: return colors[0]
                        case 1: return colors[1]
                        case 2: return LiquidGlass.emeraldMint
                        default: return LiquidGlass.warmAmber
                        }
                    }()

                    Circle()
                        .fill(particleColor)
                        .frame(width: max(3, 9 - popShockwaveScale * 5), height: max(3, 9 - popShockwaveScale * 5))
                        .offset(
                            x: cos(angle) * distance,
                            y: sin(angle) * distance
                        )
                        .opacity(max(0, 1.0 - popShockwaveScale * 0.95))
                }

                // 4 Sparkle bursts
                ForEach(0..<4, id: \.self) { i in
                    let angle = Double(i) * (.pi / 2.0) + .pi / 4.0
                    let distance = 20.0 + Double(popShockwaveScale) * 55.0
                    Image(systemName: "sparkle")
                        .font(.system(size: 15))
                        .foregroundColor(LiquidGlass.warmAmber)
                        .offset(x: cos(angle) * distance, y: sin(angle) * distance)
                        .rotationEffect(.degrees(popShockwaveScale * 120.0))
                        .opacity(max(0, 1.0 - popShockwaveScale))
                }
            } else {
                // Outer Dwell Timer Circular Ring
                Circle()
                    .stroke(LiquidGlass.cardBorder, lineWidth: 4)
                    .frame(width: 110, height: 110)

                Circle()
                    .trim(from: 0.0, to: CGFloat(dwellProgress))
                    .stroke(
                        LinearGradient(
                            colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        style: StrokeStyle(lineWidth: 6, lineCap: .round)
                    )
                    .frame(width: 110, height: 110)
                    .rotationEffect(.degrees(-90))
                    .shadow(color: LiquidGlass.emeraldMint.opacity(isLockedOnTarget ? 0.9 : 0.4), radius: 8)

                // Balloon String
                BalloonStringShape(sway: stringSway)
                    .stroke(LiquidGlass.textSecondary.opacity(0.7), lineWidth: 1.8)
                    .frame(width: 20, height: 44)
                    .offset(y: 54)

                // Balloon Body
                ZStack {
                    BalloonShape()
                        .fill(
                            RadialGradient(
                                colors: [Color.white.opacity(0.35), colors[0], colors[1]],
                                center: UnitPoint(x: 0.35, y: 0.3),
                                startRadius: 4,
                                endRadius: 52
                            )
                        )
                        .frame(width: 70, height: 88)
                        .shadow(color: colors[0].opacity(isLockedOnTarget ? 0.95 : 0.4), radius: isLockedOnTarget ? 20 : 8)

                    // Specular glossy reflection highlight
                    Ellipse()
                        .fill(Color.white.opacity(0.6))
                        .frame(width: 13, height: 28)
                        .rotationEffect(.degrees(-25))
                        .offset(x: -14, y: -16)
                }
                .scaleEffect(balloonScale)
                .animation(.interactiveSpring(response: 0.22, dampingFraction: 0.72), value: balloonScale)
            }
        }
        .contentShape(Circle().size(width: 120, height: 120))
        .onTapGesture {
            // Optional caregiver tap fallback (hands-free is default)
            triggerPop()
        }
    }

    // MARK: - Live Gaze Reticle

    @ViewBuilder
    private func liveGazeReticle(in size: CGSize) -> some View {
        if showGazeReticle {
            let posX = smoothedGaze.x * size.width
            let posY = smoothedGaze.y * size.height

            ZStack {
                // Subtle outer glow ring
                Circle()
                    .stroke(isLockedOnTarget ? LiquidGlass.emeraldMint.opacity(0.85) : LiquidGlass.celestialTeal.opacity(0.6), lineWidth: 1.8)
                    .frame(width: 24, height: 24)

                // Inner core dot
                Circle()
                    .fill(isLockedOnTarget ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal)
                    .frame(width: 8, height: 8)
                    .shadow(color: Color.black.opacity(0.25), radius: 3)
            }
            .position(x: posX, y: posY)
            .opacity(controller.currentQuality.isValid ? 0.85 : 0.0)
            .animation(.linear(duration: 0.033), value: smoothedGaze)
            .allowsHitTesting(false)
        }
    }

    // MARK: - Guidance & Distance HUD

    private var topGuidanceHUD: some View {
        VStack(spacing: 6) {
            HStack(spacing: 10) {
                // Step progress badge
                HStack(spacing: 4) {
                    Image(systemName: "scope")
                        .font(.system(size: 13, weight: .bold))
                    Text("\(currentTargetIndex + 1) / \(targetPoints.count)")
                        .font(.system(size: 13, weight: .bold, design: .monospaced))
                }
                .foregroundColor(LiquidGlass.celestialTeal)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Capsule().fill(LiquidGlass.cardBackground))
                .overlay(Capsule().stroke(LiquidGlass.cardBorder, lineWidth: 0.8))

                Spacer()

                // Distance guidance meter (40-50 cm)
                distanceBadge

                // Eye tracking lock badge
                trackingLockBadge
            }

            // Dwell progress bar indicator
            if isLockedOnTarget {
                HStack(spacing: 6) {
                    ProgressView(value: dwellProgress, total: 1.0)
                        .tint(LiquidGlass.emeraldMint)
                        .frame(maxWidth: 160)
                    Text("\(Int(dwellProgress * 100))%")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .foregroundColor(LiquidGlass.emeraldMint)
                }
                .transition(.opacity)
            }
        }
        .glassCard(cornerRadius: 14, padding: 10)
    }

    private var distanceBadge: some View {
        let dist = Int(controller.currentDistanceCm)
        let isOptimal = dist >= 38 && dist <= 52
        let statusColor = isOptimal ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber
        let iconName = isOptimal ? "arrow.left.and.right.circle.fill" : "exclamationmark.triangle.fill"

        return HStack(spacing: 4) {
            Image(systemName: iconName)
                .font(.system(size: 11))
            Text("\(dist) cm (40–50)")
                .font(.system(size: 11, weight: .bold, design: .monospaced))
        }
        .foregroundColor(statusColor)
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(Capsule().fill(statusColor.opacity(0.18)))
    }

    private var trackingLockBadge: some View {
        let isTracked = controller.currentQuality.isValid
        let color = isTracked ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber

        return Image(systemName: isTracked ? "eye.fill" : "eye.slash")
            .font(.system(size: 14, weight: .bold))
            .foregroundColor(color)
            .padding(6)
            .background(Circle().fill(color.opacity(0.18)))
    }

    private var bottomHintPill: some View {
        HStack(spacing: 6) {
            Image(systemName: isLockedOnTarget ? "sparkles" : "eyes")
                .foregroundColor(isLockedOnTarget ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal)
                .font(.system(size: 14))

            Text(isLockedOnTarget ? Localizer.tr("popping_locked_hint") : Localizer.tr("hands_free_hint"))
                .font(.system(size: 13, weight: .medium, design: .rounded))
                .foregroundColor(LiquidGlass.textPrimary)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .glassCard(cornerRadius: 14, padding: 4)
    }

    // MARK: - Game Loop & Fixation Physics

    private func startCalibrationSession() {
        currentTargetIndex = 0
        controller.calibrationTarget = targetPoints[0]
        controller.startSession()
        loopTimer?.invalidate()
        dwellProgress = 0.0
        windowElapsedSec = 0.0
        trackingDropoutFrames = 0
        collectedRawSamples = []
        fittedRawPoints = []
        isPopping = false
        isLockedOnTarget = false

        loopTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { _ in
            updateFramePhysics()
        }
    }

    private func stopCalibrationSession() {
        controller.calibrationTarget = nil
        loopTimer?.invalidate()
        loopTimer = nil
        controller.stopSession()
    }

    private func updateFramePhysics() {
        // Pause physics checks during pop explosion
        guard !isPopping else { return }
        guard currentTargetIndex < targetPoints.count else { return }

        let now = Date().timeIntervalSince1970

        // Subtle balloon string sway
        stringSway = sin(now * 2.5)

        // Advance simulated frame if in simulation mode
        if controller.isSimulationMode {
            controller.tickSimulatedFrame(timeSec: now)
        }

        // Display-only smoothing of the RAW point (never the calibrated one —
        // this screen is what produces the calibration, so it must show and
        // collect the uncorrected signal).
        let rawGaze = controller.currentRawGazePoint
        smoothedGaze = CGPoint(
            x: smoothedGaze.x * 0.65 + rawGaze.x * 0.35,
            y: smoothedGaze.y * 0.65 + rawGaze.y * 0.35
        )

        let target = targetPoints[currentTargetIndex]
        let targetPt = CGPoint(
            x: target.x * currentScreenSize.width,
            y: target.y * currentScreenSize.height
        )
        let gazePt = CGPoint(
            x: smoothedGaze.x * currentScreenSize.width,
            y: smoothedGaze.y * currentScreenSize.height
        )
        let distancePoints = hypot(gazePt.x - targetPt.x, gazePt.y - targetPt.y)

        // Soft visual feedback only (glow radius is generous — 160pt — because
        // uncalibrated gaze legitimately drifts this far from the true target;
        // this never blocks or decays progress, unlike the old hit-test gate).
        isLockedOnTarget = distancePoints < softFeedbackRadiusPoints && controller.currentQuality.isValid

        // Progress is wall-clock time, NOT gaze accuracy — that's the Option 1
        // fix. But time must still only advance while a face is actually being
        // tracked: a short dropout (a blink) is tolerated, but a sustained one
        // (no face in frame at all) freezes progress instead of ticking blindly.
        // Ticking unconditionally was the regression — it popped balloons with
        // nobody in frame because 1.6s of wall-clock time always elapses.
        if controller.currentQuality.isValid {
            trackingDropoutFrames = 0
            windowElapsedSec += 1.0 / 60.0
            // Collect raw samples only once the initial saccade has settled.
            if windowElapsedSec >= settleDelaySec {
                collectedRawSamples.append(controller.currentRawGazePoint)
            }
        } else {
            trackingDropoutFrames += 1
            // ~250ms grace for a natural blink: keep the window running so a
            // blink mid-fixation doesn't restart the whole target. Beyond that,
            // freeze — this is a real dropout (looked away, left the frame),
            // not a blink, and must never silently finish the window.
            if trackingDropoutFrames <= 15 {
                windowElapsedSec += 1.0 / 60.0
            }
        }
        dwellProgress = min(1.0, windowElapsedSec / attentionWindowSec)

        if windowElapsedSec >= attentionWindowSec && !isPopping {
            triggerPop()
        }
    }

    /// Robust per-axis median — resistant to the occasional blink/saccade
    /// outlier that sneaks past the quality gate, unlike a plain mean.
    private func medianPoint(_ points: [CGPoint]) -> CGPoint? {
        guard !points.isEmpty else { return nil }
        let xs = points.map(\.x).sorted()
        let ys = points.map(\.y).sorted()
        let mid = xs.count / 2
        let medX = xs.count % 2 == 0 ? (xs[mid - 1] + xs[mid]) / 2 : xs[mid]
        let medY = ys.count % 2 == 0 ? (ys[mid - 1] + ys[mid]) / 2 : ys[mid]
        return CGPoint(x: medX, y: medY)
    }

    private func triggerPop() {
        guard !isPopping else { return }
        isPopping = true
        popShockwaveScale = 0.0

        #if canImport(UIKit)
        let rigidGen = UIImpactFeedbackGenerator(style: .rigid)
        rigidGen.impactOccurred(intensity: 1.0)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
            let notifGen = UINotificationFeedbackGenerator()
            notifGen.notificationOccurred(.success)
        }
        #endif

        withAnimation(.easeOut(duration: 0.35)) {
            popShockwaveScale = 1.0
        }

        // Finalize this target's raw estimate before resetting the buffer —
        // falls back to the target's own position (zero assumed error) if
        // tracking dropped out for the whole window, so a bad target never
        // poisons the fit with garbage or crashes it with an empty array.
        let finalizedRaw = medianPoint(collectedRawSamples) ?? targetPoints[currentTargetIndex]
        fittedRawPoints.append(finalizedRaw)

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.38) {
            isPopping = false
            dwellProgress = 0.0
            windowElapsedSec = 0.0
            trackingDropoutFrames = 0
            collectedRawSamples = []
            isLockedOnTarget = false
            popShockwaveScale = 0.0

            if currentTargetIndex + 1 < targetPoints.count {
                withAnimation(.spring(response: 0.45, dampingFraction: 0.75)) {
                    currentTargetIndex += 1
                    controller.calibrationTarget = targetPoints[currentTargetIndex]
                }
            } else {
                controller.calibrationTarget = nil
                if let profile = GazeCalibrationProfile.fit(rawPoints: fittedRawPoints, targetPoints: targetPoints) {
                    controller.applyAndPersist(calibrationProfile: profile)
                }
                stopCalibrationSession()
                onCalibrationComplete()
            }
        }
    }

    private func handleClose() {
        stopCalibrationSession()
        if let onClose {
            onClose()
        } else {
            dismiss()
        }
    }
}
