import SwiftUI
import CurvAsdGazeKit

// MARK: - Facial Expression Detail Section

/// Apple Health-style detail section for facial expression while watching.
/// Shows smile intensity, smile reactivity, surprise, and flat expression measurements.
public struct BiomarkerFacialAffectDetailSection: View {
    public let summary: GazeSessionReportSummary

    public init(summary: GazeSessionReportSummary) {
        self.summary = summary
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            let smilePct = Int(summary.socialSmileIntensityMean * 100)
            let isSmileNormal = smilePct >= 20
            let affectStatus = isSmileNormal ? "Smiled often" : (smilePct >= 10 ? "Smiled a little" : "Smiled rarely")
            let affectColor = isSmileNormal ? LiquidGlass.emeraldMint : (smilePct >= 10 ? LiquidGlass.warmAmber : LiquidGlass.warmAmber)

            // Primary Summary Card
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Facial Expression While Watching")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(.secondary)
                        HStack(spacing: 8) {
                            Circle()
                                .fill(affectColor)
                                .frame(width: 10, height: 10)
                            Text(affectStatus)
                                .font(.system(size: 20, weight: .bold, design: .rounded))
                                .foregroundColor(affectColor)
                        }
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("\(smilePct)%")
                            .font(.system(size: 20, weight: .bold, design: .monospaced))
                            .foregroundColor(.primary)
                        Text("Mean Smile")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundColor(.secondary)
                    }
                }

                HStack(spacing: 8) {
                    summaryMetricTile(
                        title: "Smile",
                        value: "\(smilePct)%",
                        subtitle: "Cheeks + mouth",
                        color: LiquidGlass.emeraldMint
                    )
                    summaryMetricTile(
                        title: "Reactivity",
                        value: String(format: "%.2f", summary.smileReactivityIndex),
                        subtitle: "Matches clips",
                        color: LiquidGlass.celestialTeal
                    )
                    summaryMetricTile(
                        title: "Surprise",
                        value: "\(Int(summary.surpriseScoreMean * 100))%",
                        subtitle: "Brow raise",
                        color: Color.purple
                    )
                }

                HStack(spacing: 6) {
                    Image(systemName: "sparkle.magnifyingglass")
                        .font(.system(size: 11))
                        .foregroundColor(LiquidGlass.emeraldMint)
                    Text("Observation: \(isSmileNormal ? "Smiled along with the funny moments in the clips." : "Smiled less during the clips.")")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.primary)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.white.opacity(0.04))
                .cornerRadius(8)
            }
            .glassCard(cornerRadius: 16, padding: 14)

            // Measurements
            GlassSectionHeader(title: "Facial Expression Measurements", icon: "square.grid.2x2")

            VStack(spacing: 10) {
                biomarkerDetailMatrixRow(
                    icon: "face.smiling.fill",
                    title: "Social Smile",
                    observed: "\(smilePct)% Mean Intensity",
                    target: "≥ 20.0%",
                    weight: "Shared Joy",
                    status: isSmileNormal ? "Higher" : (smilePct >= 10 ? "Medium" : "Lower"),
                    statusColor: affectColor,
                    explanation: "How strongly the mouth corners and cheeks lifted while the child watched. A bigger number means a bigger smile."
                )

                biomarkerDetailMatrixRow(
                    icon: "waveform.path.badge.plus",
                    title: "Smile Reactivity",
                    observed: String(format: "%.2f Index", summary.smileReactivityIndex),
                    target: "≥ 0.30",
                    weight: "Timing",
                    status: summary.smileReactivityIndex >= 0.30 ? "Responsive" : "Less responsive",
                    statusColor: summary.smileReactivityIndex >= 0.30 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                    explanation: "How closely the child's smiles lined up in time with the funny moments in the clips."
                )

                let surpPct = Int(summary.surpriseScoreMean * 100)
                biomarkerDetailMatrixRow(
                    icon: "eye.fill",
                    title: "Surprise Expression",
                    observed: "\(surpPct)% Mean Intensity",
                    target: "≥ 15.0%",
                    weight: "Novelty Detection",
                    status: surpPct >= 15 ? "Expressive" : "Subtle",
                    statusColor: surpPct >= 15 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                    explanation: "How much the eyebrows lifted when something unexpected happened in a clip."
                )

                biomarkerDetailMatrixRow(
                    icon: "face.dashed.fill",
                    title: "Neutral Expression",
                    observed: String(format: "%.2f Score", summary.flatAffectScore),
                    target: "≤ 0.25 (Low Baseline)",
                    weight: "Expression Range",
                    status: summary.flatAffectScore <= 0.25 ? "Expressive" : "More neutral",
                    statusColor: summary.flatAffectScore <= 0.25 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                    explanation: "How much of the session the face stayed neutral. Children differ widely, and this number does not identify any condition."
                )

            }

            // Apple Health Style Educational Cards
            GlassSectionHeader(title: "About Facial Expression", icon: "info.circle")

            VStack(spacing: 12) {
                biomarkerExplanationCard(
                    title: "What is smile reactivity?",
                    content: "Smile reactivity looks at whether a child smiles along with what is on screen, for example when the person in the video laughs. Every child reacts differently, and one session is only a snapshot."
                )

                biomarkerExplanationCard(
                    title: "How smiles are measured",
                    content: "The phone's depth camera tracks how far the mouth corners and cheeks lift. Only these numbers are kept. No pictures or video are stored."
                )
            }
        }
    }
}

