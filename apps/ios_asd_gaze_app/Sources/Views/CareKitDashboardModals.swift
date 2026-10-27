import SwiftUI
import CurvAsdGazeKit

// MARK: - Clinical Details Modal Sheet

public struct ClinicalDetailsModalView: View {
    @Environment(\.dismiss) private var dismiss

    public init() {}

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        HStack(spacing: 12) {
                            ZStack {
                                Circle()
                                    .fill(LiquidGlass.warmAmber.opacity(0.2))
                                    .frame(width: 48, height: 48)
                                Image(systemName: "heart.text.square.fill")
                                    .font(.system(size: 24))
                                    .foregroundColor(LiquidGlass.warmAmber)
                            }

                            VStack(alignment: .leading, spacing: 2) {
                                Text("Observation Notes & Guidance")
                                    .font(.system(size: 18, weight: .bold, design: .rounded))
                                    .foregroundColor(.primary)
                                Text("Visual Attention Observation Diary")
                                    .font(.system(size: 13))
                                    .foregroundColor(.secondary)
                            }
                        }

                        VStack(alignment: .leading, spacing: 12) {
                            Text("Purpose & Regulatory Scope")
                                .font(.system(size: 15, weight: .bold))
                                .foregroundColor(.primary)

                            Text("This tool measures objective visual attention distributions during standardized social and geometric preference stimuli (GeoPref). It is intended as an adjunctive developmental monitoring instrument and does not provide standalone medical diagnoses.")
                                .font(.system(size: 13))
                                .foregroundColor(.secondary)

                            Divider()

                            Text("Recommended Pediatric Actions")
                                .font(.system(size: 15, weight: .bold))
                                .foregroundColor(.primary)

                            Text("• Share weekly CareKit JSON / PDF export with your child's pediatrician or developmental specialist.\n• Keep a regular daily observation (30-60 seconds) in a well-lit room.\n• Track behavioral observations and spontaneous eye contact ratings consistently.")
                                .font(.system(size: 13))
                                .foregroundColor(.secondary)
                        }
                        .glassCard(cornerRadius: 14, padding: 16)
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Observation Guidance")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}

// MARK: - Data Sharing & Privacy Hub Modal View

public struct CareKitExportModalView: View {
    @ObservedObject public var store: CarePlanStore
    @Environment(\.dismiss) private var dismiss

    @State private var includeGazeMetrics: Bool = true
    @State private var includeAffectMetrics: Bool = true
    @State private var includeDiaryNotes: Bool = true
    @State private var anonymizeIdentifiers: Bool = true
    @State private var showRawJSON: Bool = false
    @State private var isCopied: Bool = false

    public init(store: CarePlanStore) {
        self.store = store
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        headerCard
                        dataCollectionCard
                        complianceCard
                        sharingTogglesCard
                        exportActionsCard
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Data Sharing & Privacy")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") { dismiss() }
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }

