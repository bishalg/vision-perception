import SwiftUI
import CurvAsdGazeKit

// MARK: - Replay Visualization Mode

public enum ReplayVisualizationMode: String, CaseIterable, Identifiable {
    case scanpath = "FLOW"
    case heatmap = "HEATMAP"

    public var id: String { rawValue }
}

public enum ReplayTailMode: String, CaseIterable, Identifiable {
    case trailing5s = "5S TAIL"
    case fullPath = "FULL PATH"

    public var id: String { rawValue }
}

// MARK: - Full Screen Replay & Heatmap View

/// Dedicated full-screen clinical replay view featuring:
/// • Background stimulus (Social Actor vs Geometric Patterns)
/// • Flow-of-line gaze scanpath with optional 5-second trailing tail removal
/// • Fixation density heatmap overlay
/// • Comprehensive playback scrubber, speed controls, and frame-by-frame affect inspection
public struct GazeReplayHeatmapFullScreenView: View {
    @Environment(\.dismiss) private var dismiss

    public let summary: GazeSessionReportSummary
    public let recordedFrames: [GazeFrame]

    @State private var currentFrameIndex: Int = 0
    @State private var isPlaying: Bool = true
    @State private var playbackTimer: Timer?
    @State private var playbackSpeed: Double = 1.0
    @State private var visualizationMode: ReplayVisualizationMode = .scanpath
    @State private var tailMode: ReplayTailMode = .trailing5s
    /// Per-clip filter (stimulus id); nil replays the whole session.
    @State private var clipFilter: String?

    public init(
        summary: GazeSessionReportSummary,
        recordedFrames: [GazeFrame]
    ) {
        self.summary = summary
        self.recordedFrames = recordedFrames
    }

    /// Frame source honoring the per-clip filter; frames without a stimulus id group under the untagged clip.
    private var frames: [GazeFrame] {
        guard let filter = clipFilter else { return recordedFrames }
        return recordedFrames.filter { ($0.stimulusId ?? PerVideoMetricsExtractor.untaggedClipId) == filter }
    }

    /// Distinct clips present in the recording, in capture order; filter chips are hidden for a single clip.
    private var availableClips: [String] {
        var order: [String] = []
        for frame in recordedFrames {
            let id = frame.stimulusId ?? PerVideoMetricsExtractor.untaggedClipId
            if !order.contains(id) { order.append(id) }
        }
        return order
    }

    private func chipLabel(for clipId: String) -> String {
        guard clipId != PerVideoMetricsExtractor.untaggedClipId else { return "Untagged" }
        return clipId
            .replacingOccurrences(of: "_", with: " ")
            .replacingOccurrences(of: "stim", with: "Clip")
    }

    private var activeFrame: GazeFrame? {
        guard !frames.isEmpty else { return nil }
        let idx = max(0, min(currentFrameIndex, frames.count - 1))
        return frames[idx]
    }

    private var currentTimeSec: Double {
        activeFrame?.gaze.videoTimeSec ?? (Double(currentFrameIndex) * (1.0 / 60.0))
    }

    private var totalDurationSec: Double {
        max(summary.durationSec, 30.0)
    }

    public var body: some View {
        ZStack {
            // Full Screen Stimulus Background
            stimulusBackground
                .ignoresSafeArea()

            // Overlays: Scanpath or Heatmap
            GeometryReader { geo in
                ZStack {
                    if visualizationMode == .scanpath {
                        scanpathFlowOverlay(in: geo.size)
                    } else {
                        fixationHeatmapOverlay(in: geo.size)
                    }

                    // Active gaze reticle cursor
                    if let frame = activeFrame {
                        activeGazeCursor(frame: frame, in: geo.size)
                    }
                }
            }
            .ignoresSafeArea()

            // Top Header Bar & Controls
            VStack {
                topNavigationBar
                    .padding(.horizontal, 16)
                    .padding(.top, 10)

                Spacer()

                // Floating Playback Control Card
                floatingControlsPanel
                    .padding(.horizontal, 16)
                    .padding(.bottom, 24)
            }
        }
        .onAppear(perform: startPlayback)
        .onDisappear(perform: stopPlayback)
    }

    // MARK: - Top Navigation Bar (Standard iOS Design & Full-Width Controls)

