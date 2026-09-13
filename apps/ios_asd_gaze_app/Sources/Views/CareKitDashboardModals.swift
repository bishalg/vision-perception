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
                                Text("Clinical Advisory & Protocol")
                                    .font(.system(size: 18, weight: .bold, design: .rounded))
                                    .foregroundColor(.primary)
                                Text("Pediatric Visual Attention Screening")
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

                            Text("• Share weekly CareKit JSON / PDF export with your child's pediatrician or developmental specialist.\n• Maintain regular daily screenings (30-60 seconds) in a well-lit room.\n• Track behavioral observations and spontaneous eye contact ratings consistently.")
                                .font(.system(size: 13))
                                .foregroundColor(.secondary)
                        }
                        .glassCard(cornerRadius: 14, padding: 16)
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Clinical Advisory")
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

// MARK: - Export Modal View

public struct CareKitExportModalView: View {
    @ObservedObject public var store: CarePlanStore
    @Environment(\.dismiss) private var dismiss

    public init(store: CarePlanStore) {
        self.store = store
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        Text("CareKit Pediatric Export Payload")
                            .font(.system(size: 18, weight: .bold))
                            .foregroundColor(.primary)

                        Text("Format complies with Apple CareKit OCKCarePlanActivity & ResearchKit schemas for pediatrician review.")
                            .font(.system(size: 13))
                            .foregroundColor(.secondary)

                        // Payload Viewer
                        ScrollView(.horizontal) {
                            Text(store.exportClinicalCareKitJSON())
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundColor(LiquidGlass.emeraldMint)
                                .padding(12)
                                .background(Color.black.opacity(0.6)) // theme:fixed-media
                                .cornerRadius(8)
                        }

                        ShareLink(
                            item: store.exportClinicalCareKitJSON(),
                            preview: SharePreview("CareKit Clinical Report", image: Image(systemName: "doc.text"))
                        ) {
                            HStack {
                                Spacer()
                                Label("Share Clinical Report", systemImage: "square.and.arrow.up")
                                    .font(.system(size: 15, weight: .semibold))
                                Spacer()
                            }
                            .padding(.vertical, 12)
                            .background(LiquidGlass.celestialTeal)
                            .foregroundColor(.black) // theme:fixed-media
                            .cornerRadius(10)
                        }
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Clinical Export")
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

// MARK: - Milestone Detail Modal View

public struct MilestoneDetailModalView: View {
    public let milestone: MilestoneItem
    @Environment(\.dismiss) private var dismiss

    public init(milestone: MilestoneItem) {
        self.milestone = milestone
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        HStack {
                            Text(milestone.title)
                                .font(.system(size: 20, weight: .bold, design: .rounded))
                                .foregroundColor(.primary)
                            Spacer()
                            Text(milestone.dueDateString)
                                .font(.system(size: 12))
                                .foregroundColor(milestone.isCompleted ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                        }

                        Text("CDC / AAP Early Child Behavioral Checkpoints")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(.secondary)

                        VStack(alignment: .leading, spacing: 12) {
                            ForEach(milestone.checklist, id: \.self) { item in
                                HStack(alignment: .top, spacing: 10) {
                                    Image(systemName: milestone.isCompleted ? "checkmark.circle.fill" : "circle")
                                        .foregroundColor(milestone.isCompleted ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal)
                                        .font(.system(size: 16))

                                    Text(item)
                                        .font(.system(size: 13))
                                        .foregroundColor(.primary)
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                                .padding(.vertical, 4)
                            }
                        }
                        .glassCard(cornerRadius: 14, padding: 14)
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Milestone Detail")
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

// MARK: - Developmental Milestones Section View

public struct DevelopmentalMilestonesSectionView: View {
    public let milestones: [MilestoneItem]
    public let onSelect: (MilestoneItem) -> Void

    public init(milestones: [MilestoneItem], onSelect: @escaping (MilestoneItem) -> Void) {
        self.milestones = milestones
        self.onSelect = onSelect
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(Localizer.tr("carekit_milestones_title"))
                .font(.system(size: 18, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            VStack(spacing: 8) {
                ForEach(milestones) { milestone in
                    Button {
                        onSelect(milestone)
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: milestone.isCompleted ? "checkmark.circle.fill" : "circle.dashed")
                                .foregroundColor(milestone.isCompleted ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                                .font(.system(size: 20))

                            VStack(alignment: .leading, spacing: 2) {
                                Text(milestone.title)
                                    .font(.system(size: 14, weight: .semibold))
                                    .foregroundColor(.primary)

                                Text(milestone.dueDateString)
                                    .font(.system(size: 12))
                                    .foregroundColor(.secondary)
                            }

                            Spacer()

                            Image(systemName: "chevron.right")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundColor(.secondary.opacity(0.5))
                        }
                        .glassCard(cornerRadius: 12, padding: 12)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
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

// MARK: - ResearchKit Clinical Study Dashboard Card

public struct ResearchStudyDashboardCard: View {
    @ObservedObject private var syncService = ResearchKitSyncService.shared
    @State private var enrollment: ResearchStudyEnrollment = ResearchKitStudyStore.shared.fetchEnrollment()
    @State private var showConsentSheet: Bool = false
    @State private var showSurveySheet: Bool = false
    @State private var showExportSheet: Bool = false

    public init() {}

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
                        Text("Enrolled • IRB #2026-088")
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
                Text("Pediatric Clinical Research Study")
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)

                Text(enrollment.isConsented
                    ? "Participant \(enrollment.participantId) • Data streams are de-identified and encrypted."
                    : "Participate in clinical research to advance toddler neurodevelopment screening.")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }

            // Sync Status Row
            if enrollment.isConsented {
                HStack(spacing: 8) {
                    Image(systemName: syncService.syncState.isSyncing ? "arrow.triangle.2.circlepath" : "icloud.and.arrow.up")
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .font(.system(size: 12))

                    Text(syncService.syncState.description)
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)

                    Spacer()

                    if syncService.pendingQueueCount > 0 {
                        Text("\(syncService.pendingQueueCount) Pending")
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                            .foregroundColor(LiquidGlass.warmAmber)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(RoundedRectangle(cornerRadius: 8).fill(LiquidGlass.cardBackground))
            }

            // Action Buttons
            HStack(spacing: 8) {
                if !enrollment.isConsented {
                    Button {
                        showConsentSheet = true
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "signature")
                            Text("e-Consent")
                                .fontWeight(.bold)
                        }
                        .font(.system(size: 12))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(LiquidGlass.celestialTeal))
                        .foregroundColor(.black) // theme:fixed-media
                    }
                    .buttonStyle(.plain)
                } else {
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
        .sheet(isPresented: $showConsentSheet) {
            ResearchStudyConsentView { updated in
                enrollment = updated
            }
        }
        .sheet(isPresented: $showSurveySheet) {
            MCHATSurveyModalView()
        }
        .sheet(isPresented: $showExportSheet) {
            ResearchKitDoctorExportModal()
        }
        .onAppear {
            enrollment = ResearchKitStudyStore.shared.fetchEnrollment()
        }
    }
}

// MARK: - ResearchKit Doctor Export Modal

public struct ResearchKitDoctorExportModal: View {
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var syncService = ResearchKitSyncService.shared
    @State private var jsonString: String = ""
    @State private var shareURL: URL?

    public init() {}

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
                jsonString = ResearchKitStudyStore.shared.exportResearchKitJSON()
                shareURL = syncService.generateDoctorShareFile()
            }
        }
    }

    private var metadataCard: some View {
        let enrollment = ResearchKitStudyStore.shared.fetchEnrollment()
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("IRB-ASD-2026-088", systemImage: "checkmark.seal.fill")
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
                    Text("Transmit to Research Institute Server")
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
                    HStack {
                        Image(systemName: "square.and.arrow.up")
                        Text("Export Bundle for Pediatric Specialist")
                            .fontWeight(.semibold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(LiquidGlass.cardBackground)
                    .foregroundColor(LiquidGlass.emeraldMint)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }
            }
        }
    }
}