    private var headerCard: some View {
        HStack(spacing: 12) {
            ZStack {
                Circle()
                    .fill(LiquidGlass.celestialTeal.opacity(0.18))
                    .frame(width: 44, height: 44)
                Image(systemName: "lock.shield.fill")
                    .font(.system(size: 22))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text("Data Sharing & Privacy Hub")
                    .font(.system(size: 16, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)
                Text("Transparency, compliance, and pediatric health data controls")
                    .font(.system(size: 11))
                    .foregroundColor(LiquidGlass.textSecondary)
            }
        }
    }

    private var dataCollectionCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            GlassSectionHeader(title: "What Data Is Collected", icon: "doc.text.magnifyingglass")
            VStack(alignment: .leading, spacing: 8) {
                privacyBullet(icon: "eye.fill", color: LiquidGlass.emeraldMint, title: "Visual Gaze & Attention", text: "Screen X/Y points, dwell times on social vs geometric scenes, and fixation stability.")
                GlassDivider()
                privacyBullet(icon: "face.smiling.fill", color: LiquidGlass.celestialTeal, title: "Facial Affect Dynamics", text: "Social smile intensity and expression metrics processed on-device in RAM.")
                GlassDivider()
                privacyBullet(icon: "book.closed.fill", color: LiquidGlass.warmAmber, title: "Caregiver Observations", text: "Daily mood ratings, joint-attention counts, and developmental milestones.")
                GlassDivider()
                privacyBullet(icon: "video.slash.fill", color: LiquidGlass.emeraldMint, title: "Zero Video Recording", text: "Raw camera frames are processed in volatile RAM and NEVER stored or uploaded.")
            }
            .glassCard(cornerRadius: 14, padding: 12)
        }
    }

    private var complianceCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            GlassSectionHeader(title: "Compliance & Security", icon: "checkmark.seal.fill")
            VStack(alignment: .leading, spacing: 8) {
                privacyBullet(icon: "shield.lefthalf.filled", color: LiquidGlass.emeraldMint, title: "HIPAA & GDPR Standards", text: "Pediatric de-identification replaces direct IDs with anonymized tokens.")
                GlassDivider()
                privacyBullet(icon: "key.fill", color: LiquidGlass.celestialTeal, title: "AES-256 On-Device Vault", text: "Encrypted with keys managed by Apple Secure Enclave.")
                GlassDivider()
                privacyBullet(icon: "person.badge.shield.checkmark.fill", color: LiquidGlass.warmAmber, title: "100% Caregiver Ownership", text: "You control all data. Export or delete records anytime.")
            }
            .glassCard(cornerRadius: 14, padding: 12)
        }
    }

    private var sharingTogglesCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            GlassSectionHeader(title: "Sharing Configuration", icon: "slider.horizontal.3")
            VStack(spacing: 8) {
                toggleRow(title: "Include Gaze & Attention Telemetry", isOn: $includeGazeMetrics)
                GlassDivider()
                toggleRow(title: "Include Facial Affect Dynamics", isOn: $includeAffectMetrics)
                GlassDivider()
                toggleRow(title: "Include Caregiver Diary Notes", isOn: $includeDiaryNotes)
                GlassDivider()
                toggleRow(title: "Anonymize Participant Token", isOn: $anonymizeIdentifiers)
            }
            .glassCard(cornerRadius: 14, padding: 12)
        }
    }

    private var exportActionsCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: "Self-Share & Export", icon: "square.and.arrow.up")
            VStack(spacing: 10) {
                ShareLink(
                    item: store.exportClinicalCareKitJSON(),
                    preview: SharePreview("ASD Gaze Clinical Report", image: Image(systemName: "doc.text.fill"))
                ) {
                    HStack {
                        Spacer()
                        Label("Share Health Report (PDF/JSON)", systemImage: "square.and.arrow.up.fill")
                            .font(.system(size: 14, weight: .semibold))
                        Spacer()
                    }
                    .padding(.vertical, 12)
                    .background(LiquidGlass.celestialTeal)
                    .foregroundColor(.black)
                    .cornerRadius(10)
                }

                HStack(spacing: 8) {
                    Button {
                        UIPasteboard.general.string = store.exportClinicalCareKitJSON()
                        isCopied = true
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) { isCopied = false }
                    } label: {
                        Label(isCopied ? "Copied" : "Copy JSON", systemImage: isCopied ? "checkmark.circle.fill" : "doc.on.doc")
                            .font(.system(size: 12, weight: .medium))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                            .background(LiquidGlass.cardBackground)
                            .overlay(RoundedRectangle(cornerRadius: 8).stroke(LiquidGlass.cardBorder, lineWidth: 1))
                            .foregroundColor(isCopied ? LiquidGlass.emeraldMint : LiquidGlass.textPrimary)
                    }

                    Button {
                        withAnimation { showRawJSON.toggle() }
                    } label: {
                        Label(showRawJSON ? "Hide JSON" : "View JSON", systemImage: showRawJSON ? "chevron.up" : "curlybraces")
                            .font(.system(size: 12, weight: .medium))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                            .background(LiquidGlass.cardBackground)
                            .overlay(RoundedRectangle(cornerRadius: 8).stroke(LiquidGlass.cardBorder, lineWidth: 1))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }
                }

                if showRawJSON {
                    ScrollView(.horizontal) {
                        Text(store.exportClinicalCareKitJSON())
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundColor(LiquidGlass.emeraldMint)
                            .padding(10)
                            .background(Color.black.opacity(0.65))
                            .cornerRadius(8)
                    }
                }
            }
            .glassCard(cornerRadius: 14, padding: 12)
        }
    }

    private func privacyBullet(icon: String, color: Color, title: String, text: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: icon)
                .font(.system(size: 15))
                .foregroundColor(color)
                .frame(width: 20)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(LiquidGlass.textPrimary)
                Text(text)
                    .font(.system(size: 11))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func toggleRow(title: String, isOn: Binding<Bool>) -> some View {
        Toggle(title, isOn: isOn)
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(LiquidGlass.textPrimary)
            .tint(LiquidGlass.celestialTeal)
    }
}