// MARK: - Recording Quality Detail Section

/// Apple Health-style detail section for recording quality.
/// Shows the share of usable gaze samples and how blinks are handled. It displays only values
/// measured in the session; nothing here is a fixed or placeholder reading.
public struct BiomarkerDataQualityDetailSection: View {
    public let summary: GazeSessionReportSummary

    public init(summary: GazeSessionReportSummary) {
        self.summary = summary
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            let validPct = summary.validRatio * 100
            let isQualityNormal = validPct >= 75.0
            let qualityStatus = isQualityNormal ? "Good recording" : "Consider repeating"
            let qualityColor = isQualityNormal ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber

            // Primary Summary Card
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Recording Quality")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(.secondary)
                        HStack(spacing: 8) {
                            Circle()
                                .fill(qualityColor)
                                .frame(width: 10, height: 10)
                            Text(qualityStatus)
                                .font(.system(size: 20, weight: .bold, design: .rounded))
                                .foregroundColor(qualityColor)
                        }
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text(String(format: "%.1f%%", validPct))
                            .font(.system(size: 20, weight: .bold, design: .monospaced))
                            .foregroundColor(.primary)
                        Text("Usable")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundColor(.secondary)
                    }
                }

                HStack(spacing: 8) {
                    summaryMetricTile(
                        title: "Usable Samples",
                        value: "\(summary.validFrames)",
                        subtitle: "Of \(summary.totalFrames) total",
                        color: LiquidGlass.emeraldMint
                    )
                    summaryMetricTile(
                        title: "Duration",
                        value: String(format: "%.1fs", summary.durationSec),
                        subtitle: "Continuous",
                        color: LiquidGlass.celestialTeal
                    )
                    summaryMetricTile(
                        title: "Interruptions",
                        value: "\(summary.interruptionCount)",
                        subtitle: String(format: "%.1fs paused", summary.interruptedSec),
                        color: Color.blue
                    )
                }

                HStack(spacing: 6) {
                    Image(systemName: "checkmark.shield.fill")
                        .font(.system(size: 11))
                        .foregroundColor(LiquidGlass.emeraldMint)
                    Text(isQualityNormal ? "Most of the session had a clear view of the eyes." : "Part of the session had no clear view of the eyes. A repeat may give a better picture.")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.primary)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.white.opacity(0.04))
                .cornerRadius(8)
            }
            .glassCard(cornerRadius: 16, padding: 14)

            // Measurements
            GlassSectionHeader(title: "Recording Quality Measurements", icon: "square.grid.2x2")

            VStack(spacing: 10) {
                biomarkerDetailMatrixRow(
                    icon: "wave.3.forward.circle.fill",
                    title: "Usable Gaze Samples",
                    observed: String(format: "%.1f%% (%d frames)", validPct, summary.validFrames),
                    target: "≥ 75.0%",
                    weight: "Baseline",
                    status: isQualityNormal ? "Good" : "Lower",
                    statusColor: qualityColor,
                    explanation: "Share of camera frames in which the eyes were clearly visible. The other measurements are computed from these usable samples."
                )

                biomarkerDetailMatrixRow(
                    icon: "eye.slash.fill",
                    title: "Blink Handling",
                    observed: "Eyes closed below 45% open",
                    target: "Excluded",
                    weight: "Cleaning",
                    status: "On",
                    statusColor: LiquidGlass.emeraldMint,
                    explanation: "Frames in which the eyelids are mostly closed are left out so a blink is never counted as looking at something."
                )
            }

            // Apple Health Style Educational Cards
            GlassSectionHeader(title: "About Recording Quality and Privacy", icon: "info.circle")

            VStack(spacing: 12) {
                biomarkerExplanationCard(
                    title: "Why blinks are left out",
                    content: "When an eyelid closes, the gaze estimate briefly jumps downward. The app detects closed eyes, ignores those frames, and waits a few frames after the eyes reopen."
                )

                biomarkerExplanationCard(
                    title: "Privacy",
                    content: "No camera video or photos are saved. Only gaze and expression numbers are kept, encrypted on this device."
                )
            }
        }
    }
}

