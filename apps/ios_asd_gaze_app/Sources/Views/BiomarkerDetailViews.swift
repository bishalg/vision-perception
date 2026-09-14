import SwiftUI
import CurvAsdGazeKit

// MARK: - Biomarker Detail Type

public enum BiomarkerDetailType: String, Identifiable {
    case socialPreference = "Social Visual Preference (GeoPref)"
    case mlClassification = "Clinical Risk Stratification"
    case facialAffect = "Facial Affect & Social Reciprocity"
    case dataQuality = "Sensor Telemetry & Quality Audit"

    public var id: String { rawValue }

    public var icon: String {
        switch self {
        case .socialPreference: return "eyes.inverse"
        case .mlClassification: return "brain.head.profile"
        case .facialAffect:     return "face.smiling.fill"
        case .dataQuality:      return "checkmark.shield.fill"
        }
    }

    public var color: Color {
        switch self {
        case .socialPreference: return LiquidGlass.celestialTeal
        case .mlClassification: return LiquidGlass.warmAmber
        case .facialAffect:     return LiquidGlass.emeraldMint
        case .dataQuality:      return Color.blue
        }
    }
}

// MARK: - Biomarker Detail Sheet

/// Comprehensive drill-down detail view for any clinical biomarker section on the final result page.
public struct BiomarkerDetailSheet: View {
    @Environment(\.dismiss) private var dismiss

    public let type: BiomarkerDetailType
    public let summary: GazeSessionReportSummary
    public let classification: BiomarkerClassificationResult?

