import SwiftUI
import CurvCameraOverlay

// MARK: - Close-Up Mosaic Scan Mode Overlay
//
// All UI components specific to the `.closeUpMosaic` mode live here.
// This mode uses a crosshair/tile reticle and shows a 0.6s dwell progress ring
// before auto-capturing each tile. The bottom card shows which of the 4 tiles
// have been captured and what to do next.

// MARK: - Reticle (inside CameraOverlayHost reticle box)

/// Crosshair + tile-grid reticle for close-up mosaic mode.
/// Shows a dwell progress ring (orange) that fills during the 0.6s hold gate.
struct CloseUpReticleView: View {
    var dwellProgress: Float = 0.0

    var body: some View {
        GeometryReader { proxy in
            let center = CGPoint(x: proxy.size.width / 2, y: proxy.size.height / 2)
            let side = min(proxy.size.width, proxy.size.height) * 0.60

            ZStack {
                // Outer dashed scan boundary
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(
                        Color.orange.opacity(0.40),
                        style: StrokeStyle(lineWidth: 1.5, dash: [8, 5])
                    )
                    .frame(width: side * 1.1, height: side * 1.1)

                // Dwell progress ring
                if dwellProgress > 0.01 {
                    Circle()
                        .trim(from: 0.0, to: CGFloat(min(1.0, max(0.0, dwellProgress))))
                        .stroke(
                            Color.orange,
                            style: StrokeStyle(lineWidth: 4.0, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .frame(width: side * 0.85, height: side * 0.85)
                        .animation(.linear(duration: 0.08), value: dwellProgress)
                }

                // Scan area box
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(
                        dwellProgress > 0 ? Color.orange : Color.white.opacity(0.75),
                        lineWidth: dwellProgress > 0 ? 2.5 : 1.5
                    )
                    .frame(width: side, height: side)
                    .shadow(
                        color: dwellProgress > 0 ? Color.orange.opacity(0.7) : Color.clear,
                        radius: 10
                    )

                // Crosshair lines
                CrosshairLines(side: side)

                // Magnifier icon
                Image(systemName: "magnifyingglass")
                    .font(.system(size: side * 0.20, weight: .ultraLight))
                    .foregroundColor(
                        dwellProgress > 0.8 ? Color.orange : Color.white.opacity(0.22)
                    )
            }
            .position(center)
        }
    }
}

private struct CrosshairLines: View {
    let side: CGFloat

    var body: some View {
        ZStack {
            // Horizontal crosshair
            Rectangle()
                .fill(Color.white.opacity(0.4))
                .frame(width: side * 0.5, height: 1)
            // Vertical crosshair
            Rectangle()
                .fill(Color.white.opacity(0.4))
                .frame(width: 1, height: side * 0.5)

            // Corner L-brackets
            ForEach([(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)].indices, id: \.self) { i in
                let corners = [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
                CornerBracket(xSign: corners[i].0, ySign: corners[i].1, boxSide: side)
            }
        }
    }
}

private struct CornerBracket: View {
    let xSign: Double
    let ySign: Double
    let boxSide: CGFloat

    var body: some View {
        let half = boxSide / 2
        let armLen: CGFloat = 16
        let thick: CGFloat = 2

        ZStack {
            Rectangle()
                .fill(Color.white.opacity(0.85))
                .frame(width: armLen, height: thick)
                .offset(x: CGFloat(xSign) * (half - armLen / 2), y: CGFloat(ySign) * half)
            Rectangle()
                .fill(Color.white.opacity(0.85))
                .frame(width: thick, height: armLen)
                .offset(x: CGFloat(xSign) * half, y: CGFloat(ySign) * (half - armLen / 2))
        }
    }
}

// MARK: - Guidance Coaching Pill (ZStack sibling over camera)

/// HUD coaching pill for close-up mode: shows current tile name + dwell progress.
struct CloseUpGuidanceOverlayView: View {
    let currentTile: CloseUpTile?
    let statusMessage: String
    var dwellProgress: Float = 0.0

    var body: some View {
        VStack {
            if let tile = currentTile {
                VStack(spacing: 5) {
                    HStack(spacing: 7) {
                        Image(systemName: "magnifyingglass.circle.fill")
                            .foregroundColor(.orange)
                        Text(tile.rawValue.capitalized.replacingOccurrences(of: "_", with: " "))
                            .font(.system(size: 15, weight: .semibold, design: .rounded))
                            .foregroundColor(.white)
                    }

                    if dwellProgress > 0.05 {
                        HStack(spacing: 6) {
                            Text("HOLD STEADY")
                                .font(.system(size: 11, weight: .bold, design: .rounded))
                                .foregroundColor(.orange)
                            Text("\(Int(dwellProgress * 100))%")
                                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                .foregroundColor(.white)
                        }
                    } else {
                        Text(statusMessage)
                            .font(.system(size: 12, weight: .medium, design: .rounded))
                            .foregroundColor(.white.opacity(0.75))
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(
                    Capsule()
                        .fill(Color.black.opacity(0.75))
                        .overlay(
                            Capsule().stroke(
                                dwellProgress > 0 ? Color.orange : Color.orange.opacity(0.5),
                                lineWidth: dwellProgress > 0 ? 1.5 : 1.0
                            )
                        )
                )
                .padding(.top, 100)
            }
            Spacer()
        }
        .allowsHitTesting(false)
    }
}

// MARK: - Bottom Card

/// Bottom card for close-up mosaic mode.
/// Shows tile progress grid, current instruction, and a restart button.
struct CloseUpScanBottomCard: View {
    let session: CloseUpMosaicSession
    let statusMessage: String
    var dwellProgress: Float = 0.0
    let onRestart: () -> Void
    let onViewReading: () -> Void

    private var totalTiles: Int { CloseUpTile.allCases.count }
    private var capturedCount: Int { session.filledTiles.count }
    private var isComplete: Bool { capturedCount >= totalTiles }
    private var nextTile: CloseUpTile? {
        CloseUpTile.allCases.first { !session.filledTiles.contains($0) }
    }

    var body: some View {
        VStack(spacing: 10) {
            // Tile progress row
            HStack(spacing: 6) {
                ForEach(CloseUpTile.allCases, id: \.self) { tile in
                    TileStatusChip(
                        tile: tile,
                        isCaptured: session.filledTiles.contains(tile),
                        isActive: tile == nextTile
                    )
                }
            }

            // Progress line
            Text(isComplete
                 ? "All 4 regions captured — full crease detail ready"
                 : statusMessage
            )
            .font(.system(size: 13, weight: .semibold, design: .rounded))
            .foregroundColor(isComplete ? OverlayPalette.emerald : .white)
            .multilineTextAlignment(.center)
            .contentTransition(.opacity)

            // Action buttons
            HStack(spacing: 10) {
                Button(action: onRestart) {
                    Text("Restart")
                        .font(.system(size: 12, weight: .semibold, design: .rounded))
                        .foregroundColor(.white.opacity(0.75))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Capsule().stroke(Color.white.opacity(0.3), lineWidth: 1))
                }
                .buttonStyle(.plain)

                if isComplete {
                    Button(action: onViewReading) {
                        HStack(spacing: 5) {
                            Image(systemName: "sparkles")
                            Text("View Reading")
                        }
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundColor(.black)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(OverlayPalette.emerald))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.top, 2)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.black.opacity(0.75))
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(Color.orange.opacity(0.3), lineWidth: 0.8)
                )
        )
        .shadow(color: .black.opacity(0.4), radius: 12, y: 4)
    }
}

// MARK: - Tile Status Chip

private struct TileStatusChip: View {
    let tile: CloseUpTile
    let isCaptured: Bool
    let isActive: Bool

    private var shortName: String {
        switch tile {
        case .thenar:  return "Venus"
        case .distal:  return "Heart"
        case .midPalm: return "Head"
        case .ulnar:   return "Luna"
        }
    }

    var body: some View {
        VStack(spacing: 3) {
            if isCaptured {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 14))
                    .foregroundColor(OverlayPalette.emerald)
            } else if isActive {
                Circle().fill(Color.orange).frame(width: 6, height: 6)
            } else {
                Circle().fill(Color.white.opacity(0.3)).frame(width: 6, height: 6)
            }

            Text(shortName)
                .font(.system(size: 10, weight: isActive ? .bold : .medium, design: .rounded))
                .foregroundColor(
                    isCaptured ? OverlayPalette.emerald :
                        isActive ? .orange : .white.opacity(0.5)
                )
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 5)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(
                    isActive ? Color.orange.opacity(0.18) :
                        isCaptured ? OverlayPalette.emerald.opacity(0.15) :
                        Color.white.opacity(0.06)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(
                            isActive ? Color.orange.opacity(0.7) :
                                isCaptured ? OverlayPalette.emerald.opacity(0.4) :
                                Color.clear,
                            lineWidth: 1
                        )
                )
        )
    }
}

