import SwiftUI

/**
 * Real-time HUD for Mode B 3D Hand Mesh & Orbit Scan.
 * Provides:
 * - Overall progress percentage & progress bar
 * - 6 discrete angle pips (Flat, Tilt Left, Tilt Right, Thumb In, Thumb Out, Fingers Up)
 * - Animated motion direction arrows
 * - Real-time good/bad angle feedback (Hold steady vs Adjusting vs Hand moved)
 * - Educational coaching hints explaining what each angle captures
 */
public struct OrbitGuidanceOverlayView: View {
    public let session: OrbitCaptureSession
    public let state: OrbitGuidanceState

    @State private var arrowPulse = false

    public init(session: OrbitCaptureSession, state: OrbitGuidanceState) {
        self.session = session
        self.state = state
    }

    private var activeBin: OrbitBin? {
        switch state {
        case .coach(let bin), .holdSteady(let bin, _), .capturing(let bin):
            return bin
        default:
            return OrbitBin.allCases.first(where: { !session.filledBins.contains($0) })
        }
    }

    private var holdProgress: Float? {
        if case .holdSteady(_, let p) = state { return p }
        return nil
    }

    private var isGoodAngle: Bool {
        if case .holdSteady = state { return true }
        if case .capturing = state { return true }
        return false
    }

    /// Center arrow/badge only make sense once the hand is trackable in a
    /// target angle — during setup states the bottom card owns the message.
    private var showsDirectionUI: Bool {
        switch state {
        case .coach, .holdSteady, .capturing, .rejected:
            return true
        default:
            return false
        }
    }

    private var isRejected: Bool {
        if case .rejected = state { return true }
        return false
    }

    public var body: some View {
        VStack(spacing: 12) {
            // NOTE: the progress header (%, bar, angle pips) now lives in the
            // bottom scan card — top of the screen stays clear. Center keeps
            // only the direction arrow + live status badge.
            Spacer()

            // ── Center Motion Direction & Coaching Feedback ──
            if showsDirectionUI, let bin = activeBin, session.coverageFraction < 1.0 {
                VStack(spacing: 8) {
                    // Animated Direction Indicator with circular hold ring
                    ZStack {
                        Circle()
                            .fill(Color.black.opacity(0.72))
                            .frame(width: 72, height: 72)
                            .overlay(
                                Circle()
                                    .stroke(
                                        isGoodAngle ? OverlayPalette.emerald : (isRejected ? Color.red : Color.white.opacity(0.4)),
                                        lineWidth: isGoodAngle ? 3.0 : 1.5
                                    )
                                    .scaleEffect(arrowPulse && !isGoodAngle ? 1.08 : 1.0)
                            )
                            .shadow(
                                color: isGoodAngle ? OverlayPalette.emerald.opacity(0.6) : .black.opacity(0.4),
                                radius: isGoodAngle ? 14 : 6
                            )

                        if let p = holdProgress, p > 0.02 {
                            Circle()
                                .trim(from: 0.0, to: CGFloat(p))
                                .stroke(OverlayPalette.emerald, style: StrokeStyle(lineWidth: 3.5, lineCap: .round))
                                .rotationEffect(.degrees(-90))
                                .frame(width: 80, height: 80)
                                .animation(.linear(duration: 0.08), value: p)
                        }

                        Text(bin.arrowSymbol)
                            .font(.system(size: 36, weight: .bold))
                            .foregroundColor(isGoodAngle ? OverlayPalette.emerald : .white)
                            .offset(x: horizontalOffset(for: bin), y: verticalOffset(for: bin))
                    }

                    // Real-time Status Badge (only active during hold / capture / rejection)
                    statusBadge(for: bin)
                }
                .transition(.scale.combined(with: .opacity))
            } else if session.coverageFraction >= 1.0 {
                VStack(spacing: 6) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 44))
                        .foregroundColor(OverlayPalette.emerald)
                        .shadow(color: OverlayPalette.emerald.opacity(0.7), radius: 12)
                    Text("3D SCAN COMPLETE")
                        .font(.system(size: 14, weight: .bold, design: .rounded))
                        .foregroundColor(.white)
                    Text("Metric parallax relief map computed successfully")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.8))
                }
                .padding(16)
                .background(
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .fill(Color.black.opacity(0.75))
                        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(OverlayPalette.emerald.opacity(0.6), lineWidth: 1.5))
                )
            }

            Spacer()
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
                arrowPulse = true
            }
        }
    }

    // ── Real-time Status Badge ──
    @ViewBuilder
    private func statusBadge(for bin: OrbitBin) -> some View {
        switch state {
        case .holdSteady(_, let p):
            HStack(spacing: 6) {
                Circle()
                    .fill(OverlayPalette.emerald)
                    .frame(width: 8, height: 8)
                    .shadow(color: OverlayPalette.emerald.opacity(0.9), radius: 4)
                Text("PERFECT ANGLE — HOLD STEADY (\(Int(p * 100))%)")
                    .font(.system(size: 12, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 7)
            .background(
                Capsule()
                    .fill(OverlayPalette.emeraldDeep.opacity(0.75))
                    .overlay(Capsule().stroke(OverlayPalette.emerald, lineWidth: 1.5))
            )

        case .rejected(let reason):
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.yellow)
                    .font(.system(size: 12))
                Text(reason.uppercased())
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(Color.red.opacity(0.7))
                    .overlay(Capsule().stroke(Color.red.opacity(0.9), lineWidth: 1))
            )

        case .capturing:
            HStack(spacing: 6) {
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                    .scaleEffect(0.7)
                Text("RECORDING 3D KEYFRAME...")
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(Color.black.opacity(0.75))
                    .overlay(Capsule().stroke(OverlayPalette.emerald, lineWidth: 1))
            )

        default:
            EmptyView()
        }
    }

    private func horizontalOffset(for bin: OrbitBin) -> CGFloat {
        guard !isGoodAngle else { return 0 }
        switch bin {
        case .tiltLeft: return arrowPulse ? -6 : 0
        case .tiltRight: return arrowPulse ? 6 : 0
        default: return 0
        }
    }

    private func verticalOffset(for bin: OrbitBin) -> CGFloat {
        guard !isGoodAngle else { return 0 }
        switch bin {
        case .tiltForward: return arrowPulse ? -6 : 0
        default: return 0
        }
    }
}