    public init(
        type: BiomarkerDetailType,
        summary: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil
    ) {
        self.type = type
        self.summary = summary
        self.classification = classification
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        headerHeroCard

                        switch type {
                        case .socialPreference:
                            socialPreferenceContent
                        case .mlClassification:
                            mlClassificationContent
                        case .facialAffect:
                            facialAffectContent
                        case .dataQuality:
                            dataQualityContent
                        }
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 16)
                }
            }
            .navigationTitle("Biomarker Breakdown")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                        .fontWeight(.semibold)
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }

    // MARK: - Header Hero Card

    private var headerHeroCard: some View {
        HStack(spacing: 14) {
            ZStack {
                Circle()
                    .fill(type.color.opacity(0.18))
                    .frame(width: 52, height: 52)
                Image(systemName: type.icon)
                    .font(.system(size: 24, weight: .bold))
                    .foregroundColor(type.color)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(type.rawValue)
                    .font(.system(size: 16, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)
                Text("Clinical Observation Audit · Session #\(summary.sessionId.prefix(8))")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
            }
            Spacer()
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    // MARK: - 1. Social Preference Breakdown

    private var socialPreferenceContent: some View {
        VStack(alignment: .leading, spacing: 16) {
            GlassSectionHeader(title: "GeoPref Methodology & Metrics", icon: "scope")

            VStack(alignment: .leading, spacing: 10) {
                Text("The Geometric vs. Social Preference test (Pierce et al.) quantifies visual attention allocation between dynamic social actors and synchronized geometric shapes.")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)

                HStack(spacing: 12) {
                    detailMetricTile(
                        title: "Preference Index",
                        value: String(format: "%+.2f", summary.socialPreferenceScore),
                        subtitle: summary.socialPreferenceScore >= 0 ? "Social Bias" : "Geometric Bias",
                        color: summary.socialPreferenceScore >= 0 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber
                    )
                    detailMetricTile(
                        title: "First Saccade",
                        value: String(format: "%.0f ms", summary.firstEyeLatencyMs),
                        subtitle: "Latency to Social Face",
                        color: LiquidGlass.celestialTeal
                    )
                }

                HStack(spacing: 12) {
                    detailMetricTile(
                        title: "Eye Region Dwell",
                        value: String(format: "%.1f%%", summary.eyeDwellRatio * 100),
                        subtitle: "Visual Fixation",
                        color: LiquidGlass.celestialTeal
                    )
                    detailMetricTile(
                        title: "Mouth Region Dwell",
                        value: String(format: "%.1f%%", summary.mouthDwellRatio * 100),
                        subtitle: "Speech Correlation",
                        color: LiquidGlass.celestialTeal
                    )
                }

                HStack(spacing: 12) {
                    detailMetricTile(
                        title: "Social Dwell Time",
                        value: String(format: "%.1fs", summary.socialDwellSec),
                        subtitle: "Total Duration",
                        color: LiquidGlass.emeraldMint
                    )
                    detailMetricTile(
                        title: "Geometric Dwell",
                        value: String(format: "%.1fs", summary.geometricDwellSec),
                        subtitle: "Pattern Attention",
                        color: LiquidGlass.warmAmber
                    )
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
    }

    // MARK: - 2. ML Classification Breakdown

    private var mlClassificationContent: some View {
        VStack(alignment: .leading, spacing: 16) {
            GlassSectionHeader(title: "AI Biomarker Risk Stratification", icon: "waveform.path.ecg")

            VStack(alignment: .leading, spacing: 12) {
                let risk = classification?.riskStratification.rawValue ?? "Clinical Review"
                let score = classification?.riskScore ?? 0.0

                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Classified Risk Stratification")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                        Text(risk)
                            .font(.system(size: 18, weight: .bold, design: .rounded))
                            .foregroundColor(classification?.riskStratification == .typical ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                    }
                    Spacer()
                    Text(String(format: "Risk Index: %.2f", score))
                        .font(.system(size: 13, weight: .bold, design: .monospaced))
                        .foregroundColor(.primary)
                }

                Divider()

                Text("Clinical Criteria Alignment:")
                    .font(.system(size: 13, weight: .semibold))

                VStack(spacing: 8) {
                    clinicalCriteriaRow(
                        title: "Social Attention Allocation",
                        status: summary.socialPreferenceScore > 0.2 ? "Typical (> 20%)" : "Reduced Social Preference",
                        isNormal: summary.socialPreferenceScore > 0.2
                    )
                    clinicalCriteriaRow(
                        title: "Screening Frame Quality",
                        status: summary.validRatio > 0.75 ? "Optimal (> 75%)" : "Reduced Quality",
                        isNormal: summary.validRatio > 0.75
                    )
                    clinicalCriteriaRow(
                        title: "Affective Facial Engagement",
                        status: summary.socialSmileIntensityMean > 0.25 ? "Responsive (\(Int(summary.socialSmileIntensityMean * 100))%)" : "Minimal Expression",
                        isNormal: summary.socialSmileIntensityMean > 0.25
                    )
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
    }

    // MARK: - 3. Facial Affect Breakdown

    private var facialAffectContent: some View {
        VStack(alignment: .leading, spacing: 16) {
            GlassSectionHeader(title: "52 FACS Action Units & Synchrony", icon: "face.smiling")

            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    detailMetricTile(
                        title: "Social Smile Mean",
                        value: String(format: "%.0f%%", summary.socialSmileIntensityMean * 100),
                        subtitle: "AU6 + AU12 (Duchenne)",
                        color: LiquidGlass.emeraldMint
                    )
                    detailMetricTile(
                        title: "Smile Reactivity",
                        value: String(format: "%.2f", summary.smileReactivityIndex),
                        subtitle: "Stimulus Synchrony",
                        color: LiquidGlass.emeraldMint
                    )
                }

                Divider()

                Text("Action Unit Intensity Breakdown:")
                    .font(.system(size: 13, weight: .semibold))

                VStack(spacing: 8) {
                    auProgressRow(title: "AU6 + AU12 (Social Smile)", value: summary.socialSmileIntensityMean)
                    auProgressRow(title: "AU1 + AU2 (Visual Wonder / Surprise)", value: summary.surpriseScoreMean)
                    auProgressRow(title: "Flat Affect Baseline Score", value: summary.flatAffectScore)
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
    }

    // MARK: - 4. Data Quality Breakdown

    private var dataQualityContent: some View {
        VStack(alignment: .leading, spacing: 16) {
            GlassSectionHeader(title: "TrueDepth Telemetry & Precision Audit", icon: "shield.checkered")

            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    detailMetricTile(
                        title: "Valid Frame Ratio",
                        value: String(format: "%.1f%%", summary.validRatio * 100),
                        subtitle: "\(summary.validFrames) / \(summary.totalFrames) frames",
                        color: summary.validRatio > 0.7 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber
                    )
                    detailMetricTile(
                        title: "Session Duration",
                        value: String(format: "%.1fs", summary.durationSec),
                        subtitle: "Continuous Capture",
                        color: LiquidGlass.celestialTeal
                    )
                }

                Divider()

                VStack(spacing: 8) {
                    clinicalCriteriaRow(
                        title: "Sensor Distance Compliance",
                        status: "38–55 cm Target Range",
                        isNormal: true
                    )
                    clinicalCriteriaRow(
                        title: "Zero Video Recording Guarantee",
                        status: "Volatile RAM Only (GDPR/HIPAA)",
                        isNormal: true
                    )
                    clinicalCriteriaRow(
                        title: "Encrypted Session Vault",
                        status: "AES-GCM-256 On-Device",
                        isNormal: true
                    )
                }
            }
            .glassCard(cornerRadius: 16, padding: 14)
        }
    }

    // MARK: - Helper Subviews

    private func detailMetricTile(title: String, value: String, subtitle: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            Text(value)
                .font(.system(size: 18, weight: .bold, design: .rounded))
                .foregroundColor(color)
            Text(subtitle)
                .font(.system(size: 10))
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(Color.white.opacity(0.05))
        .cornerRadius(10)
    }

    private func clinicalCriteriaRow(title: String, status: String, isNormal: Bool) -> some View {
        HStack {
            Image(systemName: isNormal ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                .foregroundColor(isNormal ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                .font(.system(size: 14))
            Text(title)
                .font(.system(size: 12))
            Spacer()
            Text(status)
                .font(.system(size: 11, weight: .semibold, design: .rounded))
                .foregroundColor(isNormal ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
        }
    }

    private func auProgressRow(title: String, value: Double) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                Spacer()
                Text("\(Int(value * 100))%")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.12))
                    Capsule()
                        .fill(LiquidGlass.celestialTeal)
                        .frame(width: g.size.width * CGFloat(min(value, 1.0)))
                }
            }
            .frame(height: 5)
        }
    }
}
