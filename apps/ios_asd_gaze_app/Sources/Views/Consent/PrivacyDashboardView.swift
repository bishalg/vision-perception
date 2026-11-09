import SwiftUI

public enum PrivacyDashboardAccessibility {
    public static let view = "privacy_dashboard"
    public static let sessionCount = "privacy_session_count"
    public static let deliveryStatus = "privacy_delivery_status"
    public static let deliveryOff = "privacy_delivery_off"
    public static let consentRecord = "privacy_consent_record"
    public static let withdraw = "privacy_withdraw_consent"
    public static let deleteAll = "privacy_delete_all"
    public static func toggle(_ feature: FeatureOptIn) -> String { "privacy_optin_\(feature.rawValue)" }
}

/// Delete-all copy shared by Privacy & Your Data and Settings. In the store build delete-all is
/// one confirmation that also removes the consent record, opt-ins and the child profile (S-24).
public enum DeleteAllCopy {
    public static var title: String {
        ConsentL10n.text("privacy.delete.title", "Delete all data")
    }

    public static var button: String {
        ConsentL10n.text("privacy.delete.button", "Delete all data on this device")
    }

    public static var confirmTitle: String {
        ConsentL10n.text("privacy.delete.confirm.title", "Delete all data on this device?")
    }

    public static var confirm: String {
        ConsentL10n.text("privacy.delete.confirm", "Delete all data")
    }

    public static var cancel: String {
        ConsentL10n.text("privacy.cancel", "Cancel")
    }

    public static var message: String {
        #if ASD_STORE_BUILD
        ConsentL10n.text("privacy.delete.message.store", "This permanently removes saved sessions, session history, sessions waiting to be sent, calibration, your consent record, your optional feature choices and the child profile from this device. It cannot be undone.")
        #else
        ConsentL10n.text("privacy.delete.message", "This permanently removes saved sessions, session history, queued uploads and calibration from this device. It cannot be undone.")
        #endif
    }

    public static var detail: String {
        #if ASD_STORE_BUILD
        ConsentL10n.text("privacy.delete.detail.store", "Removes every session, the history, calibration, your consent record, your optional feature choices and the child profile from this device. To delete sessions already delivered, ask your clinic.")
        #else
        ConsentL10n.text("privacy.delete.detail", "Removes every session, the history and calibration from this device. To delete sessions already delivered, ask your clinic.")
        #endif
    }

    public static var done: String {
        #if ASD_STORE_BUILD
        ConsentL10n.text("privacy.delete.done.store", "All data on this device was deleted. The app will ask for consent again before the next session.")
        #else
        ConsentL10n.text("privacy.delete.done", "All observation data on this device was deleted.")
        #endif
    }

    public static var failed: String {
        ConsentL10n.text("privacy.delete.failed", "Some data could not be deleted. Please try again.")
    }

    /// Runs the delete for this build: everything in the store build (S-24), observation data
    /// only elsewhere (care plans have their own prompt in Settings).
    @MainActor
    public static func perform(_ service: DataDeletionService = .live()) -> DataDeletionService.Report {
        #if ASD_STORE_BUILD
        service.deleteEverything()
        #else
        service.deleteAll(includeCarePlans: false)
        #endif
    }
}

/// "Privacy & Your Data" (S-12): what is never collected, what is on this device, clinic
/// delivery state, one toggle per optional feature, the signed consent record, delete-all
/// and the legal links. Shown in every build.
public struct PrivacyDashboardView: View {
    @ObservedObject private var history: GazeSessionHistoryStore
    @ObservedObject private var optIns: FeatureOptIns
    @ObservedObject private var consent: ConsentRecordStore
    private let syncService: ResearchKitSyncService?

    @State private var showWithdrawConfirmation = false
    @State private var showDeleteConfirmation = false
    @State private var resultMessage: String?

