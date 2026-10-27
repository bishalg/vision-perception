import SwiftUI
import CurvAsdGazeKit

// MARK: - Post-Observation Result View

/// Post-observation review of what the child looked at. The only interpretation layer in
/// Release builds is `AttentionFlag`; the experimental classifier card exists only under
/// `ASD_RESEARCH_CLASSIFIER` (ADR-0004 decision 3, CLM-3). Simulated captures always show the
/// "Simulated data" badge (SIM-8) and every surface carries the not-diagnostic footer (CLM-5).
public struct GazeSessionResultView: View {
    public let summary: GazeSessionReportSummary
    public let recordedFrames: [GazeFrame]
    #if ASD_RESEARCH_CLASSIFIER
    public let classification: BiomarkerClassificationResult?
    #endif
    public let onDismiss: () -> Void
    public let onRetake: () -> Void

    @State private var replayFrameIndex: Int = 0
    @State private var isReplaying: Bool = false
    @State private var replayTimer: Timer?
    @State private var showFullScreenReplay: Bool = false
    #if DEBUG
    @State private var showExportSheet: Bool = false
    #endif
    @State private var showShareSheet: Bool = false
    @State private var shareItems: [Any] = []
    #if DEBUG
    @State private var exportText: String = ""
    #endif
    @State private var perVideoScore: PerVideoSessionScore?
    @State private var perVideoBaselineStatus: String?

    #if ASD_RESEARCH_CLASSIFIER
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
    #else
    public init(
        summary: GazeSessionReportSummary,
        recordedFrames: [GazeFrame],
        onDismiss: @escaping () -> Void,
        onRetake: @escaping () -> Void
    ) {
        self.summary = summary
        self.recordedFrames = recordedFrames
        self.onDismiss = onDismiss
        self.onRetake = onRetake
    }
    #endif

    /// Pseudonymous participant token (CNS-7). The child's name never enters a record.
    private var participantToken: String { "child-\(summary.sessionId.prefix(8))" }

