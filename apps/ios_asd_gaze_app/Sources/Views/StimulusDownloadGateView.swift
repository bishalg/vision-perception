import SwiftUI

/// Interstitial gate view in the screening wizard that verifies age-specific
/// and language-specific video stimuli are ready locally before the 60 Hz gaze timer starts.
///
/// Ensures 0% network stall or desynchronization during live eye tracking.
public struct StimulusDownloadGateView: View {
    @ObservedObject private var stimulusManager = OnDemandStimulusManager.shared
    @ObservedObject private var profileStore = ChildProfileStore.shared

    public let childAgeMonths: Int
    public let localeCode: String
    public let onReady: () -> Void
    public let onFallback: () -> Void

    public init(
        childAgeMonths: Int = 24,
        localeCode: String = "en",
        onReady: @escaping () -> Void,
        onFallback: @escaping () -> Void
    ) {
        self.childAgeMonths = childAgeMonths
        self.localeCode = localeCode
        self.onReady = onReady
        self.onFallback = onFallback
    }

    public var body: some View {
        ZStack {
            LinearGradient(
                colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                Spacer()

                // Center Hologram & Title Stack
                VStack(spacing: 20) {
                    // Animated Activity Hologram Icon
                    ZStack {
                        Circle()
                            .fill(LiquidGlass.celestialTeal.opacity(0.12))
                            .frame(width: 100, height: 100)
                        Circle()
                            .stroke(LiquidGlass.celestialTeal.opacity(0.35), lineWidth: 1.5)
                            .frame(width: 100, height: 100)

                        Image(systemName: "sparkles.tv")
                            .font(.system(size: 42, weight: .semibold))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }

                    // Header & Unclipped Subtitle
                    VStack(spacing: 8) {
                        Text("Calibrating Visual Activity")
                            .font(.system(size: 22, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textHeadline)

                        Text("Preparing dynamic visual battery for \(childAgeMonths) months")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(LiquidGlass.textSecondary)
                            .multilineTextAlignment(.center)
                            .lineLimit(nil)
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.horizontal, 20)
                    }

                    StimulusClipChecklist(
                        clips: sessionClips,
                        progress: stimulusManager.downloadProgress,
                        isDownloading: stimulusManager.isDownloading
                    )
                    .frame(maxWidth: 340)

                    // Session Duration & Battery Configuration Card
                    VStack(spacing: 8) {
                        Text("Observation Session Duration")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundColor(LiquidGlass.textSecondary)

                        HStack(spacing: 10) {
                            Button {
                                profileStore.profile.sessionDurationSec = 30
                            } label: {
                                VStack(spacing: 3) {
                                    Text("30 Seconds")
                                        .font(.system(size: 13, weight: .bold, design: .rounded))
                                    Text("4 Video Clips")
                                        .font(.system(size: 10, weight: .medium))
                                }
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 8)
                                .background(
                                    profileStore.profile.sessionDurationSec == 30
                                        ? LiquidGlass.celestialTeal.opacity(0.25)
                                        : Color.white.opacity(0.06)
                                )
                                .foregroundColor(
                                    profileStore.profile.sessionDurationSec == 30
                                        ? LiquidGlass.celestialTeal
                                        : Color.white.opacity(0.7)
                                )
                                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                                        .stroke(
                                            profileStore.profile.sessionDurationSec == 30
                                                ? LiquidGlass.celestialTeal
                                                : Color.white.opacity(0.12),
                                            lineWidth: 1
                                        )
                                )
                            }
                            .buttonStyle(.plain)

                            Button {
                                profileStore.profile.sessionDurationSec = 60
                            } label: {
                                VStack(spacing: 3) {
                                    Text("1 Minute (60s)")
                                        .font(.system(size: 13, weight: .bold, design: .rounded))
                                    Text("8 Video Clips")
                                        .font(.system(size: 10, weight: .medium))
                                }
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 8)
                                .background(
                                    profileStore.profile.sessionDurationSec == 60
                                        ? LiquidGlass.celestialTeal.opacity(0.25)
                                        : Color.white.opacity(0.06)
                                )
                                .foregroundColor(
                                    profileStore.profile.sessionDurationSec == 60
                                        ? LiquidGlass.celestialTeal
                                        : Color.white.opacity(0.7)
                                )
                                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                                        .stroke(
                                            profileStore.profile.sessionDurationSec == 60
                                                ? LiquidGlass.celestialTeal
                                                : Color.white.opacity(0.12),
                                            lineWidth: 1
                                        )
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .glassCard(cornerRadius: 16, padding: 12)
                    .frame(maxWidth: 320)
                }

                Spacer()

                // Bottom-Anchored Primary Call-to-Action
                Button {
                    if allSessionClipsOnDevice {
                        onReady()
                    } else {
                        onFallback()
                    }
                } label: {
                    HStack(spacing: 8) {
                        Text(allSessionClipsOnDevice ? "Begin Observation Activity" : "Continue with on-screen activity")
                            .font(.system(size: 16, weight: .bold, design: .rounded))
                        Image(systemName: "arrow.right.circle.fill")
                            .font(.system(size: 18))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 15)
                    .background(
                        LinearGradient(
                            colors: [LiquidGlass.celestialTeal, LiquidGlass.emeraldMint],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                    )
                    .foregroundColor(.black)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .shadow(color: LiquidGlass.emeraldMint.opacity(0.35), radius: 10, y: 4)
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 28)
                .disabled(stimulusManager.isDownloading)
                .opacity(stimulusManager.isDownloading ? 0.5 : 1)
            }
        }
        .task(id: profileStore.profile.sessionDurationSec) {
            _ = await stimulusManager.preloadStimulusPack(ageMonths: childAgeMonths, localeCode: localeCode)
        }
    }

    private var sessionPlaylist: [StimulusPlaylistItem] {
        StimulusPlaylistBattery.playlist(forTargetDuration: Double(profileStore.profile.sessionDurationSec))
    }

    private var sessionClips: [StimulusClipTransfer] {
        let wanted = Set(sessionPlaylist.map(\.videoName))
        let known = stimulusManager.clips.filter { wanted.contains($0.id) }
        if !known.isEmpty { return known }
        return sessionPlaylist.map {
            StimulusClipTransfer(id: $0.videoName, title: $0.title, phase: .waiting)
        }
    }

    private var allSessionClipsOnDevice: Bool {
        !sessionClips.isEmpty && sessionClips.allSatisfy { $0.phase == .onDevice }
    }
}

struct StimulusClipChecklist: View {
    let clips: [StimulusClipTransfer]
    let progress: Double
    let isDownloading: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(isDownloading ? "Downloading clips" : "Activity clips")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(LiquidGlass.textSecondary)
                Spacer()
                Text("\(Int(progress * 100))%")
                    .font(.system(size: 12, weight: .bold, design: .monospaced))
                    .foregroundColor(LiquidGlass.emeraldMint)
            }
            ProgressView(value: progress, total: 1.0)
                .progressViewStyle(.linear)
                .tint(LiquidGlass.emeraldMint)
            ForEach(clips) { clip in
                HStack(spacing: 8) {
                    Image(systemName: symbol(for: clip.phase))
                        .font(.system(size: 12, weight: .bold))
                        .foregroundColor(tint(for: clip.phase))
                        .frame(width: 16)
                    Text(clip.title)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(LiquidGlass.textPrimary)
                        .lineLimit(1)
                    Spacer(minLength: 8)
                    Text(clip.statusLabel)
                        .font(.system(size: 11, weight: .bold, design: .monospaced))
                        .foregroundColor(tint(for: clip.phase))
                }
            }
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    private func symbol(for phase: StimulusClipTransfer.Phase) -> String {
        switch phase {
        case .onDevice: return "checkmark.circle.fill"
        case .waiting: return "clock"
        case .downloading: return "arrow.down.circle"
        case .missing: return "exclamationmark.circle"
        }
    }

    private func tint(for phase: StimulusClipTransfer.Phase) -> Color {
        switch phase {
        case .onDevice: return LiquidGlass.emeraldMint
        case .waiting, .downloading: return LiquidGlass.celestialTeal
        case .missing: return LiquidGlass.warmAmber
        }
    }
}
