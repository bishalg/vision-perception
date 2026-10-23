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
    @State private var newConditionTitle: String = ""
    @State private var notificationStatusText: String = "Not determined"
    @State private var calibrationProfile: GazeCalibrationProfile = GazeCalibrationStore.load()
    @State private var showQuickCalibrate: Bool = false
    @State private var showCameraCheck: Bool = false
    @State private var showOnboardingFlow: Bool = false
    @State private var showDeleteAllConfirmation: Bool = false
    @State private var showDeleteCarePlansPrompt: Bool = false
    @State private var deletionResultMessage: String?

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
                        appModeSection
                        profileSection
                        medicalConditionsSection
                        careAlertsSection
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
                calibrationProfile = GazeCalibrationStore.load()
                refreshNotificationStatus()
            }
            .fullScreenCover(isPresented: $showQuickCalibrate) {
                BubbleCalibrationView(
                    onClose: {
                        showQuickCalibrate = false
                        calibrationProfile = GazeCalibrationStore.load()
                    },
                    onCalibrationComplete: {
                        showQuickCalibrate = false
                        calibrationProfile = GazeCalibrationStore.load()
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
                            .foregroundColor(Color.white.opacity(0.8))
                            .padding(16)
                    }
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
                                .fill(LiquidGlass.celestialTeal.opacity(0.15))
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
                                .fill(LiquidGlass.warmAmber.opacity(0.15))
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
                            .fill(LiquidGlass.celestialTeal.opacity(0.18))
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
                                .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.18)))
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

                GlassDivider()

                healthKitMoodRow

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
                Text("Caregiver-entered conditions and allergies for medication context. Not a diagnosis from gaze screening.")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)

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
                Toggle(Localizer.tr("settings_alerts_master"), isOn: $alertPreferences.masterEnabled)
                    .onChange(of: alertPreferences.masterEnabled) { _, enabled in
                        Task {
                            if enabled { await CareAlertScheduler.shared.requestAuthorizationIfNeeded() }
                            await CarePlanStore.shared.resyncAlerts()
                            refreshNotificationStatus()
                        }
                    }
                Toggle(Localizer.tr("settings_alerts_alarmkit"), isOn: $alertPreferences.useAlarmKitWhenAvailable)
                    .disabled(!CareAlertPreferences.isAlarmKitAvailable)
                    .onChange(of: alertPreferences.useAlarmKitWhenAvailable) { _, _ in
                        Task { await CarePlanStore.shared.resyncAlerts() }
                    }
                Toggle(Localizer.tr("settings_alerts_island"), isOn: $alertPreferences.useLiveActivityCountdown)
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
                    .foregroundColor(.secondary)
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
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(Localizer.tr("settings_active_language"))
                            .glassStyle(.headline)
                        Text(AppLocale.current.displayName)
                            .font(.system(size: 13, weight: .medium))
                            .foregroundColor(LiquidGlass.celestialTeal)
                    }
                    Spacer()
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(LiquidGlass.emeraldMint)
                }

                GlassDivider()

                Text(Localizer.tr("settings_lang_note"))
                    .glassStyle(.subheadline)
                    .fixedSize(horizontal: false, vertical: true)

                Button(action: openSystemSettings) {
                    HStack {
                        Image(systemName: "gear")
                        Text(Localizer.tr("settings_open_button"))
                            .fontWeight(.semibold)
                        Spacer()
                        Image(systemName: "arrow.up.forward.app")
                            .font(.system(size: 12))
                    }
                    .font(.system(size: 14))
                    // theme:fixed-media — dark text on the fixed teal button fill.
                    .foregroundColor(.black)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(LiquidGlass.celestialTeal))
                }
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    // MARK: - Section 2: Sensor Diagnostics

    private var sensorDiagnosticsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            GlassSectionHeader(title: Localizer.tr("settings_sensors_section"), icon: "sensor.tag.radiowaves.forward.fill")

            VStack(spacing: 12) {
                GlassDiagnosticRow(
                    title: Localizer.tr("settings_truedepth_title"),
                    subtitle: isTrueDepthAvailable ? Localizer.tr("settings_truedepth_available") : "Face tracking unsupported on this device",
                    statusText: isTrueDepthAvailable ? Localizer.tr("settings_status_active") : "Unsupported",
                    isGood: isTrueDepthAvailable
                )

                GlassDivider()

                GlassDiagnosticRow(
                    title: Localizer.tr("settings_screen_raycaster"),
                    subtitle: calibrationProfile.isCalibrated
                        ? Localizer.tr("settings_calibrated_sub")
                        : Localizer.tr("settings_uncalibrated_sub"),
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

    private func resetCalibration() {
        GazeCalibrationStore.reset()
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

                GlassDivider()

                NormCollectionToggleRow()

                GlassDivider()

                deleteAllDataRow
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
        .confirmationDialog(
            "Delete all observation data?",
            isPresented: $showDeleteAllConfirmation,
            titleVisibility: .visible
        ) {
            Button("Delete observation data", role: .destructive, action: deleteAllObservationData)
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This permanently removes saved sessions, session history, queued research uploads, calibration, M-CHAT-R/F answers and study enrollment from this device. It cannot be undone.")
        }
        .alert("Also delete care plans?", isPresented: $showDeleteCarePlansPrompt) {
            Button("Keep care plans", role: .cancel) {
                deletionResultMessage = "Observation data deleted. Your care plans were kept."
            }
            Button("Delete care plans", role: .destructive) {
                DataDeletionService.live().deleteCarePlans()
                deletionResultMessage = "Observation data and care plans deleted."
            }
        } message: {
            Text("Care plans, medications and milestones are your own notes. They are kept unless you delete them here.")
        }
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
    }

    /// CNS-6: reachable in one tap from Settings, behind a confirmation dialog;
    /// care plans are offered on a second, separate prompt.
    private var deleteAllDataRow: some View {
        Button(role: .destructive) {
            showDeleteAllConfirmation = true
        } label: {
            HStack(spacing: 10) {
                Image(systemName: "trash.fill")
                    .font(.system(size: 18))
                VStack(alignment: .leading, spacing: 2) {
                    Text("Delete all observation data")
                        .font(.system(size: 15, weight: .semibold))
                    Text("Sessions, history, uploads queue, calibration, M-CHAT-R/F answers and enrollment.")
                        .glassStyle(.subheadline)
                        .multilineTextAlignment(.leading)
                }
                Spacer()
            }
        }
        .buttonStyle(.plain)
        .foregroundColor(.red)
        .accessibilityIdentifier("settings_delete_all_data")
    }

    private func deleteAllObservationData() {
        let report = DataDeletionService.live().deleteAll(includeCarePlans: false)
        calibrationProfile = GazeCalibrationStore.load()
        if report.succeeded {
            // Present after the confirmation dialog finishes dismissing.
            DispatchQueue.main.async { showDeleteCarePlansPrompt = true }
        } else {
            deletionResultMessage = "Some data could not be deleted: \(report.failures.joined(separator: ", ")). Please try again."
        }
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
                        Text("ASD Gaze")
                            .font(.system(size: 15, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.textPrimary)
                        Text("Pediatric Vision Perception")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(LiquidGlass.textSecondary)
                    }

                    Spacer()
                }

                GlassDivider()

                HStack {
                    Text(Localizer.tr("settings_app_version"))
                        .glassStyle(.body)
                    Spacer()
                    Text("1.0.0 (Build 1)")
                        .font(.system(size: 13, weight: .bold, design: .monospaced))
                        .foregroundColor(LiquidGlass.textPrimary)
                }

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
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private var isTrueDepthAvailable: Bool {
        #if canImport(ARKit) && !os(macOS)
        return ARFaceTrackingConfiguration.isSupported
        #else
        return false
        #endif
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
                .foregroundColor(.secondary)
            Button("Authorize HealthKit") {
                Task { await healthKitMood.requestAuthorization() }
            }
            .font(.system(size: 13, weight: .semibold))
            .foregroundColor(LiquidGlass.celestialTeal)
            .buttonStyle(.plain)
            if let err = healthKitMood.lastErrorMessage {
                Text(err)
                    .font(.caption2)
                    .foregroundColor(.red)
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

/// Consent-gated opt-in that contributes de-identified per-video metric sets toward
/// building our own age-bracketed normative references (requires active research consent;
/// simulation sessions are always excluded by the gate).
struct NormCollectionToggleRow: View {
    @State private var isOptedIn: Bool = false
    @State private var hasConsent: Bool = false

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "chart.bar.xaxis")
                .foregroundColor(hasConsent ? LiquidGlass.emeraldMint : .secondary)
                .font(.system(size: 18))
            VStack(alignment: .leading, spacing: 4) {
                Text("Normative Data Collection")
                    .glassStyle(.headline)
                Text("Contribute de-identified per-video gaze metrics to help build age-based reference ranges. Requires research consent; never includes video or images.")
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