    /// Why this session cannot be exported as a FHIR health record (simulated or rejected,
    /// SIM-5/FHIR-2); `nil` when it can. Shown under the disabled export button (acceptance 2).
    private var fhirRefusalReason: String? {
        FHIRBundleBuilder.exportRefusalReason(for: GazeSessionRecord(summary: summary, participantToken: participantToken))
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
                        SimulatedDataBadge(captureSource: summary.captureSource)
                        scanpathHeroCanvas
                        GazeTelemetryReviewCard(
                            session: GazeTelemetrySession(from: GazeSessionRecord(
                                summary: summary,
                                participantToken: participantToken
                            ))
                        )
                        if let score = perVideoScore {
                            AttentionFlagBanner(assessment: AttentionFlagAlgorithm.assess(score))
                        }
                        socialPreferenceCard
                        perVideoMatrixEntry
                        medicalDisclaimerBanner
                        #if ASD_RESEARCH_CLASSIFIER
                        mlClassificationCard
                        #endif
                        facialAffectMetricsCard
                        qualityAndContractCard
                        actionButtonsSection
                        NotDiagnosticFooter()
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
                    Button(action: onDismiss) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 20))
                            .foregroundColor(LiquidGlass.textTertiary)
                    }
                    .accessibilityLabel("Close Report")
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
                    .accessibilityLabel("Share Report")
                }
            }
            .navigationDestination(for: BiomarkerDetailType.self) { detailType in
                #if ASD_RESEARCH_CLASSIFIER
                BiomarkerDetailView(
                    type: detailType,
                    summary: summary,
                    classification: classification,
                    recordedFrames: recordedFrames
                )
                #else
                BiomarkerDetailView(
                    type: detailType,
                    summary: summary,
                    recordedFrames: recordedFrames
                )
                #endif
            }
            .navigationDestination(for: PerVideoNavRoute.self) { route in
                switch route {
                case .summary:
                    if let score = perVideoScore {
                        PerVideoSummaryListView(score: score)
                    }
                case .video:
                    if let score = perVideoScore {
                        PerVideoResultDetailView(score: score, route: route)
                    }
                }
            }
            .onAppear {
                // Per-video scoring is linear over frames; compute once per presentation.
                if perVideoScore == nil {
                    let model = PerVideoMatrixModel(
                        recordedFrames: recordedFrames,
                        childAgeMonths: ChildProfileStore.shared.profile.ageBracket.midpointMonths
                    )
                    perVideoScore = model.sessionScore
                    perVideoBaselineStatus = model.baselineStatus
                }
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
        #if DEBUG
        .sheet(isPresented: $showExportSheet) {
            ResearchKitExportSheet(exportJsonText: exportText)
        }
        #endif
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
                    LiquidGlass.stimulusSocialMedia
                        .overlay(Text("People").font(.system(size: 11, weight: .bold)).foregroundColor(.white.opacity(0.4)))
                    LiquidGlass.stimulusGeometryMedia
                        .overlay(Text("Shapes").font(.system(size: 11, weight: .bold)).foregroundColor(.white.opacity(0.4)))
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
                    Text("Open Full-Screen Replay")
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

    /// Entry point into the per-stimulus report stack: summary card that pushes the sub-report landing page.
    @ViewBuilder
    private var perVideoMatrixEntry: some View {
        if let score = perVideoScore, score.videos.contains(where: { $0.hasBaseline }) {
            NavigationLink(value: PerVideoNavRoute.summary) {
                PerVideoMatrixCardView(score: score, baselineStatus: perVideoBaselineStatus)
            }
            .buttonStyle(.plain)
        }
    }

    private var socialPreferenceCard: some View {
        NavigationLink(value: BiomarkerDetailType.socialPreference) {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    HStack(spacing: 6) {
                        Image(systemName: "eyes.inverse")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(LiquidGlass.celestialTeal)
                        Text(Localizer.tr("social_attention_title"))
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
                    metricCol(title: "Eye Looking", value: Self.percentText(summary.eyeDwellRatio))
                    metricCol(title: "Mouth Looking", value: Self.percentText(summary.mouthDwellRatio))
                    metricCol(title: "First Look at Eyes", value: summary.firstEyeLatencyMs.map { String(format: "%.0f ms", $0) } ?? "Not observed")
                    metricCol(title: "Time on People", value: String(format: "%.1fs", summary.socialDwellSec))
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Experimental classifier card (Debug builds only, CLM-3)

    #if ASD_RESEARCH_CLASSIFIER
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
                        Text("Experimental Classifier (Debug Only)")
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
                        Text("Model certainty")
                            .font(.system(size: 9))
                            .foregroundColor(.secondary)
                    }
                }
                .padding(10)
                .background(RoundedRectangle(cornerRadius: 12).fill(LiquidGlass.cardBackground))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(LiquidGlass.cardBorder, lineWidth: 1))

                HStack(spacing: 12) {
                    metricCol(title: "Index", value: String(format: "%.1f%%", result.riskScore * 100))
                    metricCol(title: "Inference Latency", value: String(format: "%.1f ms", result.inferenceLatencyMs))
                    metricCol(title: "Engine", value: result.executionDevice == .appleNeuralEngine ? "Neural Engine" : "CPU")
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
        .buttonStyle(.plain)
    }
    #endif

    private var medicalDisclaimerBanner: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "cross.case.fill")
                .foregroundColor(LiquidGlass.warmAmber)
                .font(.system(size: 16))
                .padding(.top, 2)

            VStack(alignment: .leading, spacing: 4) {
                Text("A Diary of What Your Child Looked At")
                    .font(.system(size: 12, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)

                Text("EarlyGaze Observation Diary records where your child looks during short clips, so you can track it over time and share it with your pediatrician. Please talk with your pediatrician about any concerns about your child’s development.")
                    .font(.system(size: 11))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .lineSpacing(2)
            }
        }
        .padding(12)
        .background(LiquidGlass.warmAmber.opacity(0.12))
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(LiquidGlass.warmAmber.opacity(0.35), lineWidth: 1)
        )
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
                    Text("Recording Quality")
                        .font(.system(size: 13, weight: .bold))
                        .foregroundColor(.primary)
                    Text(String(format: "Usable samples: %.1f%% • Quality: %@", summary.validRatio * 100, summary.qualityVerdict.rawValue.capitalized))
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

    // MARK: - Action Buttons Section

    private var actionButtonsSection: some View {
        VStack(spacing: 12) {
            GlassButton(
                title: "New Observation",
                systemImage: "arrow.counterclockwise",
                isPrimary: true
            ) {
                onRetake()
            }

            // Share report button (vector PDF; the data file is attached in Debug builds only)
            GlassButton(
                title: "Share Report for Your Pediatrician",
                systemImage: "square.and.arrow.up.fill",
                isPrimary: false
            ) {
                triggerShareReport()
            }

            // FHIR R4 Bundle export (FHIR-1); disabled with the reason for refused sessions.
            VStack(spacing: 6) {
                GlassButton(
                    title: "Export Health Record (FHIR)",
                    systemImage: "heart.text.square",
                    isPrimary: false
                ) {
                    exportFHIRBundle()
                }
                .disabled(fhirRefusalReason != nil)
                .opacity(fhirRefusalReason == nil ? 1 : 0.5)
                .accessibilityHint(fhirRefusalReason ?? "")

                if let reason = fhirRefusalReason {
                    Text(reason)
                        .font(.system(size: 12))
                        .foregroundColor(LiquidGlass.textSecondary)
                        .multilineTextAlignment(.center)
                }
            }

            #if DEBUG
            // Developer-only raw data viewer; the released app has no research export surface.
            GlassButton(
                title: "View Data File (Debug)",
                systemImage: "doc.text.fill",
                isPrimary: false
            ) {
                prepareShareExport()
                showExportSheet = true
            }
            #endif

            // Return to Dashboard Action
            Button(action: onDismiss) {
                HStack(spacing: 6) {
                    Image(systemName: "house.fill")
                        .font(.system(size: 13))
                    Text("Return to Dashboard")
                        .font(.system(size: 14, weight: .semibold, design: .rounded))
                }
                .foregroundColor(LiquidGlass.textSecondary)
                .padding(.vertical, 8)
            }
        }
        .padding(.top, 4)
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

    private static func percentText(_ ratio: Double?) -> String {
        guard let ratio else { return "Not observed" }
        return String(format: "%.1f%%", ratio * 100)
    }

    #if DEBUG
    private func prepareShareExport() {
        #if ASD_RESEARCH_CLASSIFIER
        let result = classification ?? BiomarkerClassificationEngine.shared.classifySync(summary: summary)
        let rk = SessionVaultManager.makeResearchKitJson(
            report: summary,
            classification: result,
            perVideoScore: perVideoScore
        )
        #else
        let rk = SessionVaultManager.makeResearchKitJson(
            report: summary,
            perVideoScore: perVideoScore
        )
        #endif
        if let data = try? JSONSerialization.data(withJSONObject: rk, options: [.prettyPrinted, .sortedKeys]),
           let str = String(data: data, encoding: .utf8) {
            exportText = str
        } else {
            exportText = "{ \"sessionId\": \"\(summary.sessionId)\" }"
        }
    }
    #endif

    /// Shares the session as a FHIR R4 Bundle file (ADR-0003). The subject is the pseudonymous
    /// token only (CNS-7); the last M-CHAT-R/F result is included with its stored answers.
    private func exportFHIRBundle() {
        let record = GazeSessionRecord(summary: summary, participantToken: participantToken)
        guard FHIRBundleBuilder.exportRefusalReason(for: record) == nil else { return }
        let studyStore = ResearchKitStudyStore.shared
        let mchat = studyStore.fetchCompletedSurveys().last.map { result in
            (result, FHIRBundleBuilder.mchatAnswers(for: result, in: studyStore.fetchCompletedTaskResults()))
        }
        let subject = FHIRSubject(record: record, ageMonths: ChildProfileStore.shared.profile.ageBracket.midpointMonths)
        let bundle = FHIRBundleBuilder.bundle(for: record, mchat: mchat, extraResources: [], subject: subject)
        guard let data = try? FHIRBundleBuilder.jsonData(bundle) else { return }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("EarlyGaze_FHIR_\(summary.sessionId.prefix(8)).json")
        guard (try? data.write(to: url, options: [.atomic, .completeFileProtection])) != nil else { return }
        shareItems = [url]
        showShareSheet = true
    }

    private func triggerShareReport() {
        let profile = ChildProfileStore.shared.profile
        // The name may appear on screen and on the PDF cover only (CNS-7); never in a record.
        let trimmedName = profile.childName.trimmingCharacters(in: .whitespacesAndNewlines)
        let childName = trimmedName.isEmpty ? "Child" : trimmedName
        let childAgeMonths = profile.ageBracket.midpointMonths

        let record = GazeSessionRecord(
            summary: summary,
            participantToken: participantToken
        )

        let lastMChat = ResearchKitStudyStore.shared.fetchCompletedSurveys().last
        let carePlanStore = CarePlanStore.shared

        let input = DoctorReportInput(
            participantId: "\(childName) (\(participantToken))",
            childAgeMonths: childAgeMonths,
            guardianName: "Parent / Legal Guardian",
            assessmentDate: Date(),
            gazeRecord: record,
            mchatResult: lastMChat,
            careKitCompletionRate: carePlanStore.completionRatio,
            careKitMedicationsLogged: carePlanStore.medications.count,
            careKitDiaryEntriesLogged: carePlanStore.streakDays,
            notes: "Social preference index: \(String(format: "%+.2f", summary.socialPreferenceScore)). Usable samples: \(String(format: "%.1f%%", summary.validRatio * 100))."
        )

        let pdfData = DoctorReportGenerator.generatePDFData(input: input)
        let pdfFileName = "EarlyGaze_Report_\(summary.sessionId.prefix(8)).pdf"
        let pdfURL = FileManager.default.temporaryDirectory.appendingPathComponent(pdfFileName)
        try? pdfData.write(to: pdfURL, options: .atomic)

        var items: [Any] = []
        if FileManager.default.fileExists(atPath: pdfURL.path) {
            items.append(pdfURL)
        }

        var attachmentLines = ["• \(pdfFileName) (PDF report)"]
        #if DEBUG
        prepareShareExport()
        let jsonFileName = "EarlyGaze_Data_\(summary.sessionId.prefix(8)).json"
        let jsonURL = FileManager.default.temporaryDirectory.appendingPathComponent(jsonFileName)
        if let jsonData = exportText.data(using: .utf8) {
            try? jsonData.write(to: jsonURL, options: .atomic)
        }
        if FileManager.default.fileExists(atPath: jsonURL.path) {
            items.append(jsonURL)
            attachmentLines.append("• \(jsonFileName) (debug data file)")
        }
        #endif

        let dateStr = Date().formatted(date: .abbreviated, time: .shortened)
        let eyeMouthRatioStr: String
        if let eye = summary.eyeDwellRatio, let mouth = summary.mouthDwellRatio, mouth > 0 {
            eyeMouthRatioStr = String(format: "%.2f", eye / mouth)
        } else {
            eyeMouthRatioStr = "Not observed"
        }
        let socialDwellRatio = summary.durationSec > 0 ? (summary.socialDwellSec / summary.durationSec) : 0.0
        let geomDwellRatio = summary.durationSec > 0 ? (summary.geometricDwellSec / summary.durationSec) : 0.0
        let mchatLine = lastMChat.map { "\($0.totalFailed)/20 items flagged. \(ClaimsCopy.mchatLine(for: $0.riskLevel))" } ?? "Not completed"
        let simulatedLine = summary.captureSource == .simulated ? "\n\(DoctorReportGenerator.simulatedBanner)\n" : ""

        let reportText = """
        EARLYGAZE OBSERVATION DIARY: SESSION SUMMARY
        \(simulatedLine)
        Child: \(childName) (\(participantToken))
        Age group: \(profile.ageBracket.developmentalStage) (\(profile.ageBracket.rawValue))
        Date: \(dateStr)
        Session ID: \(summary.sessionId)
        Duration: \(String(format: "%.1f", summary.durationSec))s (\(summary.validFrames)/\(summary.totalFrames) usable samples)
        Recording quality: \(summary.qualityVerdict.rawValue.uppercased()) (\(String(format: "%.1f%%", summary.validRatio * 100)) usable)

        WHERE YOUR CHILD LOOKED:
        • Social preference index: \(String(format: "%+.2f", summary.socialPreferenceScore)) (-1.0 shapes to +1.0 people)
        • Time on people: \(String(format: "%.2f", summary.socialDwellSec))s (\(String(format: "%.1f%%", socialDwellRatio * 100)))
        • Time on shapes: \(String(format: "%.2f", summary.geometricDwellSec))s (\(String(format: "%.1f%%", geomDwellRatio * 100)))
        • Looking at eyes: \(Self.percentText(summary.eyeDwellRatio))
        • Looking at mouth: \(Self.percentText(summary.mouthDwellRatio))
        • Eye to mouth ratio: \(eyeMouthRatioStr)
        • First look at eyes: \(summary.firstEyeLatencyMs.map { String(format: "%.0f ms", $0) } ?? "Not observed")

        FACIAL EXPRESSION:
        • Mean smile: \(String(format: "%.1f%%", summary.socialSmileIntensityMean * 100))
        • Smile reactivity: \(String(format: "%.2f", summary.smileReactivityIndex))
        • Neutral expression score: \(String(format: "%.2f", summary.flatAffectScore))
        • Mean surprise: \(String(format: "%.1f%%", summary.surpriseScoreMean * 100))

        PRIVACY AND ROUTINE:
        • No video or photos were stored
        • \(mchatLine)
        • Care routine completion: \(String(format: "%.0f%%", carePlanStore.completionRatio * 100)) (\(carePlanStore.streakDays)-day streak)
        • Care plan: \(carePlanStore.tasks.count) active tasks, \(carePlanStore.medications.count) medications logged

        ATTACHMENTS:
        \(attachmentLines.joined(separator: "\n"))

        \(ClaimsCopy.notDiagnostic)
        \(ClaimsCopy.mission)
        """
        items.append(reportText)

        shareItems = items
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
        if let popover = controller.popoverPresentationController {
            popover.permittedArrowDirections = []
        }
        return controller
    }

    public func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

// MARK: - Debug data viewer (Debug builds only)

#if DEBUG
public struct ResearchKitExportSheet: View {
    public let exportJsonText: String
    @Environment(\.dismiss) private var dismiss

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

                    // CNS-9: no study configured means nothing to send.
                    if let syncService = ResearchKitSyncService.shared {
                        ResearchKitDebugSyncControls(syncService: syncService)
                    }
                }
                .padding(16)
            }
            .navigationTitle("Data File (Debug)")
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

/// Debug-only send controls, present only when a study is configured.
private struct ResearchKitDebugSyncControls: View {
    @ObservedObject var syncService: ResearchKitSyncService

    var body: some View {
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
                    Text("Send Data File (Debug)")
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
}
#endif
