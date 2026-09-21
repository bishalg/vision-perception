import SwiftUI
import CurvCameraOverlay
import CurvPalmOverlay

// MARK: - 3D Orbit Scan Mode Overlay
//
// All UI components specific to the `.orbitScan` mode live here.
// This mode shows NO PalmSilhouetteView — the reticle content is empty (Color.clear)
// and all coaching comes from OrbitGuidanceOverlayView + OrbitDirectionChip + OrbitScanBottomCard.

// MARK: - Bottom Card

/**
 * Bottom card shown instead of the still shutter during 3D Orbit mode.
 * Top HUD keeps progress + angle pips; the center keeps the direction
 * arrow; this card owns the single current instruction in plain words.
 */
// MARK: - Bottom Card

/**
 * Bottom card shown during 3D Orbit mode.
 * Contains:
 * - Progress header with angle pips
 * - Clear, friendly coaching title + subtitle
 * - Manual "Capture Angle" button (dual auto + manual control)
 * - "Finish 3D Scan" button once >= 3 angles are captured
 * - "Skip angle" and "Restart" options
 */
struct OrbitScanBottomCard: View {
    let session: OrbitCaptureSession
    let state: OrbitGuidanceState
    var isCapturing: Bool = false
    var canCompleteEarly: Bool = false
    let onCapture: () -> Void
    let onSkip: () -> Void
    let onFinishEarly: () -> Void
    let onRestart: () -> Void
    var onView3D: (() -> Void)? = nil

    private var stepNumber: Int { min(session.filledBins.count + 1, OrbitBin.allCases.count) }

    private var activeBin: OrbitBin? {
        switch state {
        case .coach(let bin), .holdSteady(let bin, _), .capturing(let bin):
            return bin
        default:
            return OrbitBin.allCases.first(where: { !session.filledBins.contains($0) })
        }
    }

    private var instruction: String {
        switch state {
        case .idle, .searching:               return "Show your open palm"
        case .lockPose:                       return "Center your palm in frame"
        case .fitHand:                        return "Show your whole hand"
        case .moveCloser:                     return "Move your hand closer"
        case .addLight:                       return "Add more light"
        case .coach(let bin),
             .holdSteady(let bin, _),
             .capturing(let bin):             return bin.displayName
        case .rejected(let reason):           return reason
        case .complete:                       return "3D Scan complete"
        }
    }

    private var subInstruction: String {
        switch state {
        case .idle:                           return "Get ready to capture angles…"
        case .searching:                      return "Position your hand in the viewfinder"
        case .lockPose:                       return "Keep hand in the center region"
        case .fitHand:                        return "Keep every fingertip inside the screen"
        case .moveCloser:                     return "Hold hand a little nearer to camera"
        case .addLight:                       return "Brighter lighting helps 3D accuracy"
        case .coach(let bin):
            switch bin {
            case .flatCenter:                 return "Auto-capturing… or tap Capture Angle below"
            case .tiltLeft:                   return "Slowly tilt hand left, or tilt phone left"
            case .tiltRight:                  return "Slowly tilt hand right, or tilt phone right"
            case .thumbToward:                return "Turn thumb slightly toward camera"
            case .thumbAway:                  return "Turn thumb slightly away from camera"
            case .tiltForward:                return "Tip fingertips slightly toward camera"
            }
        case .holdSteady(_, let p):           return "Hold steady… \(Int(p * 100))%"
        case .capturing:                      return "Saving 3D angle…"
        case .rejected:                       return "Hold hand still or tap Capture below"
        case .complete:                       return "Parallax depth map computed"
        }
    }

    private var statusColor: Color {
        switch state {
        case .holdSteady, .capturing, .complete: return OverlayPalette.emerald
        case .rejected:                          return .yellow
        default:                                 return .white.opacity(0.85)
        }
    }