// MARK: - Dwell Gate (pure value type — no UIKit, no Combine, fully testable)

/// Pure-value dwell gate for the close-up mosaic auto-capture.
/// The caller passes `evalReady` + `now` every camera frame; the gate
/// accumulates elapsed hold time and fires once `requiredDuration` is met.
/// Resets immediately if `evalReady` drops to false.
public struct CloseUpDwellGate {
    public let requiredDuration: CFTimeInterval
    private var dwellStart: CFTimeInterval?
    private var hasFired: Bool = false

    public init(requiredDuration: CFTimeInterval = 0.6) {
        self.requiredDuration = requiredDuration
    }

    /// Returns current progress 0.0–1.0. Call every frame.
    @discardableResult
    public mutating func update(evalReady: Bool, now: CFTimeInterval) -> Float {
        guard evalReady else {
            dwellStart = nil
            return 0.0
        }
        if dwellStart == nil { dwellStart = now }
        let elapsed = now - dwellStart!
        return Float(min(1.0, elapsed / requiredDuration))
    }

    /// Returns true exactly once when the dwell threshold is met.
    /// Caller must call `reset()` after consuming the fire signal.
    public mutating func shouldFire(evalReady: Bool, now: CFTimeInterval) -> Bool {
        let progress = update(evalReady: evalReady, now: now)
        guard progress >= 1.0, !hasFired else { return false }
        hasFired = true
        return true
    }

    /// Reset after a capture (or mode switch). Re-arms for the next tile.
    public mutating func reset() {
        dwellStart = nil
        hasFired = false
    }
}