// MARK: - Internal Shared Components for Affect & Quality

private func biomarkerDetailMatrixRow(
    icon: String,
    title: String,
    observed: String,
    target: String,
    weight: String,
    status: String,
    statusColor: Color,
    explanation: String
) -> some View {
    VStack(alignment: .leading, spacing: 8) {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(statusColor)
            Text(title)
                .font(.system(size: 13, weight: .bold, design: .rounded))
                .foregroundColor(.primary)
            Spacer()
            Text(status)
                .font(.system(size: 10, weight: .bold))
                .foregroundColor(statusColor)
                .padding(.horizontal, 7)
                .padding(.vertical, 3)
                .background(Capsule().fill(statusColor.opacity(0.16)))
        }

        HStack(spacing: 12) {
            HStack(spacing: 3) {
                Text("Observed:")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                Text(observed)
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundColor(.primary)
            }
            HStack(spacing: 3) {
                Text("Target:")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                Text(target)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundColor(.secondary)
            }
            Spacer()
            Text(weight)
                .font(.system(size: 10, weight: .semibold))
                .foregroundColor(LiquidGlass.celestialTeal)
        }

        Text(explanation)
            .font(.system(size: 11, weight: .regular))
            .foregroundColor(.secondary)
            .lineSpacing(2.5)
    }
    .padding(12)
    .background(Color.white.opacity(0.04))
    .cornerRadius(12)
    .overlay(
        RoundedRectangle(cornerRadius: 12)
            .stroke(Color.white.opacity(0.08), lineWidth: 1)
    )
}

private func summaryMetricTile(title: String, value: String, subtitle: String, color: Color) -> some View {
    VStack(alignment: .leading, spacing: 3) {
        Text(title)
            .font(.system(size: 10))
            .foregroundColor(.secondary)
        Text(value)
            .font(.system(size: 15, weight: .bold, design: .rounded))
            .foregroundColor(color)
        Text(subtitle)
            .font(.system(size: 9))
            .foregroundColor(.secondary)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(8)
    .background(Color.white.opacity(0.04))
    .cornerRadius(8)
}

private func biomarkerExplanationCard(title: String, content: String) -> some View {
    VStack(alignment: .leading, spacing: 6) {
        Text(title)
            .font(.system(size: 13, weight: .bold, design: .rounded))
            .foregroundColor(.primary)
        Text(content)
            .font(.system(size: 12, weight: .regular))
            .foregroundColor(.secondary)
            .lineSpacing(3)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(12)
    .background(Color.white.opacity(0.04))
    .cornerRadius(12)
    .overlay(
        RoundedRectangle(cornerRadius: 12)
            .stroke(Color.white.opacity(0.08), lineWidth: 1)
    )
}
