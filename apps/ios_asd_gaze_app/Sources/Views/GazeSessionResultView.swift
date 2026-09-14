import SwiftUI
import CurvAsdGazeKit

// MARK: - Post-Observation Clinical Result View

/// Post-observation clinical evidence and biomarker review view.
public struct GazeSessionResultView: View {
    public let summary: GazeSessionReportSummary
    public let recordedFrames: [GazeFrame]
    public let classification: BiomarkerClassificationResult?
    public let onDismiss: () -> Void
    public let onRetake: () -> Void

    @State private var replayFrameIndex: Int = 0
    @State private var isReplaying: Bool = false
    @State private var replayTimer: Timer?
    @State private var showFullScreenReplay: Bool = false
    @State private var showExportSheet: Bool = false
    @State private var showShareSheet: Bool = false
    @State private var shareItems: [Any] = []
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
                            Text("New Screening")
                        }
                        .foregroundColor(LiquidGlass.celestialTeal)
                    }
                }
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 2) {
                        Text(Localizer.tr("result_title"))
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("Session #\(summary.sessionId.prefix(8))")
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundColor(.secondary)
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: triggerShareReport) {
                        Image(systemName: "square.and.arrow.up")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }
                    .accessibilityLabel("Share Clinical Report")
                }
            }
            .navigationDestination(for: BiomarkerDetailType.self) { detailType in
                BiomarkerDetailView(
                    type: detailType,
                    summary: summary,
                    classification: classification,
                    recordedFrames: recordedFrames
                )
            }
        }
        .fullScreenCover(isPresented: $showFullScreenReplay) {
            GazeReplayHeatmapFullScreenView(
                summary: summary,
                recordedFrames: recordedFrames
            )
        }
        .sheet(isPresented: $showShareSheet) {
            ActivityViewController(activityItems: shareItems)
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
        VStack(spacing: 10) {
            ZStack {
                // Background thumbnail representing the stimulus
                HStack(spacing: 1) {
                    Color(red: 0.10, green: 0.16, blue: 0.28)
                        .overlay(Text("Social Actor").font(.system(size: 11, weight: .bold)).foregroundColor(.white.opacity(0.4)))
                    Color(red: 0.20, green: 0.08, blue: 0.24)
                        .overlay(Text("Geometric").font(.system(size: 11, weight: .bold)).foregroundColor(.white.opacity(0.4)))
                }

                // Gaze Trajectory Lines (Scanpath with Blink Discontinuity Handling)
                GeometryReader { geo in
                    let validFrames = recordedFrames.filter { $0.gaze.isValid && $0.qualityVerdict != .rejected }
                    Path { path in
                        guard validFrames.count > 1 else { return }
                        for i in 1..<validFrames.count {
                            let prev = validFrames[i - 1].gaze
                            let curr = validFrames[i].gaze
                            let dt = curr.videoTimeSec - prev.videoTimeSec
                            // Skip connecting lines across blink gaps or tracking drops
                            guard dt > 0 && dt <= 0.12 else { continue }
                            path.move(to: CGPoint(x: prev.xNorm * geo.size.width, y: prev.yNorm * geo.size.height))
                            path.addLine(to: CGPoint(x: curr.xNorm * geo.size.width, y: curr.yNorm * geo.size.height))
                        }
                    }
                    .stroke(LiquidGlass.celestialTeal.opacity(0.6), lineWidth: 2)

                    // Replay head point
                    if !recordedFrames.isEmpty {
                        let activeFrame = recordedFrames[min(replayFrameIndex, recordedFrames.count - 1)]
                        let isValid = activeFrame.gaze.isValid && activeFrame.qualityVerdict != .rejected
                        let activePt = CGPoint(x: activeFrame.gaze.xNorm * geo.size.width, y: activeFrame.gaze.yNorm * geo.size.height)
                        Circle()
                            .fill(isValid ? LiquidGlass.emeraldMint : Color.gray.opacity(0.5))
                            .frame(width: 14, height: 14)
                            .position(activePt)
                            .shadow(color: isValid ? LiquidGlass.emeraldMint : .clear, radius: 6)
                            .opacity(isValid ? 1.0 : 0.35)
                    }
                }
            }
            .frame(height: 160)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(LiquidGlass.cardBorder, lineWidth: 1)
            )

            // Replay Mini Bar
            HStack {
                Button(action: toggleReplay) {
                    Image(systemName: isReplaying ? "pause.fill" : "play.fill")
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .frame(width: 30, height: 30)
                        .background(Circle().fill(LiquidGlass.cardBackground))
                }

                Text(String(format: "%.1fs / %.1fs (%d frames)", Double(replayFrameIndex) * (1.0 / 60.0), summary.durationSec, recordedFrames.count))
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundColor(.primary.opacity(0.85))

                Spacer()
            }

            // Dedicated Button to Open Full-Screen Replay & Heatmap
            Button {
                showFullScreenReplay = true
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "arrow.up.left.and.arrow.down.right")
                        .font(.system(size: 13, weight: .semibold))
                    Text("Open Full-Screen Replay & Heatmap")
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                    Spacer()
                    Text("5s Tail / Flow")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(Color.white.opacity(0.8))
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .semibold))
                }
                .foregroundColor(.white)
                .padding(.vertical, 10)
                .padding(.horizontal, 14)
                .background(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(LiquidGlass.celestialTeal)
                )
            }
            .buttonStyle(.plain)
        }
        .glassCard(cornerRadius: 16, padding: 12)
    }

    // MARK: - Social Preference Card (Drillable)

    private var socialPreferenceCard: some View {
        NavigationLink(value: BiomarkerDetailType.socialPreference) {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    HStack(spacing: 6) {
                        Image(systemName: "eyes.inverse")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(LiquidGlass.celestialTeal)
                        Text(Localizer.tr("biomarkers_title"))
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.secondary)
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

                            Circle()
                                .fill(Color.white)
                                .frame(width: 18, height: 18)
                                .shadow(radius: 4)
                                .offset(x: max(0, min(geo.size.width - 18, normX - 9)))
                        }
                    }
                    .frame(height: 18)

                    HStack {
                        Text("Geometric (-1.0)")
                            .font(.system(size: 10))
                            .foregroundColor(.purple)
                        Spacer()
                        Text(String(format: "%+.2f", summary.socialPreferenceScore))
                            .font(.system(size: 12, weight: .bold, design: .monospaced))
                            .foregroundColor(.primary)
                        Spacer()
                        Text("Social (+1.0)")
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
        .buttonStyle(.plain)
    }

    // MARK: - CoreML Risk Stratification Card (Drillable)

    private var mlClassificationCard: some View {
        let result = classification ?? BiomarkerClassificationEngine.shared.classifySync(summary: summary)
        let stratColor: Color = {
            switch result.riskStratification {
            case .typical: return LiquidGlass.emeraldMint
            case .monitor: return LiquidGlass.warmAmber
            case .elevatedRisk: return Color.red
            }
        }()

        return NavigationLink(value: BiomarkerDetailType.mlClassification) {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    HStack(spacing: 6) {
                        Image(systemName: "brain.head.profile")
                            .font(.system(size: 13))
                            .foregroundColor(LiquidGlass.celestialTeal)
                        Text("AI Biomarker Risk Stratification")
                            .font(.system(size: 14, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.secondary)
                }

                HStack(spacing: 10) {
                    Circle()
                        .fill(stratColor)
                        .frame(width: 10, height: 10)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(result.riskStratification.rawValue)
                            .font(.system(size: 13, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                        Text("Primary Feature: \(result.primaryContributingFeature)")
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
                .background(RoundedRectangle(cornerRadius: 12).fill(LiquidGlass.cardBackground))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(LiquidGlass.cardBorder, lineWidth: 1))

                HStack(spacing: 12) {
                    metricCol(title: "Risk Index", value: String(format: "%.1f%%", result.riskScore * 100))
                    metricCol(title: "Inference Latency", value: String(format: "%.1f ms", result.inferenceLatencyMs))
                    metricCol(title: "Engine", value: result.executionDevice == .appleNeuralEngine ? "ANE CoreML" : "CPU")
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Facial Affect Card (Drillable)

    private var facialAffectMetricsCard: some View {
        NavigationLink(value: BiomarkerDetailType.facialAffect) {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    HStack(spacing: 6) {
                        Image(systemName: "face.smiling.fill")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(LiquidGlass.emeraldMint)
                        Text(Localizer.tr("emotion_title"))
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(.primary)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.secondary)
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
        .buttonStyle(.plain)
    }

    // MARK: - Quality & ResearchKit Card (Drillable)

    private var qualityAndContractCard: some View {
        NavigationLink(value: BiomarkerDetailType.dataQuality) {
            HStack(spacing: 14) {
                Image(systemName: "checkmark.seal.fill")
                    .foregroundColor(LiquidGlass.emeraldMint)
                    .font(.system(size: 26))

                VStack(alignment: .leading, spacing: 2) {
                    Text("ResearchKit & Sensor Integrity")
                        .font(.system(size: 13, weight: .bold))
                        .foregroundColor(.primary)
                    Text(String(format: "Valid Frames: %.1f%% • Verdict: %@", summary.validRatio * 100, summary.qualityVerdict.rawValue.capitalized))
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                }

                Spacer()

                Image(systemName: "chevron.right")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(.secondary)
            }
            .glassCard(cornerRadius: 14, padding: 12)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Action Buttons Section (No "Done" button)

    private var actionButtonsSection: some View {
        VStack(spacing: 10) {
            // Share Clinical Summary Button
            GlassButton(
                title: "Share Clinical Report",
                systemImage: "square.and.arrow.up.fill",
                isPrimary: true
            ) {
                triggerShareReport()
            }

            // Export ResearchKit JSON Button
            GlassButton(
                title: "View ResearchKit JSON Export",
                systemImage: "doc.text.fill",
                isPrimary: false
            ) {
                prepareShareExport()
                showExportSheet = true
            }

            // Return to Start
            Button(action: onDismiss) {
                Text("Return to Dashboard")
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .padding(.vertical, 8)
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

    private func triggerShareReport() {
        prepareShareExport()

        let dateStr = Date().formatted(date: .abbreviated, time: .shortened)
        let reportText = """
        ========================================
        ASD GAZE — CLINICAL SCREENING REPORT
        ========================================
        Session ID: \(summary.sessionId)
        Date: \(dateStr)
        Duration: \(String(format: "%.1f", summary.durationSec)) seconds
        Tracking Validity: \(String(format: "%.1f", summary.validRatio * 100))% (\(summary.validFrames)/\(summary.totalFrames) frames)

        BIOMARKERS:
        • Social Preference Index (GeoPref): \(String(format: "%+.2f", summary.socialPreferenceScore))
        • Social Dwell Time: \(String(format: "%.1f", summary.socialDwellSec))s
        • Geometric Dwell Time: \(String(format: "%.1f", summary.geometricDwellSec))s
        • Eye Fixation Ratio: \(String(format: "%.1f", summary.eyeDwellRatio * 100))%
        • Mouth Fixation Ratio: \(String(format: "%.1f", summary.mouthDwellRatio * 100))%
        • First-Eye Latency: \(String(format: "%.0f", summary.firstEyeLatencyMs)) ms

        FACIAL AFFECT (FACS):
        • Social Smile Mean: \(String(format: "%.0f%%", summary.socialSmileIntensityMean * 100))
        • Smile Reactivity Index: \(String(format: "%.2f", summary.smileReactivityIndex))
        • Flat Affect Score: \(String(format: "%.2f", summary.flatAffectScore))
        • Surprise Score Mean: \(String(format: "%.0f%%", summary.surpriseScoreMean * 100))

        AI CLASSIFICATION:
        • Stratification: \(classification?.riskStratification.rawValue ?? "Clinical Review Needed")
        • Risk Index: \(String(format: "%.2f", classification?.riskScore ?? 0.0))
        • Primary Contributor: \(classification?.primaryContributingFeature ?? "GeoPref Attention")

        RESEARCHKIT CONTRACT:
        \(exportText)
        """

        shareItems = [reportText]
        showShareSheet = true
    }
}

// MARK: - Native iOS Share Sheet Representable

public struct ActivityViewController: UIViewControllerRepresentable {
    public var activityItems: [Any]
    public var applicationActivities: [UIActivity]? = nil

    public func makeUIViewController(context: Context) -> UIActivityViewController {
        let controller = UIActivityViewController(
            activityItems: activityItems,
            applicationActivities: applicationActivities
        )
        return controller
    }

    public func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

// MARK: - ResearchKit Export Sheet

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
                    .background(Color.black.opacity(0.85))
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
                            .foregroundColor(.black)
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
                    Button("Close") { dismiss() }
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}
