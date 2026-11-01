import SwiftUI
import AsdCore
import CurvAsdGazeKit
#if canImport(AudioToolbox)
import AudioToolbox
#endif
#if canImport(UIKit)
import UIKit
#endif

// WP-G.10: the child-facing target shared by calibration (CAL-8), validation (VAL-1) and drift
// checks (DRF-1). The view only draws; the Rust pipeline decides when a window is complete and
// what it measured (`GazeTargetRunner` in the kit).

/// The character drawn for each kind of target. Validation uses a different character from
/// calibration (VAL-1) so the child sees something new.
enum TargetCharacter: String, CaseIterable {
    case balloon
    case star
    case ladybug

    static func forKind(_ kind: GazeTargetKind) -> TargetCharacter {
        switch kind {
        case .calibration: return .balloon
        case .validation: return .star
        case .driftCheck: return .ladybug
        }
    }
}

/// Short sound and haptic cues: a chime when a target appears and a pop when it is caught
/// (CAL-8). System sounds follow the ring/silent switch.
enum TargetFeedback {
    /// System sound IDs: a soft "tink" for appear and a short "tock" for the pop.
    static let appearSoundID: UInt32 = 1103
    static let popSoundID: UInt32 = 1104

    @MainActor
    static func appear(enabled: Bool) {
        guard enabled else { return }
        #if canImport(AudioToolbox) && !targetEnvironment(simulator)
        AudioServicesPlaySystemSound(appearSoundID)
        #endif
    }

    @MainActor
    static func pop(enabled: Bool) {
        guard enabled else { return }
        #if canImport(AudioToolbox) && !targetEnvironment(simulator)
        AudioServicesPlaySystemSound(popSoundID)
        #endif
        #if canImport(UIKit)
        UIImpactFeedbackGenerator(style: .rigid).impactOccurred(intensity: 1.0)
        #endif
    }
}

/// Main-actor state of the target on screen, fed by a `GazeTargetRunner`.
@MainActor
final class GazeTargetStage: ObservableObject {
    @Published private(set) var current: GazeTargetPresentation?
    @Published private(set) var shownAt = Date()
    @Published private(set) var popping = false
    @Published private(set) var poppedAt = Date()
    /// Targets shown so far in this stage (for the progress pill).
    @Published private(set) var shownCount = 0
    let soundEnabled: Bool
    /// Hold after a window closes: the pop plays, then a short empty gap before the next target.
    let popDuration: Double
    let gapDuration: Double

    init(soundEnabled: Bool = true, popDuration: Double = 0.38, gapDuration: Double = 0.22) {
        self.soundEnabled = soundEnabled
        self.popDuration = popDuration
        self.gapDuration = gapDuration
    }

    /// A runner over `session` that shows its targets here and paces at the display rate.
    func runner<Session: GazeTargetSession>(for session: Session, schedule: GazeTargetSchedule) -> GazeTargetRunner<Session> {
        GazeTargetRunner(
            session: session,
            schedule: schedule,
            onTargetShown: { [weak self] presentation in
                await self?.show(presentation)
            },
            onTargetEnded: { [weak self] presentation, result in
                await self?.ended(presentation, result: result)
            },
            nextFrame: {
                try? await Task.sleep(nanoseconds: 16_666_667)
            }
        )
    }

    func show(_ presentation: GazeTargetPresentation) {
        current = presentation
        shownAt = Date()
        popping = false
        shownCount += 1
        TargetFeedback.appear(enabled: soundEnabled)
    }

    /// A hit pops with sound; a re-show or miss just fades away (no failure cue for the child).
    func ended(_ presentation: GazeTargetPresentation, result: GazeTargetResultRecord?) async {
        if result?.outcome == .hit {
            poppedAt = Date()
            popping = true
            TargetFeedback.pop(enabled: soundEnabled)
        }
        try? await Task.sleep(nanoseconds: UInt64(popDuration * 1_000_000_000))
        current = nil
        popping = false
        try? await Task.sleep(nanoseconds: UInt64(gapDuration * 1_000_000_000))
    }

    func clear() {
        current = nil
        popping = false
    }
}

/// Draws the stage's current target at its screen position. The layout must cover the whole
/// screen (`ignoresSafeArea`) so normalised target positions are screen positions.
struct GazeTargetStageLayer: View {
    @ObservedObject var stage: GazeTargetStage

    var body: some View {
        GeometryReader { geometry in
            if let presentation = stage.current {
                ShrinkingTargetView(
                    presentation: presentation,
                    shownAt: stage.shownAt,
                    popping: stage.popping,
                    poppedAt: stage.poppedAt
                )
                .position(
                    x: presentation.target.x * geometry.size.width,
                    y: presentation.target.y * geometry.size.height
                )
                .id("\(presentation.kind)-\(presentation.index)-\(presentation.attempt)-\(stage.shownCount)")
                .transition(.scale.combined(with: .opacity))
            }
        }
        .allowsHitTesting(false)
        .animation(.spring(response: 0.35, dampingFraction: 0.75), value: stage.current)
    }
}

/// CAL-8 target: an animated character that appears large and shrinks over the window to a
/// small bright centre (well under 1 degree at 45 cm), then pops.
struct ShrinkingTargetView: View {
    let presentation: GazeTargetPresentation
    let shownAt: Date
    let popping: Bool
    let poppedAt: Date