// MARK: - Clinical Advisory Banner View

public struct ClinicalAdvisoryBannerView: View {
    public let onDismiss: () -> Void
    public let onMoreDetails: () -> Void

    public init(onDismiss: @escaping () -> Void, onMoreDetails: @escaping () -> Void) {
        self.onDismiss = onDismiss
        self.onMoreDetails = onMoreDetails
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            // Alert Header Row
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(LiquidGlass.warmAmber)
                    .font(.system(size: 12))

                Text(Localizer.tr("dashboard_clinical_advisory"))
                    .font(.system(size: 11, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.warmAmber)

                Spacer()

                Text("13 Sep")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.secondary)

                Button {
                    onDismiss()
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 16))
                        .foregroundColor(.secondary.opacity(0.8))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Dismiss Alert")
            }

            // Body: Pediatric heart/clinical icon + Title + Description
            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    Circle()
                        .fill(LiquidGlass.warmAmber.opacity(0.2))
                        .frame(width: 40, height: 40)
                    Image(systemName: "heart.text.square.fill")
                        .font(.system(size: 20))
                        .foregroundColor(LiquidGlass.warmAmber)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text(Localizer.tr("dashboard_clinical_title"))
                        .font(.system(size: 14, weight: .bold))
                        .foregroundColor(.primary)

                    Text(Localizer.tr("dashboard_clinical_desc"))
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            // More Details Action Button
            HStack {
                Button {
                    onMoreDetails()
                } label: {
                    Text(Localizer.tr("dashboard_more_details"))
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.black) // theme:fixed-media
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(Capsule().fill(LiquidGlass.warmAmber))
                }
                .buttonStyle(.plain)

                Spacer()
            }
            .padding(.top, 2)
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(LiquidGlass.warmAmber.opacity(0.12))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(LiquidGlass.warmAmber.opacity(0.35), lineWidth: 1)
        )
    }
}

// MARK: - Research sync status