    private var topNavigationBar: some View {
        VStack(spacing: 10) {
            // Top Row: Standard iOS Back Chevron "<" Button
            HStack {
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(.white)
                        .frame(width: 38, height: 38)
                        .background(Circle().fill(.ultraThinMaterial.opacity(0.85)))
                        .overlay(Circle().stroke(Color.white.opacity(0.2), lineWidth: 1))
                        .shadow(color: Color.black.opacity(0.2), radius: 4, y: 2)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Back")

                Spacer()

                // Subtle title badge
                Text("REPLAY")
                    .font(.system(size: 12, weight: .bold, design: .rounded))
                    .tracking(1.2)
                    .foregroundColor(.white.opacity(0.85))
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(Capsule().fill(.ultraThinMaterial.opacity(0.75)))
                    .overlay(Capsule().stroke(Color.white.opacity(0.15), lineWidth: 1))
            }

            // Row 2: Mode Segmented Controller (Full Width)
            Picker("Mode", selection: $visualizationMode) {
                ForEach(ReplayVisualizationMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: .infinity)
            .background(.ultraThinMaterial.opacity(0.75))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            // Row 3: Tail Mode Segmented Controller (Full Width)
            Picker("Tail Mode", selection: $tailMode) {
                ForEach(ReplayTailMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: .infinity)
            .background(.ultraThinMaterial.opacity(0.75))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            // Row 4: Per-clip filter chips (only when multiple clips were captured)
            if availableClips.count > 1 {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        filterChip(title: "All Clips", clipId: nil)
                        ForEach(availableClips, id: \.self) { clipId in
                            filterChip(title: chipLabel(for: clipId), clipId: clipId)
                        }
                    }
                    .padding(.horizontal, 2)
                    .padding(.vertical, 1)
                }
            }
        }
        .onChange(of: clipFilter) { _ in
            // Reset playback into the newly selected clip's frame range.
            currentFrameIndex = 0
        }
    }

    private func filterChip(title: String, clipId: String?) -> some View {
        let isSelected = clipFilter == clipId
        return Button {
            clipFilter = clipId
        } label: {
            Text(title)
                .font(.system(size: 10, weight: .bold, design: .rounded))
                .lineLimit(1)
                .foregroundColor(isSelected ? .black : .white.opacity(0.8))
                .padding(.horizontal, 9)
                .padding(.vertical, 5)
                .background(Capsule().fill(isSelected ? AnyShapeStyle(LiquidGlass.emeraldMint) : AnyShapeStyle(.ultraThinMaterial.opacity(0.75))))
                .overlay(Capsule().stroke(Color.white.opacity(0.15), lineWidth: 0.8))
        }
        .buttonStyle(.plain)
    }

    // MARK: - Stimulus Background (Social Actor vs Geometric)