/**
 * Progress header for the 3D orbit scan: title, overall %, progress track,
 * and the 6 discrete angle pips. Rendered inside the bottom scan card so
 * the top of the screen stays clear.
 */
public struct OrbitProgressHeaderView: View {
    public let session: OrbitCaptureSession
    public let state: OrbitGuidanceState

    public init(session: OrbitCaptureSession, state: OrbitGuidanceState) {
        self.session = session
        self.state = state
    }

    private var activeBin: OrbitBin? {
        switch state {
        case .coach(let bin), .holdSteady(let bin, _), .capturing(let bin):
            return bin
        default:
            return OrbitBin.allCases.first(where: { !session.filledBins.contains($0) })
        }
    }

    private var isGoodAngle: Bool {
        if case .holdSteady = state { return true }
        if case .capturing = state { return true }
        return false
    }

    private var progressPercent: Int {
        Int(round(session.displayProgress(for: state) * 100))
    }

    public var body: some View {
        VStack(spacing: 8) {
            HStack {
                Image(systemName: "rotate.3d")
                    .foregroundColor(OverlayPalette.emerald)
                Text("3D HAND MESH SCAN")
                    .font(.system(size: 11, weight: .heavy, design: .rounded))
                    .foregroundColor(.white.opacity(0.85))
                Spacer()
                Text("\(progressPercent)% • \(session.filledBins.count)/\(OrbitBin.allCases.count)")
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(session.coverageFraction >= 1.0 ? OverlayPalette.emerald : .white)
            }

            // Progress track
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(Color.white.opacity(0.18))
                        .frame(height: 4)

                    Capsule()
                        .fill(
                            LinearGradient(
                                colors: [OverlayPalette.emerald, Color.cyan],
                                startPoint: .leading,
                                endPoint: .trailing
                            )
                        )
                        .frame(width: max(0, geo.size.width * CGFloat(session.displayProgress(for: state))), height: 4)
                        .animation(.spring(response: 0.35), value: session.coverageFraction)
                }
            }
            .frame(height: 4)

            // 6 Angle Pips
            HStack(spacing: 6) {
                ForEach(OrbitBin.allCases, id: \.rawValue) { bin in
                    pipView(for: bin)
                }
            }
            .padding(.top, 4)
        }
    }

    @ViewBuilder
    private func pipView(for bin: OrbitBin) -> some View {
        let isDone = session.filledBins.contains(bin)
        let isActive = (bin == activeBin && !isDone)

        HStack(spacing: 3) {
            if isDone {
                Image(systemName: "checkmark")
                    .font(.system(size: 8, weight: .heavy))
                    .foregroundColor(.white)
            } else {
                Text(bin.arrowSymbol)
                    .font(.system(size: 9, weight: .bold))
                    .foregroundColor(isActive ? (isGoodAngle ? OverlayPalette.emerald : .white) : .white.opacity(0.5))
            }
            Text(bin.shortLabel)
                .font(.system(size: 9, weight: isActive ? .bold : .medium, design: .rounded))
                .foregroundColor(isDone ? .white : (isActive ? .white : .white.opacity(0.55)))
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 4)
        .background(
            Capsule()
                .fill(
                    isDone
                        ? OverlayPalette.emeraldDeep.opacity(0.85)
                        : (isActive ? Color.white.opacity(0.22) : Color.white.opacity(0.08))
                )
                .overlay(
                    Capsule()
                        .stroke(
                            isDone
                                ? OverlayPalette.emerald
                                : (isActive ? (isGoodAngle ? OverlayPalette.emerald : Color.white.opacity(0.6)) : Color.clear),
                            lineWidth: isActive ? 1.4 : 0.8
                        )
                )
        )
    }
}