    public init(
        history: GazeSessionHistoryStore = .shared,
        optIns: FeatureOptIns = .shared,
        consent: ConsentRecordStore = .shared,
        syncService: ResearchKitSyncService? = ResearchKitSyncService.shared
    ) {
        self.history = history
        self.optIns = optIns
        self.consent = consent
        self.syncService = syncService
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                neverCollectedSection
                onDeviceSection
                deliverySection
                optionalFeaturesSection
                consentSection
                deleteSection
                legalSection
                NotDiagnosticFooter()
            }
            .padding(20)
            .readableColumn()
        }
        .background(LiquidGlass.deepCanvas.ignoresSafeArea())
        .navigationTitle(ConsentL10n.text("privacy.title", "Privacy & Your Data"))
        .navigationBarTitleDisplayMode(.inline)
        // A container keeps the children's own ids (Maestro taps them, WP-S.7).
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(PrivacyDashboardAccessibility.view)
        .confirmationDialog(
            ConsentL10n.text("privacy.withdraw.confirm.title", "Withdraw consent?"),
            isPresented: $showWithdrawConfirmation,
            titleVisibility: .visible
        ) {
            Button(ConsentL10n.text("privacy.withdraw.confirm", "Withdraw consent"), role: .destructive) {
                consent.revoke()
                resultMessage = ConsentL10n.text("privacy.withdraw.done", "Consent withdrawn. The app will ask for consent again before the next session.")
            }
            Button(ConsentL10n.text("privacy.cancel", "Cancel"), role: .cancel) {}
        } message: {
            Text(ConsentL10n.text("privacy.withdraw.message", "No new sessions can start until you agree again. Sessions already on this device stay until you delete them."))
        }
        .confirmationDialog(
            DeleteAllCopy.confirmTitle,
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button(DeleteAllCopy.confirm, role: .destructive, action: deleteAll)
            Button(DeleteAllCopy.cancel, role: .cancel) {}
        } message: {
            Text(DeleteAllCopy.message)
        }
        .alert(
            ConsentL10n.text("privacy.title", "Privacy & Your Data"),
            isPresented: Binding(get: { resultMessage != nil }, set: { if !$0 { resultMessage = nil } })
        ) {
            Button(ConsentL10n.text("privacy.ok", "OK"), role: .cancel) { resultMessage = nil }
        } message: {
            Text(resultMessage ?? "")
        }
    }

    // MARK: Sections

    private var neverCollectedSection: some View {
        section(ConsentL10n.text("privacy.never.title", "What is never collected"), icon: "hand.raised") {
            bullet("video.slash", ConsentL10n.text("privacy.never.camera", "No camera images, photos or video"))
            bullet("mic.slash", ConsentL10n.text("privacy.never.audio", "No audio of a session"))
            bullet("person.crop.circle.badge.xmark", ConsentL10n.text("privacy.never.name", "Your child's name never leaves this device"))
            bullet("chart.bar.xaxis", ConsentL10n.text("privacy.never.analytics", "No analytics"))
            bullet("megaphone", ConsentL10n.text("privacy.never.ads", "No ads"))
            bullet("location.slash", ConsentL10n.text("privacy.never.tracking", "No tracking"))
        }
    }

    private var onDeviceSection: some View {
        section(ConsentL10n.text("privacy.device.title", "On this device"), icon: "iphone") {
            HStack {
                Text(ConsentL10n.text("privacy.device.sessions", "Saved sessions")).glassStyle(.body)
                Spacer()
                Text("\(history.records.count)")
                    .glassStyle(.headline)
                    .accessibilityIdentifier(PrivacyDashboardAccessibility.sessionCount)
            }
            Text(ConsentL10n.text("privacy.device.protected", "Encrypted and excluded from backups."))
                .glassStyle(.subheadline)
        }
    }

    private var deliverySection: some View {
        section(ConsentL10n.text("privacy.delivery.title", "Clinic delivery"), icon: "paperplane") {
            if let syncService, syncService.isSessionSyncEnabled {
                ClinicDeliveryStatusRow(syncService: syncService, optIns: optIns)
            } else {
                Text(ConsentL10n.text("privacy.delivery.notenrolled", "Not enrolled"))
                    .glassStyle(.headline)
                    .accessibilityIdentifier(PrivacyDashboardAccessibility.deliveryStatus)
                Text(ConsentL10n.text("privacy.delivery.local", "Sessions stay on this device until your clinic enrols it."))
                    .glassStyle(.subheadline)
            }
        }
    }

    private var optionalFeaturesSection: some View {
        section(ConsentL10n.text("privacy.optional.title", "Optional features"), icon: "switch.2") {
            Text(ConsentL10n.text("privacy.optional.intro", "Every optional feature is off until you turn it on. Sessions work with all of them off."))
                .glassStyle(.subheadline)
            // S-16: only the opt-ins whose feature exists in this build.
            let features = FeatureOptIn.available()
            ForEach(features) { feature in
                VStack(alignment: .leading, spacing: 4) {
                    Toggle(isOn: Binding(get: { optIns.isOn(feature) }, set: { setOptIn(feature, $0) })) {
                        Label(feature.title, systemImage: feature.systemImage)
                            .glassStyle(.headline)
                    }
                    .tint(LiquidGlass.emeraldMint)
                    .accessibilityIdentifier(PrivacyDashboardAccessibility.toggle(feature))
                    Text(feature.rowDescription)
                        .glassStyle(.caption)
                        .fixedSize(horizontal: false, vertical: true)
                    NavigationLink {
                        FeatureOptInDetailView(feature: feature)
                    } label: {
                        Text(ConsentL10n.text("privacy.optional.details", "Details"))
                            .font(.system(size: 13, weight: .semibold))
                    }
                    .tint(LiquidGlass.celestialTeal)
                }
                if feature != features.last { GlassDivider() }
            }
        }
    }

    private var consentSection: some View {
        section(ConsentL10n.text("privacy.consent.title", "Your consent"), icon: "signature") {
            if let record = consent.record {
                Text(String(
                    format: ConsentL10n.text("privacy.consent.summary", "Agreed by %@ (%@) on %@."),
                    record.guardianName,
                    record.relationship.displayName,
                    record.agreedAt.formatted(date: .abbreviated, time: .omitted)
                ))
                .glassStyle(.body)
                if !consent.isCurrent {
                    Text(ConsentL10n.text("privacy.consent.outdated", "The consent text has changed. The app will ask again before the next session."))
                        .glassStyle(.subheadline, color: LiquidGlass.warmAmber)
                }
            } else {
                Text(ConsentL10n.text("privacy.consent.none", "No consent recorded. The app will ask before the next session."))
                    .glassStyle(.body)
            }
            NavigationLink {
                ConsentRecordView(store: consent)
            } label: {
                Text(ConsentL10n.text("privacy.consent.view", "View consent record"))
                    .font(.system(size: 14, weight: .semibold))
            }
            .tint(LiquidGlass.celestialTeal)
            .accessibilityIdentifier(PrivacyDashboardAccessibility.consentRecord)
            if consent.record != nil {
                Button(ConsentL10n.text("privacy.withdraw", "Withdraw consent"), role: .destructive) {
                    showWithdrawConfirmation = true
                }
                .accessibilityIdentifier(PrivacyDashboardAccessibility.withdraw)
            }
        }
    }

    private var deleteSection: some View {
        section(DeleteAllCopy.title, icon: "trash") {
            Text(DeleteAllCopy.detail)
                .glassStyle(.subheadline)
                .fixedSize(horizontal: false, vertical: true)
            Button(DeleteAllCopy.button, role: .destructive) {
                showDeleteConfirmation = true
            }
            .foregroundColor(LiquidGlass.signalAtypical)
            .accessibilityIdentifier(PrivacyDashboardAccessibility.deleteAll)
        }
    }

    @ViewBuilder
    private var legalSection: some View {
        let privacyURL = StoreBuild.privacyPolicyURL()
        let supportURL = StoreBuild.supportURL()
        if privacyURL != nil || supportURL != nil {
            section(ConsentL10n.text("privacy.legal.title", "Legal"), icon: "doc.text") {
                if let privacyURL {
                    Link(ConsentL10n.text("privacy.legal.policy", "Privacy Policy"), destination: privacyURL)
                        .tint(LiquidGlass.celestialTeal)
                }
                if let supportURL {
                    Link(ConsentL10n.text("privacy.legal.support", "Support"), destination: supportURL)
                        .tint(LiquidGlass.celestialTeal)
                }
            }
        }
    }

    // MARK: Actions

    /// Records the choice and applies it at once (S-15, S-18): reminders are rescheduled or
    /// removed, and turning clinic delivery back on resumes the waiting queue.
    private func setOptIn(_ feature: FeatureOptIn, _ value: Bool) {
        optIns.set(feature, value)
        switch feature {
        case .reminders:
            #if !ASD_STORE_BUILD
            Task { await CarePlanStore.shared.resyncAlerts() }
            #endif
        case .clinicDelivery:
            if value { syncService?.requestFlush(.foreground) }
        case .ageRangeCheck, .healthKitMood, .dictation, .photoAttachments:
            break
        }
    }

    private func deleteAll() {
        let report = DeleteAllCopy.perform()
        let message = report.succeeded ? DeleteAllCopy.done : DeleteAllCopy.failed
        // Present after the confirmation dialog finishes dismissing.
        DispatchQueue.main.async { resultMessage = message }
    }

    // MARK: Layout helpers

    private func section<Content: View>(_ title: String, icon: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: title, icon: icon)
            VStack(alignment: .leading, spacing: 10) {
                content()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private func bullet(_ icon: String, _ text: String) -> some View {
        Label {
            Text(text).glassStyle(.body)
        } icon: {
            Image(systemName: icon).foregroundColor(LiquidGlass.emeraldMint)
        }
    }
}

