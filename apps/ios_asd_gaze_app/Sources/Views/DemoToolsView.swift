#if ASD_DEMO_TOOLS
import SwiftUI
import CurvAsdGazeKit

/// Hidden diagnostic and demo control panel for presentation and testing (WP-A.1, H-3).
///
/// This view is compiled strictly under `ASD_DEMO_TOOLS` (present in Debug and DemoRelease
/// configurations; strictly omitted from production Release builds and audited by
/// `scripts/check_release_symbols.sh`).
public struct DemoToolsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var replaySessionActive = false
    @State private var syntheticSummary: GazeSessionReportSummary?
    @State private var syntheticFrames: [GazeFrame] = []
    @State private var isDemoServerEnabled: Bool = UserDefaults.standard.bool(forKey: "asd_demo_server_enabled")
    @State private var demoServerURL: String = UserDefaults.standard.string(forKey: "asd_demo_server_url") ?? "https://demo.curv.engineering"
    @State private var queueCount: Int = 0

    public init() {}

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
                    VStack(alignment: .leading, spacing: 20) {
                        headerSection
                        syntheticSessionSection
                        backendEndpointSection
                        syncQueueSection
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Demo Tools")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
            .fullScreenCover(isPresented: $replaySessionActive) {
                if let summary = syntheticSummary {
                    GazeSessionResultView(
                        summary: summary,
                        recordedFrames: syntheticFrames,
                        onDismiss: { replaySessionActive = false },
                        onRetake: { replaySessionActive = false }
                    )
                }
            }
            .onAppear {
                refreshQueueCount()
            }
        }
    }

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "wrench.and.screwdriver.fill")
                    .foregroundColor(LiquidGlass.warmAmber)
                Text("Presentation & Validation Mode")
                    .font(.system(size: 15, weight: .semibold, design: .rounded))
                    .foregroundColor(.primary)
            }
            Text("Simulate live session outcomes and observe cloud sync without requiring an infant volunteer on stage.")
                .font(.system(size: 13))
                .foregroundColor(.secondary)
        }
        .padding(14)
        .background(LiquidGlass.cardBackground.opacity(0.6))
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private var syntheticSessionSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("SYNTHETIC SESSIONS")
                .font(.system(size: 11, weight: .bold))
                .foregroundColor(.secondary)

            Button {
                triggerSyntheticPlayback(isTypical: true)
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Replay Typical Social Gaze Session")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(.primary)
                        Text("72% social preference, strong eye dwell (36%), optimal tracking quality.")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    Image(systemName: "play.circle.fill")
                        .font(.system(size: 24))
                        .foregroundColor(LiquidGlass.emeraldMint)
                }
                .padding(14)
                .background(LiquidGlass.cardBackground)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            }

            Button {
                triggerSyntheticPlayback(isTypical: false)
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Replay Geometric Preference Session")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(.primary)
                        Text("28% social preference, elevated geometric dwell, valid frames.")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    Image(systemName: "play.circle.fill")
                        .font(.system(size: 24))
                        .foregroundColor(LiquidGlass.warmAmber)
                }
                .padding(14)
                .background(LiquidGlass.cardBackground)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            }
        }
    }

    private var backendEndpointSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("DEMO BACKEND SERVER")
                .font(.system(size: 11, weight: .bold))
                .foregroundColor(.secondary)

            Toggle("Enable Demo Sync Tunnel", isOn: $isDemoServerEnabled)
                .font(.system(size: 15, weight: .medium))
                .onChange(of: isDemoServerEnabled) { _, newValue in
                    UserDefaults.standard.set(newValue, forKey: "asd_demo_server_enabled")
                }

            if isDemoServerEnabled {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Server Ingest Base URL")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(.secondary)

                    TextField("https://demo.curv.engineering", text: $demoServerURL)
                        .textFieldStyle(.roundedBorder)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .onChange(of: demoServerURL) { _, newValue in
                            UserDefaults.standard.set(newValue, forKey: "asd_demo_server_url")
                        }
                }
            }
        }
        .padding(14)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private var syncQueueSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("OFFLINE SYNC STATUS")
                .font(.system(size: 11, weight: .bold))
                .foregroundColor(.secondary)

            HStack {
                Text("Pending Upload Payloads")
                    .font(.system(size: 14))
                Spacer()
                Text("\(queueCount)")
                    .font(.system(size: 15, weight: .bold, design: .monospaced))
                    .foregroundColor(queueCount > 0 ? LiquidGlass.warmAmber : LiquidGlass.emeraldMint)
            }

            Button {
                ResearchKitSyncService.purgeQueuedUploads()
                refreshQueueCount()
            } label: {
                Text("Purge Outgoing Upload Queue")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(.red)
            }
            .padding(.top, 4)
        }
        .padding(14)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private func refreshQueueCount() {
        queueCount = ResearchKitSyncService.shared?.pendingQueueCount ?? 0
    }

    private func triggerSyntheticPlayback(isTypical: Bool) {
        let sid = "DEMO-" + UUID().uuidString.prefix(8)
        let socialScore = isTypical ? 0.72 : 0.28
        let summary = GazeSessionReportSummary(
            sessionId: sid,
            captureSource: .simulated,
            durationSec: 30.0,
            totalFrames: 1800,
            validFrames: 1720,
            validRatio: 0.955,
            socialPreferenceScore: socialScore,
            socialDwellSec: isTypical ? 21.6 : 8.4,
            geometricDwellSec: isTypical ? 8.4 : 21.6,
            eyeDwellRatio: isTypical ? 0.36 : 0.12,
            mouthDwellRatio: isTypical ? 0.14 : 0.08,
            firstEyeLatencyMs: isTypical ? 340.0 : 780.0,
            socialSmileIntensityMean: isTypical ? 0.45 : 0.15,
            smileReactivityIndex: isTypical ? 0.52 : 0.18,
            flatAffectScore: isTypical ? 0.12 : 0.48,
            surpriseScoreMean: isTypical ? 0.28 : 0.10,
            qualityVerdict: .optimal,
            subjectAgeSignal: nil,
            interruptionCount: 0,
            interruptedSec: 0.0
        )

        syntheticSummary = summary
        syntheticFrames = []
        replaySessionActive = true
    }
}
#endif
