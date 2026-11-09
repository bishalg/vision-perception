import SwiftUI
import CurvAsdGazeKit
import UserNotifications
#if canImport(ARKit)
import ARKit
#endif

/// System Settings & Diagnostics view adhering to Apple's native Form / List design.
/// Directs language localization to iOS Settings and displays sensor telemetry status.
public struct SettingsView: View {
    @Environment(\.openURL) private var openURL
    @ObservedObject private var childProfileStore = ChildProfileStore.shared
    @ObservedObject private var alertPreferences = CareAlertPreferences.shared
    @ObservedObject private var healthKitMood = HealthKitMoodStore.shared
    @ObservedObject private var appMode = AppModeStore.shared
    /// S-15: reminders and Health mood follow their opt-ins.
    @ObservedObject private var optIns = FeatureOptIns.shared
    @State private var newConditionTitle: String = ""
    @State private var notificationStatusText: String = "Not determined"
    @State private var calibrationProfile: GazeCalibrationProfile = .identity
    /// CAL-1: the diagnostic calibration check is its own session for the current child.
    @State private var calibrationCheckSessionId = UUID().uuidString
    @State private var showQuickCalibrate: Bool = false
    @State private var showCameraCheck: Bool = false
    @State private var showOnboardingFlow: Bool = false
    @State private var showDeleteAllConfirmation: Bool = false
    @State private var showDeleteCarePlansPrompt: Bool = false
    @State private var deletionResultMessage: String?
    #if ASD_DEMO_TOOLS
    @State private var showDemoToolsView: Bool = false
    #endif
    #if ASD_BENCH_CAPTURE
    @State private var showBenchCapture: Bool = false
    #endif

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
                        #if ASD_DEMO_TOOLS
                        demoToolsSection
                        #endif
                        #if ASD_BENCH_CAPTURE
                        benchCaptureSection
                        #endif
                        #if ASD_STORE_BUILD
                        // S-6: mode, medical conditions and care alerts belong to the clinic platform.
                        profileSection
                        #else
                        appModeSection
                        profileSection
                        medicalConditionsSection
                        careAlertsSection
                        #endif
                        screeningToolsSection
                        languageSection
                        sensorDiagnosticsSection
                        privacySecuritySection
                        aboutSection
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 16)
                }
            }
            .navigationTitle(Localizer.tr("tab_settings"))
            .navigationBarTitleDisplayMode(.large)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .onAppear {
                calibrationProfile = BoundCalibrationStore.load(for: calibrationCheckBinding)
                refreshNotificationStatus()
            }
            .fullScreenCover(isPresented: $showQuickCalibrate) {
                // A calibration check in a session of its own (CAL-1: never applied elsewhere).
                BubbleCalibrationView(
                    onClose: {
                        showQuickCalibrate = false
                        calibrationProfile = BoundCalibrationStore.load(for: calibrationCheckBinding)
                    },
                    onCalibrationComplete: {
                        showQuickCalibrate = false
                        calibrationProfile = BoundCalibrationStore.load(for: calibrationCheckBinding)
                    }
                )
            }
            .fullScreenCover(isPresented: $showCameraCheck) {
                ZStack(alignment: .topTrailing) {
                    GazePositioningStepView(
                        onReady: { showCameraCheck = false }
                    )
                    Button {
                        showCameraCheck = false
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 24))
                            // theme:fixed-media — close glyph over the positioning camera sheet.
                            .foregroundColor(Color.white.opacity(0.8))
                            .padding(16)
                    }
                    .accessibilityLabel("Close")
                    .accessibilityIdentifier("settings_camera_check_close")
                }
            }
            .fullScreenCover(isPresented: $showOnboardingFlow) {
                OnboardingFlowView(
                    onCompleted: {
                        showOnboardingFlow = false
                    }
                )
            }
        }
    }

    // MARK: - Section 0.5: Screening & Calibration Tools

    private var screeningToolsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(
                title: Localizer.tr("settings_screening_tools_section"),
                icon: "waveform.path.ecg",
                iconColor: LiquidGlass.celestialTeal
            )

            VStack(spacing: 0) {
                // Quick Calibrate
                Button {
                    showQuickCalibrate = true
                } label: {
                    HStack(spacing: 12) {
                        ZStack {
                            Circle()
                                .fill(LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity))
                                .frame(width: 36, height: 36)
                            Image(systemName: "target")
                                .font(.system(size: 16, weight: .semibold))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }
                        VStack(alignment: .leading, spacing: 2) {
                            Text(Localizer.tr("settings_quick_calibrate"))
                                .glassStyle(.headline)
                            Text(Localizer.tr("settings_quick_calibrate_sub"))
                                .glassStyle(.caption)
                        }
                        Spacer()
                        Image(systemName: "chevron.right")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundColor(LiquidGlass.textTertiary)
                    }
                    .padding(.vertical, 12)
                }
                .buttonStyle(.plain)

                GlassDivider()

                // Camera & Distance Check
                Button {
                    showCameraCheck = true
                } label: {
                    HStack(spacing: 12) {
                        ZStack {
                            Circle()
                                .fill(LiquidGlass.warmAmber.opacity(LiquidGlass.tintOpacity))
                                .frame(width: 36, height: 36)
                            Image(systemName: "camera.viewfinder")
                                .font(.system(size: 16, weight: .semibold))
                                .foregroundColor(LiquidGlass.warmAmber)
                        }
                        VStack(alignment: .leading, spacing: 2) {
                            Text(Localizer.tr("settings_camera_check"))
                                .glassStyle(.headline)
                            Text(Localizer.tr("settings_camera_check_sub"))
                                .glassStyle(.caption)
                        }
                        Spacer()
                        Image(systemName: "chevron.right")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundColor(LiquidGlass.textTertiary)
                    }
                    .padding(.vertical, 12)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 14)
            .glassCard(cornerRadius: 14, padding: 0)
        }
    }

    // MARK: - Section 1: Participant & Caregiver Profile

    private var profileSection: some View {
        let profile = childProfileStore.profile
        let childName = profile.childName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "Liam G." : profile.childName

        return VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: "Caregiver & Participant Profile", icon: "person.crop.circle.fill")

            VStack(spacing: 12) {
                HStack(spacing: 14) {
                    ZStack {
                        Circle()
                            .fill(LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity))
                            .frame(width: 52, height: 52)
                        Circle()
                            .stroke(LiquidGlass.celestialTeal.opacity(0.4), lineWidth: 1.2)
                            .frame(width: 52, height: 52)
                        Image(systemName: "person.crop.circle.fill")
                            .font(.system(size: 38))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }

                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(childName)
                                .font(.system(size: 17, weight: .bold, design: .rounded))
                                .foregroundColor(LiquidGlass.textPrimary)
                            Text(profile.ageBracket.rawValue)
                                .font(.system(size: 11, weight: .bold, design: .rounded))
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(LiquidGlass.tintOpacity)))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }

                        Text("Cohort: \(profile.ageBracket.developmentalStage)")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundColor(LiquidGlass.textSecondary)

                        Text("Language: \(languageDisplayName(for: profile.preferredLanguage))")
                            .font(.system(size: 11, weight: .regular))
                            .foregroundColor(LiquidGlass.textTertiary)
                    }

                    Spacer()
                }

                #if !ASD_STORE_BUILD
                GlassDivider()

                healthKitMoodRow
                #endif

                #if !ASD_STORE_BUILD
                // Store build (S-6): consent state lives in Privacy & Your Data, and care
                // features (CareKit) are not part of the store build.
                GlassDivider()

                HStack {
                    Label("Privacy & PIA Protocol", systemImage: "hand.raised.shield.fill")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(LiquidGlass.textPrimary)
                    Spacer()
                    Text(profile.hasConsentedPIA ? "Verified On-Device" : "Consent Pending")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(profile.hasConsentedPIA ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                }

                GlassDivider()

                HStack {
                    Label("CareKit Sync Status", systemImage: "arrow.triangle.2.circlepath")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(LiquidGlass.textPrimary)
                    Spacer()
                    Text("Encrypted On-Device")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
                #endif

                GlassDivider()

                Button {
                    showOnboardingFlow = true
                } label: {
                    HStack {
                        Image(systemName: "slider.horizontal.3")
                            .font(.system(size: 13, weight: .semibold))
                        Text("Edit Profile & Re-run Onboarding")
                            .font(.system(size: 13, weight: .semibold, design: .rounded))
                        Spacer()
                        Image(systemName: "chevron.right")
                            .font(.system(size: 12, weight: .semibold))
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .padding(.vertical, 4)
                }
                .buttonStyle(.plain)
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private var medicalConditionsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_conditions_section"), icon: "cross.case.fill")

            VStack(alignment: .leading, spacing: 10) {
                Text("Caregiver-entered conditions and allergies for medication context. Not a diagnosis; gaze observations are not a medical assessment.")
                    .font(.system(size: 12))
                    .foregroundColor(LiquidGlass.textSecondary)

                TextField("Allergies", text: Binding(
                    get: { childProfileStore.profile.allergies },
                    set: { childProfileStore.profile.allergies = $0 }
                ))
                .textFieldStyle(.roundedBorder)

                ForEach(childProfileStore.profile.medicalConditions) { condition in
                    HStack {
                        Text(condition.title)
                        Spacer()
                        Button(role: .destructive) {
                            childProfileStore.profile.medicalConditions.removeAll { $0.id == condition.id }
                        } label: {
                            Image(systemName: "trash")
                        }
                        .buttonStyle(.plain)
                    }
                }

                HStack {
                    TextField("Add condition", text: $newConditionTitle)
                        .textFieldStyle(.roundedBorder)
                    Button("Add") {
                        let title = newConditionTitle.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !title.isEmpty else { return }
                        childProfileStore.profile.medicalConditions.append(MedicalCondition(title: title))
                        newConditionTitle = ""
                    }
                }
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private var careAlertsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_alerts_section"), icon: "bell.badge.fill")

            VStack(alignment: .leading, spacing: 12) {
                if !optIns.reminders {
                    // S-15: reminders run only while their opt-in is on.
                    Text(Localizer.tr("settings_alerts_optin_off"))
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(LiquidGlass.warmAmber)
                        .accessibilityIdentifier("settings_alerts_optin_off")
                }
                Toggle(Localizer.tr("settings_alerts_master"), isOn: $alertPreferences.masterEnabled)
                    .disabled(!optIns.reminders)
                    .onChange(of: alertPreferences.masterEnabled) { _, enabled in
                        Task {
                            if enabled { await CareAlertScheduler.shared.requestAuthorizationIfNeeded() }
                            await CarePlanStore.shared.resyncAlerts()
                            refreshNotificationStatus()
                        }
                    }
                Toggle(Localizer.tr("settings_alerts_alarmkit"), isOn: $alertPreferences.useAlarmKitWhenAvailable)
                    .disabled(!optIns.reminders || !CareAlertPreferences.isAlarmKitAvailable)
                    .onChange(of: alertPreferences.useAlarmKitWhenAvailable) { _, _ in
                        Task { await CarePlanStore.shared.resyncAlerts() }
                    }
                Toggle(Localizer.tr("settings_alerts_island"), isOn: $alertPreferences.useLiveActivityCountdown)
                    .disabled(!optIns.reminders)
                    .onChange(of: alertPreferences.useLiveActivityCountdown) { _, _ in
                        Task { await CarePlanStore.shared.resyncAlerts() }
                    }

                HStack {
                    Text(Localizer.tr("settings_alerts_status"))
                        .font(.system(size: 13, weight: .medium))
                    Spacer()
                    Text(notificationStatusText)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }

                Button(action: openSystemSettings) {
                    HStack {
                        Text(Localizer.tr("settings_alerts_open"))
                            .font(.system(size: 13, weight: .semibold))
                        Spacer()
                        Image(systemName: "arrow.up.forward.app")
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
                .buttonStyle(.plain)

                Text("Medication doses and diary check-ins share the same reminder, alarm, and countdown architecture. You can still turn each time on or off in the dashboard editors.")
                    .font(.system(size: 12))
                    .foregroundColor(LiquidGlass.textSecondary)
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private func languageDisplayName(for code: String) -> String {
        switch code {
        case "hi": return "हिन्दी (Hindi)"
        case "ne": return "नेपाली (Nepali)"
        case "zh": return "中文 (Mandarin)"
        default: return "English"
        }
    }

    // MARK: - Section 1: Language & Localization

    private var languageSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_language_section"), icon: "globe")

            VStack(alignment: .leading, spacing: 12) {
                // WP-LN.1: iOS owns per-app language; this row opens the app's page in Settings.
                Button(action: openSystemSettings) {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(Localizer.tr("settings_language_row"))
                                .glassStyle(.headline)
                            Text(AppLocale.current.displayName)
                                .font(.system(size: 13, weight: .medium))
                                .foregroundColor(LiquidGlass.celestialTeal)
                        }
                        Spacer()
                        Image(systemName: "chevron.right")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(LiquidGlass.textSecondary)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("settings_language_row")

                GlassDivider()

                Text(Localizer.tr("settings_lang_note"))
                    .glassStyle(.subheadline)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    // MARK: - Section 2: Sensor Diagnostics

    /// Store build (S-6): plain words instead of the engineering labels Debug keeps.
    private static var calibrationRowTitle: String {
        #if ASD_STORE_BUILD
        Localizer.tr("store_settings_gaze_calibration")
        #else
        Localizer.tr("settings_screen_raycaster")
        #endif
    }

    private var uncalibratedSubtitle: String {
        #if ASD_STORE_BUILD
        isTrueDepthAvailable ? Localizer.tr("store_face_tracking_available") : Self.faceTrackingUnavailableText
        #else
        Localizer.tr("settings_uncalibrated_sub")
        #endif
    }

    private static var faceTrackingUnavailableText: String {
        #if ASD_STORE_BUILD
        Localizer.tr("store_face_tracking_unavailable")
        #else
        "Face tracking unsupported on this device" // i18n-ok: Debug/DemoRelease diagnostics text, unchanged
        #endif
    }

    private var sensorDiagnosticsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_sensors_section"), icon: "sensor.tag.radiowaves.forward.fill")

            VStack(spacing: 12) {
                GlassDiagnosticRow(
                    title: Localizer.tr("settings_truedepth_title"),
                    subtitle: isTrueDepthAvailable ? Localizer.tr("settings_truedepth_available") : Localizer.tr("settings_truedepth_unavailable"),
                    statusText: isTrueDepthAvailable ? Localizer.tr("settings_status_active") : Localizer.tr("settings_status_unavailable"),
                    isGood: isTrueDepthAvailable
                )

                GlassDivider()

                GlassDiagnosticRow(
                    title: Self.calibrationRowTitle,
                    subtitle: calibrationProfile.isCalibrated
                        ? Localizer.tr("settings_calibrated_sub")
                        : uncalibratedSubtitle,
                    statusText: calibrationProfile.isCalibrated ? Localizer.tr("settings_calibrated") : Localizer.tr("settings_uncalibrated"),
                    isGood: calibrationProfile.isCalibrated
                )

                GlassDivider()

                GlassDiagnosticRow(
                    title: Localizer.tr("settings_coremotion"),
                    subtitle: Localizer.tr("settings_coremotion_sub"),
                    statusText: Localizer.tr("settings_status_active"),
                    isGood: true
                )

                if calibrationProfile.isCalibrated {
                    GlassDivider()

                    Button(role: .destructive, action: resetCalibration) {
                        HStack(spacing: 6) {
                            Image(systemName: "arrow.counterclockwise.circle.fill")
                            Text(Localizer.tr("settings_reset_calibration"))
                                .font(.system(size: 13, weight: .semibold))
                            Spacer()
                        }
                    }
                    .buttonStyle(.plain)
                    .foregroundColor(LiquidGlass.warmAmber)
                }
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private var calibrationCheckBinding: CalibrationBinding {
        ChildProfileStore.shared.calibrationBinding(sessionId: calibrationCheckSessionId)
    }

    private func resetCalibration() {
        BoundCalibrationStore.reset()
        calibrationProfile = .identity
        NotificationCenter.default.post(name: GazeSessionController.didUpdateCalibrationNotification, object: nil)
    }

    // MARK: - Section 3: Privacy & Security

    /// Parent/clinic mode switcher — the same choice made at first launch, changeable anytime.
    private var appModeSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: "App Mode", icon: "arrow.triangle.2.circlepath")

            VStack(alignment: .leading, spacing: 12) {
                Picker("App Mode", selection: Binding(
                    get: { appMode.mode },
                    set: { newValue in
                        withAnimation(.easeInOut(duration: 0.2)) {
                            appMode.select(newValue)
                        }
                    }
                )) {
                    ForEach(AppMode.allCases) { mode in
                        Label(mode.displayName, systemImage: mode.systemImage).tag(mode)
                    }
                }
                .pickerStyle(.segmented)

                Text(appMode.mode.tagline)
                    .glassStyle(.subheadline)

                if appMode.mode == .clinic {
                    Label("Shared-device mode: role-based access controls and per-child profiles roll out in upcoming updates.", systemImage: "info.circle")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(LiquidGlass.warmAmber)
                }
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private var privacySecuritySection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_privacy_section"), icon: "lock.shield.fill")

            VStack(alignment: .leading, spacing: 12) {
                // S-12: "Privacy & Your Data" in every build (WP-S.5).
                NavigationLink {
                    PrivacyDashboardView()
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: "hand.raised.fill")
                            .foregroundColor(LiquidGlass.celestialTeal)
                            .font(.system(size: 18))
                        VStack(alignment: .leading, spacing: 2) {
                            Text(ConsentL10n.text("privacy.title", "Privacy & Your Data"))
                                .glassStyle(.headline)
                            Text(ConsentL10n.text("privacy.settings.row", "Optional features, permissions, your consent and delete all."))
                                .glassStyle(.subheadline)
                                .multilineTextAlignment(.leading)
                        }
                        Spacer()
                        Image(systemName: "chevron.right")
                            .foregroundColor(LiquidGlass.textTertiary)
                    }
                    // The whole row is tappable, including the gap on wide (iPad) layouts.
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("settings_privacy_dashboard")

                GlassDivider()

                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "video.slash.fill")
                        .foregroundColor(LiquidGlass.emeraldMint)
                        .font(.system(size: 18))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(Localizer.tr("settings_zero_video_title"))
                            .glassStyle(.headline)
                        Text(Localizer.tr("settings_zero_video_desc"))
                            .glassStyle(.subheadline)
                    }
                }

                GlassDivider()

                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "key.fill")
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .font(.system(size: 18))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(Localizer.tr("settings_encrypted_vault_title"))
                            .glassStyle(.headline)
                        Text(Localizer.tr("settings_encrypted_vault_desc"))
                            .glassStyle(.subheadline)
                    }
                }

                #if ASD_STORE_BUILD
                // S-19: enrolment follows the clinic delivery opt-in, not a study; the norm
                // collection row is not part of the store build.
                if let syncService = ResearchKitSyncService.shared, syncService.isSessionSyncEnabled {
                    GlassDivider()

                    DeviceEnrolmentRow(syncService: syncService)
                        .accessibilityIdentifier("settings_device_enrolment_row")
                }
                #else
                // CNS-9: research rows exist only when a study is configured.
                if ResearchStudyConfiguration.isResearchEnabled {
                    GlassDivider()

                    NormCollectionToggleRow()
                        .accessibilityIdentifier("settings_norm_collection_row")

                    // WP-A.4: only when the study configures the /v1 API.
                    if let syncService = ResearchKitSyncService.shared, syncService.isSessionSyncEnabled {
                        GlassDivider()

                        DeviceEnrolmentRow(syncService: syncService)
                            .accessibilityIdentifier("settings_device_enrolment_row")
                    }
                }
                #endif

                GlassDivider()

                deleteAllDataRow
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
        #if ASD_STORE_BUILD
        // S-24: one confirmation that removes everything; the store build has no care plans.
        .confirmationDialog(
            DeleteAllCopy.confirmTitle,
            isPresented: $showDeleteAllConfirmation,
            titleVisibility: .visible
        ) {
            Button(DeleteAllCopy.confirm, role: .destructive, action: deleteAllObservationData)
                .accessibilityIdentifier("settings_delete_all_confirm")
            Button(DeleteAllCopy.cancel, role: .cancel) {}
        } message: {
            Text(DeleteAllCopy.message)
        }
        #else
        .confirmationDialog(
            "Delete all observation data?",
            isPresented: $showDeleteAllConfirmation,
            titleVisibility: .visible
        ) {
            Button("Delete observation data", role: .destructive, action: deleteAllObservationData)
                .accessibilityIdentifier("settings_delete_all_confirm")
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(Self.deleteAllConfirmationMessage)
        }
        .alert("Observation Data Deleted", isPresented: $showDeleteCarePlansPrompt) {
            Button("Keep care plans", role: .cancel) {
                deletionResultMessage = "Observation data deleted. Your care plans were kept."
            }
            .accessibilityIdentifier("settings_keep_care_plans")
            Button("Delete care plans", role: .destructive) {
                DataDeletionService.live().deleteCarePlans()
                deletionResultMessage = "Observation data and care plans deleted."
            }
            .accessibilityIdentifier("settings_delete_care_plans")
        } message: {
            Text("Your observation data has been removed. Care plans, medications and milestones are your own notes. Would you also like to delete them?")
        }
        #endif
        .alert(
            "Delete all observation data",
            isPresented: Binding(
                get: { deletionResultMessage != nil },
                set: { if !$0 { deletionResultMessage = nil } }
            )
        ) {
            Button("OK", role: .cancel) { deletionResultMessage = nil }
        } message: {
            Text(deletionResultMessage ?? "")
        }
        #if ASD_DEMO_TOOLS
        .sheet(isPresented: $showDemoToolsView) {
            DemoToolsView()
        }
        #endif
        #if ASD_BENCH_CAPTURE
        .fullScreenCover(isPresented: $showBenchCapture) {
            BenchCaptureView()
        }
        #endif
    }

    /// Delete-all copy names research data only in a build that has a study (CNS-9).
    static var deleteAllConfirmationMessage: String {
        #if ASD_STORE_BUILD
        // Store build: no M-CHAT or care data exists; same scope as Privacy & Your Data (S-24).
        return DeleteAllCopy.message
        #else
        return ResearchStudyConfiguration.isResearchEnabled
            ? "This permanently removes saved sessions, session history, queued research uploads, calibration, M-CHAT-R/F answers and study enrollment from this device. It cannot be undone."
            : "This permanently removes saved sessions, session history, calibration and M-CHAT-R/F answers from this device. It cannot be undone."
        #endif
    }

    static var deleteAllRowDetail: String {
        #if ASD_STORE_BUILD
        // What `DataDeletionService.deleteEverything` removes (S-24).
        return DeleteAllCopy.detail
        #else
        return ResearchStudyConfiguration.isResearchEnabled
            ? "Sessions, history, uploads queue, calibration, M-CHAT-R/F answers and enrollment."
            : "Sessions, history, calibration and M-CHAT-R/F answers."
        #endif
    }

    /// CNS-6: reachable in one tap from Settings, behind a confirmation dialog;
    /// care plans are offered on a second, separate prompt (not in the store build, S-24).
    private var deleteAllDataRow: some View {
        Button(role: .destructive) {
            showDeleteAllConfirmation = true
        } label: {
            HStack(spacing: 10) {
                Image(systemName: "trash.fill")
                    .font(.system(size: 18))
                VStack(alignment: .leading, spacing: 2) {
                    #if ASD_STORE_BUILD
                    Text(DeleteAllCopy.button)
                        .font(.system(size: 15, weight: .semibold))
                    #else
                    Text("Delete all observation data")
                        .font(.system(size: 15, weight: .semibold))
                    #endif
                    Text(Self.deleteAllRowDetail)
                        .glassStyle(.subheadline)
                        .multilineTextAlignment(.leading)
                }
                Spacer()
            }
        }
        .buttonStyle(.plain)
        .foregroundColor(LiquidGlass.signalAtypical)
        .accessibilityIdentifier("settings_delete_all_data")
    }

    private func deleteAllObservationData() {
        #if ASD_STORE_BUILD
        // S-24: everything goes, including consent, opt-ins and the child profile; no second prompt.
        let report = DeleteAllCopy.perform()
        calibrationProfile = BoundCalibrationStore.load(for: calibrationCheckBinding)
        let message = report.succeeded ? DeleteAllCopy.done : DeleteAllCopy.failed
        // Present after the confirmation dialog finishes dismissing.
        DispatchQueue.main.async { deletionResultMessage = message }
        #else
        let report = DataDeletionService.live().deleteAll(includeCarePlans: false)
        calibrationProfile = BoundCalibrationStore.load(for: calibrationCheckBinding)
        if report.succeeded {
            // Present after the confirmation dialog finishes dismissing.
            DispatchQueue.main.async { showDeleteCarePlansPrompt = true }
        } else {
            deletionResultMessage = "Some data could not be deleted: \(report.failures.joined(separator: ", ")). Please try again."
        }
        #endif
    }

    // MARK: - Section 4: About

    private var aboutSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_about_section"), icon: "info.circle.fill")

            VStack(spacing: 10) {
                // App Brand Banner Row
                HStack(spacing: 12) {
                    Image("AppLogo")
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .frame(width: 44, height: 44)
                        .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 11, style: .continuous)
                                .stroke(LiquidGlass.cardBorder, lineWidth: 1)
                        )

                    VStack(alignment: .leading, spacing: 2) {
                        Text(StoreBuild.displayName())
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textPrimary)
                        Text(Localizer.tr("settings_gaze_capture_sub"))
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(LiquidGlass.textSecondary)
                    }

                    Spacer()
                }

                if AppLocale.current != .english {
                    Text(Localizer.tr("about_translation_under_review"))
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(LiquidGlass.textSecondary)
                        .accessibilityIdentifier("settings_translation_under_review")
                }

                GlassDivider()

                HStack {
                    Text(Localizer.tr("settings_app_version"))
                        .glassStyle(.body)
                    Spacer()
                    Text(Self.appVersionLabel)
                        .font(.system(size: 13, weight: .bold, design: .monospaced))
                        .foregroundColor(LiquidGlass.textPrimary)
                        .accessibilityIdentifier("settings_app_version")
                }

                #if ASD_STORE_BUILD
                // S-6 / S-9: the same legal links as the store listing.
                if let url = StoreBuild.privacyPolicyURL() {
                    GlassDivider()
                    aboutLinkRow(title: "Privacy Policy", systemImage: "hand.raised.fill", url: url)
                        .accessibilityIdentifier("settings_privacy_policy")
                }
                if let url = StoreBuild.supportURL() {
                    GlassDivider()
                    aboutLinkRow(title: "Support", systemImage: "questionmark.circle.fill", url: url)
                        .accessibilityIdentifier("settings_support")
                }
                #else
                GlassDivider()

                HStack {
                    Text(Localizer.tr("settings_dev_lab"))
                        .glassStyle(.body)
                    Spacer()
                    Text("CurvAsdGazeKit")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(LiquidGlass.celestialTeal)
                }

                GlassDivider()

                HStack {
                    Text(Localizer.tr("settings_regulatory_class"))
                        .glassStyle(.body)
                    Spacer()
                    Text(Localizer.tr("settings_regulatory_value"))
                        .glassStyle(.subheadline)
                }
                #endif
            }
            .glassCard(cornerRadius: 14, padding: 14)

            #if ASD_STORE_BUILD
            Text(StoreBuild.disclaimer)
                .font(.footnote)
                .foregroundColor(LiquidGlass.textSecondary)
                .padding(.horizontal, 4)
            #endif
        }
    }

    #if ASD_STORE_BUILD
    private func aboutLinkRow(title: String, systemImage: String, url: URL) -> some View {
        Button {
            openURL(url)
        } label: {
            HStack {
                Label(title, systemImage: systemImage)
                    .glassStyle(.body)
                Spacer()
                Image(systemName: "arrow.up.right.square")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .accessibilityHidden(true)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
    #endif

    #if ASD_DEMO_TOOLS
    private var demoToolsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: "Demo & Support Tools", icon: "wrench.and.screwdriver.fill")
            Button {
                showDemoToolsView = true
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Open Demo Tools")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(.primary)
                        Text("Synthetic session replay, cloud sync tunnel, and queue controls.")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .foregroundColor(.secondary)
                }
                .glassCard(cornerRadius: 14, padding: 14)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("settings_demo_tools")
        }
    }
    #endif

    #if ASD_BENCH_CAPTURE
    /// WP-G.8 / BEN-2: Debug-only entry to the bench capture screen.
    private var benchCaptureSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: "Bench capture", icon: "scope")
            Button {
                showBenchCapture = true
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Open Bench Capture")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(.primary)
                        Text("5, 9 and 13-point target runs that record raw tracking logs for the accuracy bench.")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .foregroundColor(.secondary)
                }
                .glassCard(cornerRadius: 14, padding: 14)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("settings_bench_capture")
        }
    }
    #endif

    /// "1.0.0 (Build 3)" from the bundle, so About always matches the uploaded build.
    static var appVersionLabel: String {
        let info = Bundle.main.infoDictionary ?? [:]
        let version = info["CFBundleShortVersionString"] as? String ?? "?"
        let build = info["CFBundleVersion"] as? String ?? "?"
        return "\(version) (Build \(build))"
    }

    private var isTrueDepthAvailable: Bool {
        // Same answer as the capture entry gate (ARFaceTrackingConfiguration.isSupported).
        CaptureEntryGate.deviceSupportsFaceTracking
    }

    private var healthKitMoodRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Apple Health mood", systemImage: "heart.text.square")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(LiquidGlass.textPrimary)
                Spacer()
                Text(healthKitMood.authorizationStatusText)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                    .multilineTextAlignment(.trailing)
            }
            Text("Manual emotion logs can write State of Mind samples to the Health app.")
                .font(.system(size: 11))
                .foregroundColor(LiquidGlass.textSecondary)
            if optIns.healthKitMood {
                Button("Authorize HealthKit") {
                    Task { await healthKitMood.requestAuthorization() }
                }
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(LiquidGlass.celestialTeal)
                .buttonStyle(.plain)
            } else {
                // S-15: Health mood runs only while its opt-in is on.
                Text(Localizer.tr("healthkit_optin_off"))
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(LiquidGlass.warmAmber)
            }
            if let err = healthKitMood.lastErrorMessage {
                Text(err)
                    .font(.caption2)
                    .foregroundColor(LiquidGlass.signalAtypical)
            }
        }
    }

    private func refreshNotificationStatus() {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            let text: String
            switch settings.authorizationStatus {
            case .authorized, .provisional, .ephemeral: text = "Allowed"
            case .denied: text = "Denied — enable in iOS Settings"
            case .notDetermined: text = "Not determined"
            @unknown default: text = "Unknown"
            }
            DispatchQueue.main.async {
                notificationStatusText = text
            }
        }
    }

    private func openSystemSettings() {
        #if canImport(UIKit)
        if let url = URL(string: UIApplication.openSettingsURLString) {
            openURL(url) { accepted in
                if !accepted {
                    UIApplication.shared.open(url, options: [:], completionHandler: nil)
                }
            }
        }
        #endif
    }
}