/// Enrolment state of this device, observed live from the sync service. With the clinic
/// delivery opt-in off it says so and how many sessions wait on the device (S-19).
private struct ClinicDeliveryStatusRow: View {
    @ObservedObject var syncService: ResearchKitSyncService
    @ObservedObject var optIns: FeatureOptIns

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if !optIns.clinicDelivery {
                Text(ClinicSyncStatusLine.deliveryOffText(waiting: syncService.pendingQueueCount))
                    .glassStyle(.subheadline, color: LiquidGlass.warmAmber)
                    .accessibilityIdentifier(PrivacyDashboardAccessibility.deliveryOff)
            }
            Text(isEnrolled
                 ? ConsentL10n.text("privacy.delivery.enrolled", "Enrolled")
                 : ConsentL10n.text("privacy.delivery.notenrolled", "Not enrolled"))
                .glassStyle(.headline)
                .accessibilityIdentifier(PrivacyDashboardAccessibility.deliveryStatus)
            Text(detail).glassStyle(.subheadline)
        }
    }

    private var isEnrolled: Bool {
        if case .registered = syncService.deviceRegistration { return true }
        return false
    }

    private var detail: String {
        switch syncService.deviceRegistration {
        case .registered:
            return ConsentL10n.text("privacy.delivery.enrolled.detail", "Sessions are delivered to your clinic's console when online.")
        case .needsEnrolment:
            return ConsentL10n.text("privacy.delivery.expired", "Enrolment expired. Sessions stay on this device until your clinic gives you a new code.")
        case .unregistered, .unavailable:
            return ConsentL10n.text("privacy.delivery.local", "Sessions stay on this device until your clinic enrols it.")
        }
    }
}