    private var stimulusBackground: some View {
        HStack(spacing: 0) {
            // Left: Social Actor Canvas
            ZStack {
                LinearGradient(
                    colors: [LiquidGlass.stimulusSocialTop, LiquidGlass.stimulusSocialBottom],
                    startPoint: .top, endPoint: .bottom
                )

                VStack(spacing: 12) {
                    ZStack {
                        Circle()
                            .fill(RadialGradient(
                                colors: [Color.yellow, Color.orange],
                                center: .center, startRadius: 10, endRadius: 50
                            ))
                            .frame(width: 90, height: 90)

                        VStack(spacing: 8) {
                            HStack(spacing: 18) {
                                Circle().fill(Color.black).frame(width: 8, height: 12)
                                Circle().fill(Color.black).frame(width: 8, height: 12)
                            }
                            Capsule().fill(Color.red).frame(width: 24, height: 8)
                        }
                    }
                    Text("Social Stimulus (Actor)")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(Color.white.opacity(0.6))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            // Right: Geometric Patterns Canvas
            ZStack {
                LinearGradient(
                    colors: [LiquidGlass.stimulusGeometryTop, LiquidGlass.stimulusGeometryBottom],
                    startPoint: .top, endPoint: .bottom
                )

                VStack(spacing: 12) {
                    Image(systemName: "hexagon.fill")
                        .font(.system(size: 50))
                        .foregroundColor(LiquidGlass.celestialTeal.opacity(0.75))

                    Image(systemName: "star.fill")
                        .font(.system(size: 26))
                        .foregroundColor(LiquidGlass.warmAmber.opacity(0.7))

                    Text("Dynamic Geometric Pattern")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(Color.white.opacity(0.6))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    // MARK: - Scanpath Flow Overlay (With Optional 5-Second Tail Removal)

    @ViewBuilder
    private func scanpathFlowOverlay(in size: CGSize) -> some View {
        let maxIdx = min(currentFrameIndex, frames.count)
        let visibleFrames: [GazeFrame] = {
            let validSubarray = frames.prefix(maxIdx).filter { f in
                f.gaze.isValid && f.qualityVerdict != .rejected
            }
            if tailMode == .fullPath {
                return Array(validSubarray)
            } else {
                // 5-second trailing window: only points where time >= (currentTime - 5.0)
                let cutoffTime = max(0, currentTimeSec - 5.0)
                return validSubarray.filter { f in
                    f.gaze.videoTimeSec >= cutoffTime
                }
            }
        }()

        Canvas { context, canvasSize in
            guard visibleFrames.count > 1 else { return }

            for i in 1..<visibleFrames.count {
                let prev = visibleFrames[i - 1].gaze
                let curr = visibleFrames[i].gaze

                // Suppress spurious connector lines across blink gaps or loss of tracking
                let dt = curr.videoTimeSec - prev.videoTimeSec
                guard dt > 0 && dt <= 0.12 else { continue }

                var path = Path()
                path.move(to: CGPoint(x: prev.xNorm * canvasSize.width, y: prev.yNorm * canvasSize.height))
                path.addLine(to: CGPoint(x: curr.xNorm * canvasSize.width, y: curr.yNorm * canvasSize.height))

                // If in trailing tail mode, fade line opacity based on age relative to 5s window
                let opacity: Double = {
                    if tailMode == .fullPath {
                        return 0.75
                    } else {
                        let age = currentTimeSec - curr.videoTimeSec
                        return max(0.1, min(1.0, 1.0 - (age / 5.0)))
                    }
                }()

                let strokeColor = (curr.xNorm < 0.5 ? LiquidGlass.celestialTeal : LiquidGlass.warmAmber)
                    .opacity(opacity)

                context.stroke(
                    path,
                    with: .color(strokeColor),
                    lineWidth: tailMode == .trailing5s ? 3.0 : 2.0
                )
            }
        }
    }

    // MARK: - Fixation Heatmap Overlay

    @ViewBuilder
    private func fixationHeatmapOverlay(in size: CGSize) -> some View {
        let maxIdx = min(currentFrameIndex, frames.count)
        let validSampledFrames: [GazeFrame] = {
            let validSubarray = frames.prefix(maxIdx).filter {
                $0.gaze.isValid && $0.qualityVerdict != .rejected
            }
            if tailMode == .fullPath {
                return Array(validSubarray)
            } else {
                let cutoffTime = max(0, currentTimeSec - 5.0)
                return validSubarray.filter { $0.gaze.videoTimeSec >= cutoffTime }
            }
        }()

        ZStack {
            ForEach(Array(validSampledFrames.enumerated()), id: \.offset) { item in
                if item.offset % 3 == 0 { // Sample every 3rd frame for performant Gaussian density
                    let pt = item.element.gaze
                    Circle()
                        .fill(
                            RadialGradient(
                                colors: [
                                    (pt.xNorm < 0.5 ? LiquidGlass.celestialTeal : LiquidGlass.warmAmber).opacity(0.25),
                                    Color.clear
                                ],
                                center: .center,
                                startRadius: 2,
                                endRadius: 36
                            )
                        )
                        .frame(width: 72, height: 72)
                        .position(x: pt.xNorm * size.width, y: pt.yNorm * size.height)
                }
            }
        }
    }

    // MARK: - Active Gaze Cursor

    private func activeGazeCursor(frame: GazeFrame, in size: CGSize) -> some View {
        let isOcularLoss = !frame.gaze.isValid || frame.qualityVerdict == .rejected
        let pt = CGPoint(x: frame.gaze.xNorm * size.width, y: frame.gaze.yNorm * size.height)

        return ZStack {
            Circle()
                .stroke(isOcularLoss ? Color.gray.opacity(0.4) : LiquidGlass.emeraldMint, lineWidth: 2.5)
                .frame(width: 32, height: 32)
                .shadow(color: isOcularLoss ? .clear : LiquidGlass.emeraldMint.opacity(0.8), radius: 8)

            Circle()
                .fill(isOcularLoss ? Color.gray.opacity(0.5) : LiquidGlass.emeraldMint)
                .frame(width: 10, height: 10)
        }
        .position(pt)
        .opacity(isOcularLoss ? 0.35 : 1.0)
    }

    // MARK: - Floating Playback Controls Panel

    private var floatingControlsPanel: some View {
        VStack(spacing: 12) {
            // Emotion & Excitement Pill (Centered directly above timeline values)
            if let frame = activeFrame {
                HStack(spacing: 8) {
                    Text(frame.affect.dominantEmotion.emoji)
                        .font(.system(size: 15))

                    Text(frame.affect.dominantEmotion.rawValue.uppercased())
                        .font(.system(size: 11, weight: .bold, design: .rounded))
                        .foregroundColor(.white)

                    Text("•")
                        .foregroundColor(.white.opacity(0.4))
                        .font(.system(size: 10))

                    let smilePct = Int(frame.affect.socialSmileIntensity * 100)
                    Text("Smile \(smilePct)%")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundColor(LiquidGlass.emeraldMint)

                    Text("•")
                        .foregroundColor(.white.opacity(0.4))
                        .font(.system(size: 10))

                    let excitementPct = Int(frame.affect.expressivityIndex * 100)
                    Text("Excitement \(excitementPct)%")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 6)
                .background(Capsule().fill(.ultraThinMaterial.opacity(0.85)))
                .overlay(
                    Capsule().stroke(Color.white.opacity(0.18), lineWidth: 1)
                )
                .shadow(color: Color.black.opacity(0.2), radius: 6, y: 2)
            }

            // Timeline Scrubber Slider
            HStack(spacing: 12) {
                Text(String(format: "%02d:%02d", Int(currentTimeSec) / 60, Int(currentTimeSec) % 60))
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(.white)

                Slider(
                    value: Binding(
                        get: { Double(currentFrameIndex) },
                        set: { newIdx in
                            currentFrameIndex = Int(newIdx)
                        }
                    ),
                    in: 0...Double(max(1, frames.count - 1))
                )
                .tint(LiquidGlass.emeraldMint)

                Text(String(format: "%02d:%02d", Int(totalDurationSec) / 60, Int(totalDurationSec) % 60))
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(Color.white.opacity(0.65))
            }

            // Controls Row: Step Backward, Play/Pause, Step Forward, Speed Menu, Frame Info
            HStack(spacing: 14) {
                // Step Backward 1s (60 frames)
                Button {
                    currentFrameIndex = max(0, currentFrameIndex - 60)
                } label: {
                    Image(systemName: "gobackward.10")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(.white.opacity(0.85))
                        .frame(width: 32, height: 32)
                }
                .buttonStyle(.plain)

                // Play / Pause Button
                Button {
                    togglePlayback()
                } label: {
                    Image(systemName: isPlaying ? "pause.fill" : "play.fill")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(.black)
                        .frame(width: 42, height: 42)
                        .background(Circle().fill(LiquidGlass.emeraldMint))
                        .shadow(color: LiquidGlass.emeraldMint.opacity(0.4), radius: 6)
                }
                .buttonStyle(.plain)

                // Step Forward 1s (60 frames)
                Button {
                    currentFrameIndex = min(max(0, frames.count - 1), currentFrameIndex + 60)
                } label: {
                    Image(systemName: "goforward.10")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(.white.opacity(0.85))
                        .frame(width: 32, height: 32)
                }
                .buttonStyle(.plain)

                Spacer()

                // Speed Selector Menu
                Menu {
                    Button("0.5x Speed") { playbackSpeed = 0.5; updatePlaybackTimer() }
                    Button("1.0x Normal") { playbackSpeed = 1.0; updatePlaybackTimer() }
                    Button("2.0x Fast") { playbackSpeed = 2.0; updatePlaybackTimer() }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "speedometer")
                            .font(.system(size: 11))
                        Text(String(format: "%.1fx", playbackSpeed))
                            .font(.system(size: 12, weight: .bold, design: .monospaced))
                    }
                    .foregroundColor(.white)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(Capsule().fill(.ultraThinMaterial.opacity(0.7)))
                    .overlay(Capsule().stroke(Color.white.opacity(0.2), lineWidth: 1))
                }

                // Frame info badge
                Text(String(format: "%d / %d", min(currentFrameIndex + 1, frames.count), frames.count))
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundColor(.white.opacity(0.6))
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .background(.ultraThinMaterial.opacity(0.85))
        .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .stroke(LiquidGlass.cardBorder, lineWidth: 1)
        )
        .shadow(color: Color.black.opacity(0.35), radius: 12, y: 6)
    }

    // MARK: - Playback Loop

    private func startPlayback() {
        isPlaying = true
        updatePlaybackTimer()
    }

    private func stopPlayback() {
        playbackTimer?.invalidate()
        playbackTimer = nil
        isPlaying = false
    }

    private func togglePlayback() {
        if isPlaying {
            stopPlayback()
        } else {
            startPlayback()
        }
    }

    private func updatePlaybackTimer() {
        playbackTimer?.invalidate()
        guard isPlaying else { return }

        let interval = (1.0 / 60.0) / playbackSpeed
        playbackTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { _ in
            MainActor.assumeIsolated {
                guard !frames.isEmpty else { return }
                if currentFrameIndex + 1 < frames.count {
                    currentFrameIndex += 1
                } else {
                    currentFrameIndex = 0 // loop
                }
            }
        }
    }
}