// MARK: - Normative Data Collection Opt-In

#if !ASD_STORE_BUILD

/// Consent-gated opt-in that contributes de-identified per-video metric sets toward
/// building our own age-bracketed normative references (requires active research consent;
/// simulation sessions are always excluded by the gate).
struct NormCollectionToggleRow: View {
    @State private var isOptedIn: Bool = false
    @State private var hasConsent: Bool = false

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "chart.bar.xaxis")
                .foregroundColor(hasConsent ? LiquidGlass.emeraldMint : LiquidGlass.textSecondary)
                .font(.system(size: 18))
            VStack(alignment: .leading, spacing: 4) {
                Text("Normative Data Collection")
                    .glassStyle(.headline)
                Text("Contribute de-identified per-video gaze metrics to help build age-based reference ranges. Requires research consent; never includes video or images.")
                    .glassStyle(.subheadline)
                Text(Localizer.tr("research_raw_samples_note"))
                    .glassStyle(.subheadline)
                Toggle(isOn: Binding(
                    get: { isOptedIn },
                    set: { newValue in
                        guard hasConsent else { return }
                        ResearchKitStudyStore.shared.setNormCollectionOptIn(newValue)
                        isOptedIn = newValue
                    }
                )) {
                    Text(hasConsent ? "Contribute to reference data" : "Requires research consent")
                        .font(.system(size: 13, weight: .semibold))
                }
                .toggleStyle(.switch)
                .tint(hasConsent ? LiquidGlass.emeraldMint : .gray)
                .disabled(!hasConsent)
                .accessibilityLabel("Normative data collection opt-in")
            }
        }
        .onAppear {
            let enrollment = ResearchKitStudyStore.shared.fetchEnrollment()
            hasConsent = enrollment.isConsented
            isOptedIn = enrollment.normCollectionOptIn == true
        }
    }
}
#endif