struct SyncStatusView: View {
    @ObservedObject var syncService: ResearchKitSyncService

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: syncService.syncState.isSyncing ? "arrow.triangle.2.circlepath" : "icloud.and.arrow.up")
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .font(.system(size: 12))

                Text(syncService.syncState.description)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(.secondary)
                    .lineLimit(2)

                Spacer(minLength: 8)

                if syncService.pendingQueueCount > 0 {
                    Text("\(syncService.pendingQueueCount) Pending")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .foregroundColor(LiquidGlass.warmAmber)
                }
            }

            if let lastSyncedAt = syncService.lastSyncedAt {
                Text("Last success \(lastSyncedAt.formatted(date: .omitted, time: .shortened))")
                    .font(.system(size: 10, weight: .medium))
                    .foregroundColor(LiquidGlass.textTertiary)
            }

            if syncService.deadLetterCount > 0 {
                HStack(spacing: 8) {
                    Text("\(syncService.deadLetterCount) stuck")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .foregroundColor(LiquidGlass.signalAtypical)
                    Spacer()
                    Button("Retry stuck") {
                        syncService.retryDeadLetterItems()
                        syncService.syncNow(forceBundleCompile: false)
                    }
                    .font(.system(size: 11, weight: .bold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .disabled(syncService.syncState.isSyncing)
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(RoundedRectangle(cornerRadius: 8).fill(LiquidGlass.cardBackground))
    }
}

// MARK: - Research study dashboard card

/// Shown only when this build has a configured study (CNS-9): callers obtain
/// the service from `ResearchKitSyncService.shared`, which is `nil` otherwise.
public struct ResearchStudyDashboardCard: View {
    /// H3 gate: false unless the build is licensed for M-CHAT-R/F.
    static var offersMCHATEntry: Bool { MCHATAvailability.canStartQuestionnaire }

    @ObservedObject private var syncService: ResearchKitSyncService
    @State private var enrollment: ResearchStudyEnrollment = ResearchKitStudyStore.shared.fetchEnrollment()
    @State private var showConsentSheet: Bool = false
    @State private var showSurveySheet: Bool = false
    @State private var showHistorySheet: Bool = false
    @State private var showExportSheet: Bool = false

    public init(syncService: ResearchKitSyncService) {
        self.syncService = syncService
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header: Apple ResearchKit badge + Enrollment status
            HStack {
                Label("Apple ResearchKit", systemImage: "waveform.path.ecg.rectangle.fill")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundColor(LiquidGlass.celestialTeal)

                Spacer()

                if enrollment.isConsented {
                    HStack(spacing: 4) {
                        Circle().fill(LiquidGlass.emeraldMint).frame(width: 6, height: 6)
                        Text("Sharing on • Optional")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundColor(LiquidGlass.emeraldMint)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(LiquidGlass.emeraldMint.opacity(0.12)))
                } else {
                    Text("Consent Required")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundColor(LiquidGlass.warmAmber)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(Capsule().fill(LiquidGlass.warmAmber.opacity(0.15)))
                }
            }

            VStack(alignment: .leading, spacing: 4) {
                Text("Optional Data Sharing")
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)

                Text(enrollment.isConsented
                    ? "Participant \(enrollment.participantId) • Data streams are de-identified and encrypted."
                    : "Optionally share de-identified observation data. This is not a diagnosis or a medical assessment.")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }

            if enrollment.isConsented {
                SyncStatusView(syncService: syncService)
            }

            // Action Buttons
            HStack(spacing: 8) {
                if !enrollment.isConsented {
                    Button {
                        showConsentSheet = true
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "signature")
                            Text("Review consent")
                                .fontWeight(.bold)
                        }
                        .font(.system(size: 12))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(LiquidGlass.celestialTeal))
                        .foregroundColor(.black) // theme:fixed-media
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("research_card_consent")
                } else {
                    if Self.offersMCHATEntry {
                    Button {
                        showSurveySheet = true
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "list.clipboard.fill")
                            Text("M-CHAT Survey")
                                .fontWeight(.bold)
                        }
                        .font(.system(size: 12))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(Capsule().fill(LiquidGlass.cardBackground))
                        .foregroundColor(LiquidGlass.celestialTeal)
                    }
                    .buttonStyle(.plain)
                    }

                    Button {
                        syncService.syncNow()
                    } label: {
                        HStack(spacing: 4) {
                            if syncService.syncState.isSyncing {
                                ProgressView().scaleEffect(0.6).tint(.black)
                            } else {
                                Image(systemName: "arrow.up.circle.fill")
                            }
                            Text("Transmit")
                                .fontWeight(.bold)
                        }
                        .font(.system(size: 12))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(Capsule().fill(LiquidGlass.emeraldMint))
                        .foregroundColor(.black) // theme:fixed-media
                    }
                    .buttonStyle(.plain)
                    .disabled(syncService.syncState.isSyncing)
                }

                Spacer()

                Button {
                    showHistorySheet = true
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "clock.arrow.circlepath")
                        Text("History")
                            .fontWeight(.medium)
                    }
                    .font(.system(size: 12))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
                .buttonStyle(.plain)

                Button {
                    showExportSheet = true
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "doc.text.magnifyingglass")
                        Text("Study Vault")
                            .fontWeight(.medium)
                    }
                    .font(.system(size: 12))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(LiquidGlass.celestialTeal.opacity(0.08))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(LiquidGlass.celestialTeal.opacity(0.25), lineWidth: 1)
        )
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("dashboard_research_card")
        .sheet(isPresented: $showConsentSheet) {
            ResearchStudyConsentView(configuration: syncService.configuration) { updated in
                enrollment = updated
            }
        }
        .sheet(isPresented: $showSurveySheet) {
            #if ASD_MCHAT_LICENSED
            MCHATSurveyModalView()
            #else
            EmptyView()
            #endif
        }
        .sheet(isPresented: $showHistorySheet) {
            MCHATHistoryView()
        }
        .sheet(isPresented: $showExportSheet) {
            ResearchKitDoctorExportModal(syncService: syncService)
        }
        .onAppear {
            enrollment = ResearchKitStudyStore.shared.fetchEnrollment()
        }
    }
}

