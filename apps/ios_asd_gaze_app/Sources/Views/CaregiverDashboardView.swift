import SwiftUI
import CurvAsdGazeKit

/// Main caregiver dashboard for longitudinal milestones and screening sessions.
public struct CaregiverDashboardView: View {
    @State private var selectedLocale: AppLocale = .english
    public let onStartScreening: () -> Void

    public init(onStartScreening: @escaping () -> Void) {
        self.onStartScreening = onStartScreening
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                // Liquid Glass twilight gradient background
                LinearGradient(
                    colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        headerSection
                        regulatoryNoticeCard
                        quickActionCard
                        milestonesSection
                        recentSessionsSection
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 16)
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Picker("Language", selection: $selectedLocale) {
                        ForEach(AppLocale.allCases) { locale in
                            Text(locale.displayName).tag(locale)
                        }
                    }
                    .pickerStyle(.menu)
                    .tint(LiquidGlass.celestialTeal)
                }
            }
        }
    }

    // MARK: - Subviews

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Localizer.tr("dashboard_title", lang: selectedLocale))
                .font(.system(size: 26, weight: .bold, design: .rounded))
                .foregroundColor(.white)

            Text(Localizer.tr("dashboard_subtitle", lang: selectedLocale))
                .font(.system(size: 14, weight: .medium))
                .foregroundColor(Color.white.opacity(0.7))
        }
    }

    private var regulatoryNoticeCard: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "info.circle.fill")
                .foregroundColor(LiquidGlass.celestialTeal)
                .font(.system(size: 20))

            VStack(alignment: .leading, spacing: 4) {
                Text("Clinical Research & Screening Tool")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(.white)

                Text("This app measures objective visual attention distributions. It does not provide diagnostic decisions. Consult a developmental pediatrician for comprehensive evaluation.")
                    .font(.system(size: 12))
                    .foregroundColor(Color.white.opacity(0.75))
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .glassCard(cornerRadius: 14, padding: 14)
    }

    private var quickActionCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Ready for Today's Observation?")
                        .font(.system(size: 16, weight: .semibold, design: .rounded))
                        .foregroundColor(.white)

                    Text("Includes 1-minute 5-point calibration followed by short video clips.")
                        .font(.system(size: 13))
                        .foregroundColor(Color.white.opacity(0.7))
                }
                Spacer()
                Image(systemName: "eyes")
                    .font(.system(size: 30))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }

            GlassButton(
                title: Localizer.tr("start_screening", lang: selectedLocale),
                systemImage: "play.fill",
                isPrimary: true,
                action: onStartScreening
            )
        }
        .glassCard(cornerRadius: 16, padding: 16)
    }

    private var milestonesSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Developmental Milestones (CareKit)")
                .font(.system(size: 18, weight: .semibold, design: .rounded))
                .foregroundColor(.white)

            VStack(spacing: 10) {
                milestoneRow(
                    age: "18 Months Checkup",
                    dateText: "Completed May 15, 2026",
                    status: "Completed",
                    isDone: true
                )
                milestoneRow(
                    age: "24 Months Follow-up",
                    dateText: "Recommended Window: Nov 2026",
                    status: "Due Soon",
                    isDone: false
                )
                milestoneRow(
                    age: "36 Months Final Review",
                    dateText: "Scheduled for Nov 2027",
                    status: "Upcoming",
                    isDone: false
                )
            }
        }
    }

    private func milestoneRow(age: String, dateText: String, status: String, isDone: Bool) -> some View {
        HStack(spacing: 14) {
            Image(systemName: isDone ? "checkmark.circle.fill" : "circle.dashed")
                .foregroundColor(isDone ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                .font(.system(size: 22))

            VStack(alignment: .leading, spacing: 2) {
                Text(age)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundColor(.white)

                Text(dateText)
                    .font(.system(size: 12))
                    .foregroundColor(Color.white.opacity(0.6))
            }
            Spacer()

            Text(status)
                .font(.system(size: 11, weight: .semibold))
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
                .background(
                    Capsule()
                        .fill(isDone ? LiquidGlass.emeraldMint.opacity(0.2) : LiquidGlass.warmAmber.opacity(0.2))
                )
                .foregroundColor(isDone ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
        }
        .glassCard(cornerRadius: 12, padding: 12)
    }

    private var recentSessionsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Recent Telemetry Summaries")
                .font(.system(size: 18, weight: .semibold, design: .rounded))
                .foregroundColor(.white)

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Split-Screen GeoPref Battery")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundColor(.white)
                        Text("30s clip • 60 Hz TrueDepth • ID: #7F8B")
                            .font(.system(size: 12))
                            .foregroundColor(Color.white.opacity(0.6))
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("94% Valid")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundColor(LiquidGlass.emeraldMint)
                        Text("Quality: Optimal")
                            .font(.system(size: 11))
                            .foregroundColor(Color.white.opacity(0.6))
                    }
                }

                Divider()
                    .background(Color.white.opacity(0.1))

                HStack(spacing: 20) {
                    biomarkerMetric(title: "Social Pref", value: "+0.48")
                    biomarkerMetric(title: "Eye Dwell", value: "28.5%")
                    biomarkerMetric(title: "First-Eye Lat", value: "480ms")
                    biomarkerMetric(title: "Entropy", value: "1.42")
                }
            }
            .glassCard(cornerRadius: 14, padding: 14)
        }
    }

    private func biomarkerMetric(title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.system(size: 11))
                .foregroundColor(Color.white.opacity(0.6))
            Text(value)
                .font(.system(size: 14, weight: .bold, design: .monospaced))
                .foregroundColor(.white)
        }
    }
}
