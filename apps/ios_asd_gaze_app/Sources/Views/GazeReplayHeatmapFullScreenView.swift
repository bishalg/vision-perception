import SwiftUI
import CurvAsdGazeKit

// MARK: - Replay Visualization Mode

public enum ReplayVisualizationMode: String, CaseIterable, Identifiable {
    case scanpath = "Scanpath Flow"
    case heatmap = "Fixation Heatmap"

    public var id: String { rawValue }
}

public enum ReplayTailMode: String, CaseIterable, Identifiable {
    case trailing5s = "Last 5s Tail"
    case fullPath = "Full Path"

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

    public init(
        summary: GazeSessionReportSummary,
        recordedFrames: [GazeFrame]
    ) {
        self.summary = summary
        self.recordedFrames = recordedFrames
    }

    private var activeFrame: GazeFrame? {
        guard !recordedFrames.isEmpty else { return nil }
        let idx = max(0, min(currentFrameIndex, recordedFrames.count - 1))
        return recordedFrames[idx]
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

            // Top Header Bar
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

    // MARK: - Top Navigation Bar

    private var topNavigationBar: some View {
        HStack {
            Button {
                dismiss()
            } label: {
                HStack(spacing: 5) {
                    Image(systemName: "chevron.left")
                        .font(.system(size: 14, weight: .bold))
                    Text("Report")
                        .font(.system(size: 14, weight: .semibold, design: .rounded))
                }
                .foregroundColor(.white)
                .padding(.horizontal, 12)
                .padding(.vertical, 7)
                .background(Capsule().fill(.ultraThinMaterial.opacity(0.8)))
            }
            .buttonStyle(.plain)

            Spacer()

            // Mode Selector: Scanpath vs Heatmap
            Picker("Mode", selection: $visualizationMode) {
                ForEach(ReplayVisualizationMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 220)
            .background(.ultraThinMaterial.opacity(0.7))
            .clipShape(RoundedRectangle(cornerRadius: 8))

            Spacer()

            // Active Emotion Tag at this frame
            if let frame = activeFrame {
                HStack(spacing: 4) {
                    Text(frame.affect.dominantEmotion.emoji)
                        .font(.system(size: 14))
                    Text(frame.affect.dominantEmotion.rawValue)
                        .font(.system(size: 11, weight: .semibold, design: .rounded))
                        .foregroundColor(.white)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Capsule().fill(.ultraThinMaterial.opacity(0.8)))
            }
        }
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
        let maxIdx = min(currentFrameIndex, recordedFrames.count)
        let visibleFrames: [GazeFrame] = {
            let validSubarray = recordedFrames.prefix(maxIdx).filter { f in
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
        let maxIdx = min(currentFrameIndex, recordedFrames.count)
        let validSampledFrames = Array(recordedFrames.prefix(maxIdx).filter {
            $0.gaze.isValid && $0.qualityVerdict != .rejected
        })

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
                    in: 0...Double(max(1, recordedFrames.count - 1))
                )
                .tint(LiquidGlass.emeraldMint)

                Text(String(format: "%02d:%02d", Int(totalDurationSec) / 60, Int(totalDurationSec) % 60))
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(Color.white.opacity(0.65))
            }

            // Controls Row: Play/Pause, Speed, and Tail Toggle
            HStack(spacing: 14) {
                // Play / Pause Button
                Button {
                    togglePlayback()
                } label: {
                    Image(systemName: isPlaying ? "pause.fill" : "play.fill")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(.black)
                        .frame(width: 40, height: 40)
                        .background(Circle().fill(LiquidGlass.emeraldMint))
                        .shadow(color: LiquidGlass.emeraldMint.opacity(0.4), radius: 6)
                }
                .buttonStyle(.plain)

                // Speed Selector
                Menu {
                    Button("0.5x Speed") { playbackSpeed = 0.5; updatePlaybackTimer() }
                    Button("1.0x Normal") { playbackSpeed = 1.0; updatePlaybackTimer() }
                    Button("2.0x Fast") { playbackSpeed = 2.0; updatePlaybackTimer() }
                } label: {
                    Text(String(format: "%.1fx", playbackSpeed))
                        .font(.system(size: 12, weight: .bold, design: .monospaced))
                        .foregroundColor(.white)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(Capsule().fill(.ultraThinMaterial.opacity(0.7)))
                }

                Spacer()

                // Tail Removal Toggle (Last 5s vs Full Path)
                Picker("Tail Mode", selection: $tailMode) {
                    ForEach(ReplayTailMode.allCases) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 170)
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
                guard !recordedFrames.isEmpty else { return }
                if currentFrameIndex + 1 < recordedFrames.count {
                    currentFrameIndex += 1
                } else {
                    currentFrameIndex = 0 // loop
                }
            }
        }
    }
}