    var body: some View {
        VStack(spacing: 8) {
            OrbitProgressHeaderView(session: session, state: state)

            VStack(spacing: 2) {
                Text("Step \(stepNumber) of \(OrbitBin.allCases.count): \(instruction)")
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)
                    .contentTransition(.opacity)
                    .animation(.easeInOut(duration: 0.2), value: instruction)

                Text(subInstruction)
                    .font(.system(size: 12, weight: .medium, design: .rounded))
                    .foregroundColor(statusColor)
                    .multilineTextAlignment(.center)
            }

            if case .complete = state {
                OrbitCompleteActions(onRestart: onRestart, onView3D: onView3D)
            } else {
                // Primary action bar: Manual Capture + Optional Early Finish
                HStack(spacing: 10) {
                    Button(action: onCapture) {
                        HStack(spacing: 6) {
                            if isCapturing {
                                ProgressView()
                                    .progressViewStyle(CircularProgressViewStyle(tint: .black))
                                    .scaleEffect(0.75)
                            } else {
                                Image(systemName: "camera.circle.fill")
                                    .font(.system(size: 15, weight: .bold))
                            }
                            Text("Capture Angle")
                                .font(.system(size: 13, weight: .bold, design: .rounded))
                        }
                        .foregroundColor(.black)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(OverlayPalette.emerald))
                        .shadow(color: OverlayPalette.emerald.opacity(0.4), radius: 6, y: 2)
                    }
                    .buttonStyle(.plain)
                    .disabled(isCapturing)

                    if canCompleteEarly || session.filledBins.count >= 3 {
                        Button(action: onFinishEarly) {
                            HStack(spacing: 5) {
                                Image(systemName: "checkmark.seal.fill")
                                Text("Finish (\(session.filledBins.count)/\(OrbitBin.allCases.count))")
                            }
                            .font(.system(size: 13, weight: .bold, design: .rounded))
                            .foregroundColor(.white)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 8)
                            .background(Capsule().fill(Color.cyan.opacity(0.40)))
                            .overlay(Capsule().stroke(Color.cyan, lineWidth: 1.2))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.top, 4)

                // Secondary actions: Skip angle / Restart
                HStack(spacing: 16) {
                    Button(action: onSkip) {
                        HStack(spacing: 3) {
                            Image(systemName: "forward.fill")
                                .font(.system(size: 9))
                            Text("Skip angle")
                                .font(.system(size: 11, weight: .medium, design: .rounded))
                        }
                        .foregroundColor(.white.opacity(0.70))
                    }
                    .buttonStyle(.plain)

                    Text("•")
                        .foregroundColor(.white.opacity(0.35))
                        .font(.system(size: 10))

                    Button(action: onRestart) {
                        Text("Restart scan")
                            .font(.system(size: 11, weight: .medium, design: .rounded))
                            .foregroundColor(.white.opacity(0.70))
                    }
                    .buttonStyle(.plain)
                }
                .padding(.top, 2)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.black.opacity(0.76))
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(Color.white.opacity(0.2), lineWidth: 0.8)
                )
        )
        .shadow(color: .black.opacity(0.4), radius: 12, y: 4)
    }
}

// MARK: - Complete Actions (separated to avoid type-checker timeout)

private struct OrbitCompleteActions: View {
    let onRestart: () -> Void
    let onView3D: (() -> Void)?

    var body: some View {
        HStack(spacing: 12) {
            Button(action: onRestart) {
                Text("Restart scan")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .foregroundColor(.white.opacity(0.75))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(
                        Capsule().stroke(Color.white.opacity(0.3), lineWidth: 1)
                    )
            }
            .buttonStyle(.plain)

            if let onView3D {
                Button(action: onView3D) {
                    HStack(spacing: 6) {
                        Image(systemName: "cube.transparent.fill")
                        Text("View 3D Hand Model")
                    }
                    .font(.system(size: 13, weight: .bold, design: .rounded))
                    .foregroundColor(.black)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(Capsule().fill(OverlayPalette.emerald))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.top, 4)
    }
}

// MARK: - Direction Chip

/**
 * Compact "which way do I move" chip pinned below the mode capsule during
 * orbit mode. The big center arrow is hidden behind the user's own hand,
 * so this is the glanceable direction cue.
 */
struct OrbitDirectionChip: View {
    let session: OrbitCaptureSession
    let state: OrbitGuidanceState

    @State private var nudge = false

    private var activeBin: OrbitBin? {
        switch state {
        case .coach(let bin), .holdSteady(let bin, _), .capturing(let bin):
            return bin
        default:
            return OrbitBin.allCases.first(where: { !session.filledBins.contains($0) })
        }
    }

    private var isComplete: Bool {
        if case .complete = state { return true }
        return session.filledBins.count >= OrbitBin.allCases.count
    }

    private var nudgeOffset: CGSize {
        switch activeBin {
        case .tiltLeft:    return CGSize(width: -9, height: 0)
        case .tiltRight:   return CGSize(width: 9, height: 0)
        case .tiltForward: return CGSize(width: 0, height: -9)
        case .thumbToward: return CGSize(width: 7, height: 5)
        case .thumbAway:   return CGSize(width: -7, height: 5)
        default:           return .zero
        }
    }

    private var setupHint: String? {
        switch state {
        case .idle, .searching: return "Show palm"
        case .lockPose:         return "Center palm"
        case .fitHand:          return "Whole hand"
        case .moveCloser:       return "Come closer"
        case .addLight:         return "More light"
        default:                return nil
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            if isComplete {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 20, weight: .bold))
                    .foregroundColor(OverlayPalette.emerald)
                Text("Complete")
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            } else if let hint = setupHint {
                Image(systemName: "hand.raised.fill")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundColor(.yellow)
                Text(hint)
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            } else {
                Text(activeBin?.arrowSymbol ?? "✋")
                    .font(.system(size: 24, weight: .bold))
                    .foregroundColor(.cyan)
                    .offset(nudge ? nudgeOffset : .zero)
                    .animation(.easeInOut(duration: 0.7).repeatForever(autoreverses: true), value: nudge)
                    .onAppear { nudge = true }
                Text(activeBin?.shortLabel ?? "Get ready")
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundColor(.white)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(
            Capsule()
                .fill(Color.black.opacity(0.68))
                .overlay(Capsule().stroke(Color.cyan.opacity(0.5), lineWidth: 1))
        )
        .shadow(color: .black.opacity(0.4), radius: 8, y: 3)
        .allowsHitTesting(false)
    }
}