// MARK: - ResearchKit Doctor Export Modal

public struct ResearchKitDoctorExportModal: View {
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var syncService: ResearchKitSyncService
    @State private var jsonString: String = ""
    @State private var shareURL: URL?
    @State private var pdfShareURL: URL?

    public init(syncService: ResearchKitSyncService) {
        self.syncService = syncService
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        metadataCard
                        jsonPreviewCard
                        actionButtons
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Research Study Vault")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
            .onAppear {
                let diary = MoodDiaryStore.shared.recentEntries.map { DiaryObservationRecord(entry: $0) }
                jsonString = ResearchKitStudyStore.shared.exportResearchKitJSON(diaryObservations: diary)
                shareURL = syncService.generateDoctorShareFile()
                pdfShareURL = syncService.generateDoctorReportPDF()
            }
        }
    }

    private var metadataCard: some View {
        let enrollment = ResearchKitStudyStore.shared.fetchEnrollment()
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Observation record", systemImage: "checkmark.seal.fill")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                Spacer()
                Text("Schema v1.4")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundColor(.secondary)
            }

            Text("Participant: \(enrollment.participantId)")
                .font(.system(size: 14, weight: .bold, design: .monospaced))
                .foregroundColor(.primary)

            Text("Conforms to Apple ResearchKit ORKTaskResult and ORKActiveStepResult schemas for specialist pediatric review.")
                .font(.system(size: 12))
                .foregroundColor(.secondary)
        }
        .glassCard(cornerRadius: 14, padding: 14)
    }

    private var jsonPreviewCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("ResearchKit Study Package Payload")
                .font(.system(size: 12, weight: .bold))
                .foregroundColor(.secondary)

            ScrollView {
                Text(jsonString)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundColor(.green)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
            }
            .frame(maxHeight: 260)
            .background(Color.black.opacity(0.85)) // theme:fixed-media
            .cornerRadius(12)
        }
    }

    private var actionButtons: some View {
        VStack(spacing: 10) {
            Button {
                syncService.syncNow()
            } label: {
                HStack {
                    if syncService.syncState.isSyncing {
                        ProgressView().tint(.black)
                    } else {
                        Image(systemName: "icloud.and.arrow.up.fill")
                    }
                    Text("Send to \(syncService.configuration.institution)")
                        .fontWeight(.bold)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(LiquidGlass.celestialTeal)
                .foregroundColor(.black) // theme:fixed-media
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(syncService.syncState.isSyncing)

            if let shareURL {
                ShareLink(item: shareURL) {
                    Label("Export Bundle for Pediatric Specialist", systemImage: "square.and.arrow.up")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                        .background(LiquidGlass.cardBackground)
                        .foregroundColor(LiquidGlass.emeraldMint)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                }
            }

            if let pdfShareURL {
                ShareLink(item: pdfShareURL) {
                    Label("Export Doctor Report (PDF)", systemImage: "doc.text.fill")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                        .background(LiquidGlass.cardBackground)
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                }
            }
        }
    }
}