/// What one optional feature does, stores and never stores, and the iOS permissions it needs.
struct FeatureOptInDetailView: View {
    let feature: FeatureOptIn
    @State private var statuses: [PermissionKind: PermissionStatus] = [:]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Label(feature.title, systemImage: feature.systemImage)
                    .font(.system(size: 22, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textHeadline)
                detailBlock(ConsentL10n.text("optin.detail.what", "What it does"), feature.rowDescription)
                detailBlock(ConsentL10n.text("optin.detail.stored", "What is stored"), feature.whatIsStored)
                detailBlock(ConsentL10n.text("optin.detail.never", "What is never stored"), feature.whatIsNeverStored)
                if let whenTurnedOff = feature.whenTurnedOff {
                    detailBlock(ConsentL10n.text("optin.detail.off", "When you turn it off"), whenTurnedOff)
                }

                VStack(alignment: .leading, spacing: 10) {
                    Text(ConsentL10n.text("optin.detail.permissions", "iOS permissions")).glassStyle(.headline)
                    if feature.requiredPermissions.isEmpty {
                        Text(ConsentL10n.text("optin.detail.nopermissions", "This feature needs no iOS permission."))
                            .glassStyle(.body)
                    } else {
                        ForEach(feature.requiredPermissions) { kind in
                            HStack {
                                Label(kind.title, systemImage: kind.systemImage).glassStyle(.body)
                                Spacer()
                                Text(statuses[kind]?.displayText ?? "")
                                    .glassStyle(.subheadline)
                            }
                            if let note = kind.statusNote {
                                Text(note).glassStyle(.caption)
                            }
                        }
                        Text(PermissionKind.camera.howToChangeLater).glassStyle(.caption)
                        Button(ConsentL10n.text("optin.detail.settings", "Open iOS Settings")) {
                            AppSettingsLink.open()
                        }
                        .tint(LiquidGlass.celestialTeal)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .glassCard(cornerRadius: 14, padding: 14)
            }
            .padding(20)
            .readableColumn()
        }
        .background(LiquidGlass.deepCanvas.ignoresSafeArea())
        .navigationTitle(feature.title)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            for kind in feature.requiredPermissions {
                statuses[kind] = await kind.currentStatus()
            }
        }
    }

    private func detailBlock(_ title: String, _ text: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).glassStyle(.headline)
            Text(text).glassStyle(.body).fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(cornerRadius: 14, padding: 14)
    }
}