    /// Character size when it appears and at the end of the window (points).
    static let startSize: CGFloat = 150
    static let endSize: CGFloat = 26
    /// The bright centre the child ends up fixating.
    static let centreSize: CGFloat = 8

    static func size(progress: Double) -> CGFloat {
        let p = CGFloat(min(max(progress, 0), 1))
        // Ease out: most of the shrink early, so the small centre is on screen while samples count.
        let eased = 1 - (1 - p) * (1 - p)
        return startSize + (endSize - startSize) * eased
    }

    var body: some View {
        TimelineView(.animation) { context in
            let elapsed = context.date.timeIntervalSince(shownAt)
            let progress = presentation.windowSec > 0 ? elapsed / presentation.windowSec : 1
            let size = Self.size(progress: progress)
            let wobble = sin(elapsed * 6.0) * 8.0 * (1 - min(progress, 1))
            ZStack {
                if popping {
                    PopBurst(color: Self.color(for: presentation), start: poppedAt, now: context.date)
                } else {
                    character(size: size)
                        .rotationEffect(.degrees(wobble))
                    Circle()
                        .fill(Color.white)
                        .frame(width: Self.centreSize, height: Self.centreSize)
                        .shadow(color: .white.opacity(0.9), radius: 4)
                }
            }
            .frame(width: Self.startSize, height: Self.startSize)
        }
        .accessibilityElement()
        .accessibilityIdentifier("gaze_target_\(TargetCharacter.forKind(presentation.kind).rawValue)")
    }

    static func color(for presentation: GazeTargetPresentation) -> Color {
        switch TargetCharacter.forKind(presentation.kind) {
        case .balloon:
            let palette = [LiquidGlass.balloonSky, LiquidGlass.balloonRose, LiquidGlass.balloonLeaf, LiquidGlass.balloonApricot, LiquidGlass.balloonViolet]
            return palette[presentation.index % palette.count]
        case .star:
            return LiquidGlass.warmAmber
        case .ladybug:
            return LiquidGlass.balloonRoseDeep
        }
    }

    @ViewBuilder
    private func character(size: CGFloat) -> some View {
        let color = Self.color(for: presentation)
        switch TargetCharacter.forKind(presentation.kind) {
        case .balloon:
            ZStack {
                BalloonShape()
                    .fill(RadialGradient(
                        colors: [Color.white.opacity(0.45), color, color.opacity(0.85)],
                        center: UnitPoint(x: 0.35, y: 0.3),
                        startRadius: 2,
                        endRadius: size * 0.6
                    ))
                    .frame(width: size * 0.8, height: size)
                CharacterFace(size: size * 0.45)
                    .offset(y: -size * 0.12)
            }
        case .star:
            ZStack {
                Image(systemName: "star.fill")
                    .resizable()
                    .scaledToFit()
                    .foregroundStyle(color)
                    .frame(width: size, height: size)
                    .shadow(color: color.opacity(0.7), radius: size * 0.08)
                CharacterFace(size: size * 0.35)
            }
        case .ladybug:
            Image(systemName: "ladybug.fill")
                .resizable()
                .scaledToFit()
                .foregroundStyle(color, Color.black)
                .frame(width: size, height: size)
        }
    }
}

/// Two eyes and a smile, scaled with the character.
private struct CharacterFace: View {
    let size: CGFloat

    var body: some View {
        VStack(spacing: size * 0.12) {
            HStack(spacing: size * 0.3) {
                Circle().fill(Color.black.opacity(0.8)).frame(width: size * 0.16, height: size * 0.2)
                Circle().fill(Color.black.opacity(0.8)).frame(width: size * 0.16, height: size * 0.2)
            }
            Capsule()
                .fill(Color.black.opacity(0.7))
                .frame(width: size * 0.36, height: size * 0.08)
        }
    }
}

/// Confetti ring for the pop at the end of a caught target.
private struct PopBurst: View {
    let color: Color
    let start: Date
    let now: Date

    var body: some View {
        let t = CGFloat(min(max(now.timeIntervalSince(start) / 0.35, 0), 1))
        ZStack {
            Circle()
                .stroke(color.opacity(Double(1 - t)), lineWidth: 3)
                .frame(width: 30 + t * 120, height: 30 + t * 120)
            ForEach(0..<10, id: \.self) { i in
                let angle = Double(i) * .pi / 5
                Circle()
                    .fill(i.isMultiple(of: 2) ? color : LiquidGlass.warmAmber)
                    .frame(width: 7, height: 7)
                    .offset(x: cos(angle) * (20 + t * 60), y: sin(angle) * (20 + t * 60))
                    .opacity(Double(1 - t))
            }
        }
    }
}

/// Small pill under the targets: what the child should do and how far along the layout is.
struct TargetHintPill: View {
    let text: String
    let progress: String?

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "eyes")
                .foregroundColor(LiquidGlass.celestialTeal)
            Text(text)
                .font(.system(size: 14, weight: .semibold, design: .rounded))
                .foregroundColor(.white)
            if let progress {
                Text(progress)
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(LiquidGlass.emeraldMint)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(Capsule().fill(Color.white.opacity(0.12)))
    }
}
