import SwiftUI
import CurvAsdGazeKit

// MARK: - Session storage state (store build, S-3 / S-4)

/// Where one recorded session is, as far as the device can tell. The upload queue keeps no
/// per-session receipt the UI can read, so the state is derived from the queue counts and the
/// last successful sync: a session counts as uploaded only when the queue is empty and the
/// last sync happened after the session ended.
enum SessionStorageState: Equatable {
    /// No clinic upload is configured (the default store build, S-4).
    case onDevice
    case waiting
    case needsAttention
    case uploaded

    static func resolve(endedAt: Date, syncConfigured: Bool, summary: SyncStatusSummary?) -> SessionStorageState {
        guard syncConfigured, let summary else { return .onDevice }
        let queued = summary.waiting + summary.uploading
        if queued == 0, summary.needsAttention == 0, let last = summary.lastSyncedAt, last >= endedAt {
            return .uploaded
        }
        if queued == 0, summary.needsAttention > 0 {
            return .needsAttention
        }
        return .waiting
    }

    var text: String {
        switch self {
        case .onDevice: return "Saved on this device"
        case .waiting: return "Waiting to upload"
        case .needsAttention: return "Upload needs attention"
        case .uploaded: return "Delivered to your clinic"
        }
    }

    var systemImage: String {
        switch self {
        case .onDevice: return "iphone"
        case .waiting: return "icloud.and.arrow.up"
        case .needsAttention: return "exclamationmark.icloud"
        case .uploaded: return "checkmark.icloud"
        }
    }
}

// MARK: - Sessions Tab (store build)

/// The store build's list of recorded observation sessions (S-2): date, duration, clips,
/// recording quality and where the session is. It shows no score, flag, band, social
/// preference or affect value; the clinic reviews each session on its console.
struct StoreSessionsView: View {
    @ObservedObject private var history = GazeSessionHistoryStore.shared

    private var syncService: ResearchKitSyncService? {
        guard let service = ResearchKitSyncService.shared, service.isSessionSyncEnabled else { return nil }
        return service
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if let syncService {
                        ClinicSyncStatusLine(syncService: syncService)
                    } else {
                        Label(StoreBuild.localOnlyNotice, systemImage: "iphone")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }

                Section {
                    if history.records.isEmpty {
                        Text(Localizer.tr("store_no_sessions"))
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("store_sessions_empty")
                    } else {
                        ForEach(history.records.reversed()) { record in
                            StoreSessionRow(record: record, syncService: syncService)
                        }
                    }
                } footer: {
                    Text(StoreBuild.disclaimer)
                        .font(.footnote)
                        .padding(.top, 8)
                }
            }
            .navigationTitle(Localizer.tr("store_tab_sessions"))
        }
    }
}

/// One session: when, how long, how many clips, recording quality and storage state only.
private struct StoreSessionRow: View {
    let record: GazeSessionRecord
    let syncService: ResearchKitSyncService?

    private var storage: SessionStorageState {
        SessionStorageState.resolve(
            endedAt: record.endedAt,
            syncConfigured: syncService != nil,
            summary: syncService?.statusSummary
        )
    }

    private var durationText: String {
        let total = max(0, Int(record.durationSec.rounded()))
        return String(format: "%d:%02d", total / 60, total % 60)
    }

    private var clipsText: String {
        let count = record.perVideo?.count ?? 0
        return count == 1 ? "1 clip" : "\(count) clips"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(record.startedAt.formatted(date: .abbreviated, time: .shortened))
                .font(.headline)
            HStack(spacing: 12) {
                Label(durationText, systemImage: "clock")
                Label(clipsText, systemImage: "film")
            }
            .font(.subheadline)
            .foregroundStyle(.secondary)
            Text(String(
                format: "Recording quality: %@ · Usable samples: %.0f%%",
                record.qualityVerdict.capitalized,
                record.validFrameRatio * 100
            ))
            .font(.subheadline)
            Label(storage.text, systemImage: storage.systemImage)
                .font(.footnote)
                .foregroundStyle(storage == .needsAttention ? LiquidGlass.warmAmber : LiquidGlass.textSecondary)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("store_session_row")
    }
}
