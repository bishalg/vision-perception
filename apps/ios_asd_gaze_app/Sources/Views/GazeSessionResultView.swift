import SwiftUI
import CurvAsdGazeKit

/// Post-observation clinical evidence and biomarker review view.
/// Directly mirrors the Palm Scanner's CaptureResultView architecture.
public struct GazeSessionResultView: View {
    public let summary: GazeSessionReportSummary
    public let recordedFrames: [GazeFrame]
    public let classification: BiomarkerClassificationResult?
    public let onDismiss: () -> Void
    public let onRetake: () -> Void

    @State private var replayFrameIndex: Int = 0
    @State private var isReplaying: Bool = false
    @State private var replayTimer: Timer?
    @State private var showHeatmap: Bool = false
    @State private var showExportSheet: Bool = false
    @State private var exportText: String = ""

    public init(
        summary: GazeSessionReportSummary,
        recordedFrames: [GazeFrame],
        classification: BiomarkerClassificationResult? = nil,
        onDismiss: @escaping () -> Void,
        onRetake: @escaping () -> Void
    ) {
        self.summary = summary
        self.recordedFrames = recordedFrames
        self.classification = classification
        self.onDismiss = onDismiss
        self.onRetake = onRetake
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()

                ScrollView {
                    VStack(spacing: 16) {
                        scanpathHeroCanvas
                        socialPreferenceCard
                        mlClassificationCard
                        facialAffectMetricsCard
                        qualityAndContractCard
                        actionButtonsSection
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                }
            }
            .navigationTitle(Localizer.tr("result_title"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(action: onRetake) {
                        HStack(spacing: 4) {
                            Image(systemName: "arrow.counterclockwise")
                            Text(Localizer.tr("retake"))
                        }
                        .foregroundColor(LiquidGlass.celestialTeal)
                    }
                }
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 2) {
                        Text(Localizer.tr("result_title"))
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("Session #\(summary.sessionId)")
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundColor(.secondary)
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    HStack(spacing: 12) {
                        Button(action: prepareShareExport) {
                            Image(systemName: "square.and.arrow.up")
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }
                        Button(action: onDismiss) {
                            Text("Done")
                                .fontWeight(.semibold)
                                .foregroundColor(LiquidGlass.emeraldMint)
                        }
                    }
                }
            }
        }
        .sheet(isPresented: $showExportSheet) {
            ResearchKitExportSheet(exportJsonText: exportText)
        }
        .onDisappear {
            replayTimer?.invalidate()
        }
    }

    // MARK: - Scanpath Hero Canvas

    private var scanpathHeroCanvas: some View {
        VStack(spacing: 8) {
            ZStack {
                // Background thumbnail representing the stimulus
                HStack(spacing: 1) {
                    Color(red: 0.10, green: 0.16, blue: 0.28)
                        .overlay(Text("Social Actor").font(.system(size: 12, weight: .bold)).foregroundColor(.white.opacity(0.4)))
                    Color(red: 0.20, green: 0.08, blue: 0.24)
                        .overlay(Text("Geometric").font(.system(size: 12, weight: .bold)).foregroundColor(.white.opacity(0.4)))
                }

                // Gaze Trajectory Lines (Scanpath)
                GeometryReader { geo in
                    let points = sampleGazeTrajectory(in: geo.size)
                    Path { path in
                        guard let first = points.first else { return }
                        path.move(to: first)
                        for pt in points.dropFirst() {
                            path.addLine(to: pt)
                        }
                    }
                    .stroke(LiquidGlass.celestialTeal.opacity(0.6), lineWidth: 2)

                    // Replay head point
                    if !points.isEmpty {
                        let activePt = points[min(replayFrameIndex, points.count - 1)]
                        Circle()
                            .fill(LiquidGlass.emeraldMint)
                            .frame(width: 14, height: 14)
                            .position(activePt)
                            .shadow(color: LiquidGlass.emeraldMint, radius: 6)
                    }
                }
            }
            .frame(height: 170)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(LiquidGlass.cardBorder, lineWidth: 1)
            )

            // Replay Controls Bar
            HStack {
                Button(action: toggleReplay) {
                    Image(systemName: isReplaying ? "pause.fill" : "play.fill")
                        .font(.system(size: 14))
                        .foregroundColor(.primary)
                        .frame(width: 32, height: 32)
                        .background(Circle().fill(LiquidGlass.cardBackground))
                }

                Text(String(format: "Frame %d / %d (%.1fs)", min(replayFrameIndex, max(1, recordedFrames.count)), max(1, recordedFrames.count), summary.durationSec))
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundColor(.primary.opacity(0.85))

                Spacer()

                Toggle("Heatmap", isOn: $showHeatmap)
                    .toggleStyle(.button)
                    .tint(LiquidGlass.celestialTeal)
                    .font(.system(size: 11, weight: .semibold))
            }
        }
        .glassCard(cornerRadius: 16, padding: 12)
    }

    // MARK: - Social Preference Card

    private var socialPreferenceCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(Localizer.tr("biomarkers_title"))
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)
                Spacer()
                Text("GeoPref Index")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }

            // Preference Spectrum Bar [-1.0 ... +1.0]
            VStack(spacing: 4) {
                GeometryReader { geo in
                    let clamped = max(-1.0, min(1.0, summary.socialPreferenceScore))
                    let normX = (clamped + 1.0) / 2.0 * geo.size.width

                    ZStack(alignment: .leading) {
                        Capsule()
                            .fill(
                                LinearGradient(
                                    colors: [Color.purple.opacity(0.7), Color.gray.opacity(0.3), LiquidGlass.celestialTeal],
                                    startPoint: .leading, endPoint: .trailing
                                )
                            )
                            .frame(height: 12)

                        // Marker pill on the replay timeline (media surface).
                        // theme:fixed-media
                        Circle()
                            .fill(Color.white)
                            .frame(width: 18, height: 18)
                            .shadow(radius: 4)
                            .offset(x: max(0, min(geo.size.width - 18, normX - 9)))
                    }
                }
                .frame(height: 18)

                HStack {
                    Text("Geometric Focus (-1.0)")
                        .font(.system(size: 10))
                        .foregroundColor(.purple)
                    Spacer()
                    Text(String(format: "%+.2f", summary.socialPreferenceScore))
                        .font(.system(size: 12, weight: .bold, design: .monospaced))
                        .foregroundColor(.primary)
                    Spacer()
                    Text("Social Focus (+1.0)")
                        .font(.system(size: 10))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }

            Divider().background(.primary.opacity(0.08))

            // Additional Gaze Metrics Grid
            HStack(spacing: 16) {
                metricCol(title: "Eye Dwell", value: String(format: "%.1f%%", summary.eyeDwellRatio * 100))
                metricCol(title: "Mouth Dwell", value: String(format: "%.1f%%", summary.mouthDwellRatio * 100))
                metricCol(title: "First-Eye Lat", value: String(format: "%.0f ms", summary.firstEyeLatencyMs))
                metricCol(title: "Social Dwell", value: String(format: "%.1fs", summary.socialDwellSec))
            }
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    // MARK: - Facial Affect & Emotion Card

    private var facialAffectMetricsCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(Localizer.tr("emotion_title"))
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)
                Spacer()
                Text("52 FACS Telemetry")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundColor(LiquidGlass.emeraldMint)
            }

            HStack(spacing: 16) {
                metricCol(title: "Smile Reactivity", value: String(format: "%.2f", summary.smileReactivityIndex))
                metricCol(title: "Mean Smile", value: String(format: "%d%%", Int(summary.socialSmileIntensityMean * 100)))
                metricCol(title: "Flat Affect", value: String(format: "%.2f", summary.flatAffectScore))
                metricCol(title: "Surprise Mean", value: String(format: "%d%%", Int(summary.surpriseScoreMean * 100)))
            }
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    // MARK: - CoreML Biomarker Risk Stratification Card

    private var mlClassificationCard: some View {
        let result = classification ?? BiomarkerClassificationEngine.shared.classifySync(summary: summary)
        let stratColor: Color = {
            switch result.riskStratification {
            case .typical: return LiquidGlass.emeraldMint
            case .monitor: return LiquidGlass.warmAmber
            case .elevatedRisk: return Color.red
            }
        }()

        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                HStack(spacing: 6) {
                    Image(systemName: "cpu.fill")
                        .font(.system(size: 13))
                        .foregroundColor(LiquidGlass.celestialTeal)
                    Text("CoreML Biomarker Risk Stratification")
                        .font(.system(size: 14, weight: .bold, design: .rounded))
                        .foregroundColor(.primary)
                }
                Spacer()
                Text(result.executionDevice == .appleNeuralEngine ? "ANE Accelerated" : "Clinical Baseline")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 3)
                    .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.15)))
            }

            // Stratification Badge Banner
            HStack(spacing: 10) {
                Circle()
                    .fill(stratColor)
                    .frame(width: 10, height: 10)

                VStack(alignment: .leading, spacing: 2) {
                    Text(result.riskStratification.rawValue)
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundColor(.primary)
                    Text("Primary Contributing Feature: \(result.primaryContributingFeature)")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)
                }

                Spacer()

                VStack(alignment: .trailing, spacing: 1) {
                    Text(String(format: "%.0f%%", result.confidence * 100))
                        .font(.system(size: 14, weight: .bold, design: .monospaced))
                        .foregroundColor(.primary)
                    Text("Confidence")
                        .font(.system(size: 9))
                        .foregroundColor(.secondary)
                }
            }
            .padding(10)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(LiquidGlass.cardBackground)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(LiquidGlass.cardBorder, lineWidth: 1)
            )

            HStack(spacing: 12) {
                metricCol(title: "Risk Probability", value: String(format: "%.1f%%", result.riskScore * 100))
                metricCol(title: "Inference Latency", value: String(format: "%.1f ms", result.inferenceLatencyMs))
                metricCol(title: "Execution Device", value: result.executionDevice == .appleNeuralEngine ? "ANE" : "CPU")
            }
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    // MARK: - Quality & ResearchKit Contract Card

    private var qualityAndContractCard: some View {
        HStack(spacing: 14) {
            Image(systemName: "checkmark.seal.fill")
                .foregroundColor(LiquidGlass.emeraldMint)
                .font(.system(size: 26))

            VStack(alignment: .leading, spacing: 2) {
                Text("ResearchKit & CareKit Contract")
                    .font(.system(size: 13, weight: .bold))
                    .foregroundColor(.primary)
                Text(String(format: "Valid Samples: %.1f%% • Verdict: %@", summary.validRatio * 100, summary.qualityVerdict.rawValue.capitalized))
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
            }

            Spacer()

            Button("View JSON") {
                prepareShareExport()
                showExportSheet = true
            }
            .font(.system(size: 11, weight: .bold))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Capsule().fill(LiquidGlass.cardBackground))
            .foregroundColor(LiquidGlass.celestialTeal)
        }
        .glassCard(cornerRadius: 14, padding: 12)
    }

    // MARK: - Action Buttons Section

    private var actionButtonsSection: some View {
        VStack(spacing: 10) {
            GlassButton(
                title: Localizer.tr("export_pdf"),
                systemImage: "doc.text.fill",
                isPrimary: true
            ) {
                // Generates PDF export via SessionVaultManager
                prepareShareExport()
                showExportSheet = true
            }

            Button(action: onDismiss) {
                Text("Done")
                    .font(.system(size: 15, weight: .semibold, design: .rounded))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
            }
        }
    }

    // MARK: - Helper Methods

    private func metricCol(title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            Text(value)
                .font(.system(size: 14, weight: .bold, design: .monospaced))
                .foregroundColor(.primary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func sampleGazeTrajectory(in size: CGSize) -> [CGPoint] {
        guard !recordedFrames.isEmpty else {
            return (0..<20).map { i in
                CGPoint(x: size.width * (0.2 + 0.03 * Double(i)), y: size.height * (0.4 + 0.02 * sin(Double(i))))
            }
        }
        return recordedFrames.map { frame in
            CGPoint(x: frame.gaze.xNorm * size.width, y: frame.gaze.yNorm * size.height)
        }
    }

    private func toggleReplay() {
        if isReplaying {
            replayTimer?.invalidate()
            isReplaying = false
        } else {
            isReplaying = true
            replayTimer?.invalidate()
            replayTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { _ in
                MainActor.assumeIsolated {
                    let maxCount = max(1, recordedFrames.count)
                    replayFrameIndex = (replayFrameIndex + 1) % maxCount
                }
            }
        }
    }

    private func prepareShareExport() {
        let result = classification ?? BiomarkerClassificationEngine.shared.classifySync(summary: summary)
        let rk = SessionVaultManager.makeResearchKitJson(report: summary, classification: result)
        if let data = try? JSONSerialization.data(withJSONObject: rk, options: [.prettyPrinted]),
           let str = String(data: data, encoding: .utf8) {
            exportText = str
        } else {
            exportText = "{ \"sessionId\": \"\(summary.sessionId)\" }"
        }
    }
}

/// Modal sheet displaying the generated ResearchKit JSON result contract.
public struct ResearchKitExportSheet: View {
    public let exportJsonText: String
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var syncService = ResearchKitSyncService.shared

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                VStack(spacing: 14) {
                    ScrollView {
                        Text(exportJsonText)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundColor(.green)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(16)
                    }
                    .background(Color.black.opacity(0.85)) // theme:fixed-media
                    .cornerRadius(12)

                    VStack(spacing: 8) {
                        Button {
                            syncService.syncNow()
                        } label: {
                            HStack {
                                if syncService.syncState.isSyncing {
                                    ProgressView().tint(.black)
                                } else {
                                    Image(systemName: "icloud.and.arrow.up.fill")
                                }
                                Text("Transmit to Research Institute Server")
                                    .fontWeight(.bold)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                            .background(LiquidGlass.celestialTeal)
                            .foregroundColor(.black) // theme:fixed-media
                            .clipShape(RoundedRectangle(cornerRadius: 12))
                        }
                        .disabled(syncService.syncState.isSyncing)

                        Text(syncService.syncState.description)
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                    }
                }
                .padding(16)
            }
            .navigationTitle("ResearchKit Result JSON")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}
