import SwiftUI
import CurvAsdGazeKit
#if canImport(ARKit)
import ARKit
#endif

/// System Settings & Diagnostics view adhering to Apple's native Form / List design.
/// Directs language localization to iOS Settings and displays sensor telemetry status.
public struct SettingsView: View {
    @Environment(\.openURL) private var openURL
    @State private var calibrationProfile: GazeCalibrationProfile = GazeCalibrationStore.load()

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
            // Re-read on every appearance: calibrating happens in a sibling tab.
            .onAppear { calibrationProfile = GazeCalibrationStore.load() }
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
                    subtitle: isTrueDepthAvailable ? Localizer.tr("settings_truedepth_available") : Localizer.tr("settings_simulation_mode"),
                    statusText: isTrueDepthAvailable ? Localizer.tr("settings_status_active") : Localizer.tr("settings_status_simulation"),
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
    }

    // MARK: - Section 3: Privacy & Security

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
            }
            .glassCard(cornerRadius: 14, padding: 14)
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
                        Text("Curv ASD Gaze")
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
