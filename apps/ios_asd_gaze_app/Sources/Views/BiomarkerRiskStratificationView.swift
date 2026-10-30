import SwiftUI
import CurvAsdGazeKit

#if ASD_RESEARCH_CLASSIFIER

// MARK: - Research Classifier Detail Section (Debug builds only)

/// Debug-only research view (ADR-0004 decision 3, CLM-3). The whole file is compiled out of
/// Release builds, where `AttentionFlag` is the only interpretation layer a parent sees.
/// Shows the experimental classifier output with its feature matrix for developers.
public struct BiomarkerRiskStratificationDetailSection: View {
    public let summary: GazeSessionReportSummary
    public let classification: BiomarkerClassificationResult?
    public let recordedFrames: [GazeFrame]

    public init(
        summary: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil,
        recordedFrames: [GazeFrame] = []
    ) {
        self.summary = summary
        self.classification = classification
        self.recordedFrames = recordedFrames
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            let result = classification ?? BiomarkerClassificationEngine.shared.classifySync(summary: summary)
            let strat = result.riskStratification
            let stratColor: Color = {
                switch strat {
                case .typical: return LiquidGlass.emeraldMint
                case .monitor: return LiquidGlass.warmAmber
                case .elevatedRisk: return LiquidGlass.signalAtypical
                }
            }()

            // 0. Guideline 1.4.1 Medical Disclaimer Banner
            medicalDisclaimerCard

            // 1. Primary Classification Summary Card
            primarySummaryCard(result: result, strat: strat, stratColor: stratColor)

            // 2. Feature Matrix: All Metrics Well Spread in Dedicated Cell Rows with Details Explained
            GlassSectionHeader(title: "Feature Matrix", icon: "square.grid.2x2")

            featureMatrixRows

            // 3. Apple Health-Style Educational Cards
            GlassSectionHeader(title: "About the Research Classifier", icon: "info.circle")

            aboutCardsSection
        }
    }

    private var medicalDisclaimerCard: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "cross.case.fill")
                .foregroundColor(LiquidGlass.warmAmber)
                .font(.system(size: 16))
                .padding(.top, 2)

            VStack(alignment: .leading, spacing: 4) {
                Text("Research build only. Not a medical diagnosis.")
                    .font(.system(size: 12, weight: .bold, design: .rounded))
                    .foregroundColor(LiquidGlass.textPrimary)

                Text("This experimental output exists only in developer builds. It compares visual engagement against published cohort ranges and is not shown to parents. This is not a diagnostic tool. Part of Open Spectrum Tracking.")
                    .font(.system(size: 11))
                    .foregroundColor(LiquidGlass.textSecondary)
                    .lineSpacing(2)
            }
        }
        .padding(12)
        .background(LiquidGlass.warmAmber.opacity(LiquidGlass.tintOpacity))
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(LiquidGlass.warmAmber.opacity(0.35), lineWidth: 1)
        )
    }

    // MARK: - Primary Summary Card

    private func primarySummaryCard(
        result: BiomarkerClassificationResult,
        strat: RiskStratification,
        stratColor: Color
    ) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Experimental classifier output")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)
                    HStack(spacing: 8) {
                        Circle()
                            .fill(stratColor)
                            .frame(width: 10, height: 10)
                        Text(strat.rawValue)
                            .font(.system(size: 20, weight: .bold, design: .rounded))
                            .foregroundColor(stratColor)
                    }
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(String(format: "%.1f%%", result.riskScore * 100))
                        .font(.system(size: 20, weight: .bold, design: .monospaced))
                        .foregroundColor(.primary)
                    Text("Index")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundColor(.secondary)
                }
            }

            HStack(spacing: 8) {
                summaryTile(
                    title: "Model certainty",
                    value: String(format: "%.0f%%", result.confidence * 100),
                    subtitle: "Experimental",
                    color: LiquidGlass.celestialTeal
                )
                summaryTile(
                    title: "Inference",
                    value: String(format: "%.1fms", result.inferenceLatencyMs),
                    subtitle: "Latency",
                    color: LiquidGlass.emeraldMint
                )
                summaryTile(
                    title: "Engine",
                    value: result.executionDevice == .appleNeuralEngine ? "ANE" : "CoreML",
                    subtitle: "On-Device",
                    color: Color.purple
                )
            }

            HStack(spacing: 6) {
                Image(systemName: "sparkle.magnifyingglass")
                    .font(.system(size: 11))
                    .foregroundColor(LiquidGlass.celestialTeal)
                Text("Primary Contributing Factor: \(result.primaryContributingFeature)")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(.primary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(LiquidGlass.cardInset)
            .cornerRadius(8)
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    // MARK: - Feature Matrix Rows

    private var featureMatrixRows: some View {
        VStack(spacing: 10) {
            // Row 1: Social Preference
            let socialPrefPct = Int((summary.socialDwellSec / max(0.1, summary.socialDwellSec + summary.geometricDwellSec)) * 100)
            let isSocialNormal = socialPrefPct >= 60
            let isSocialMonitor = socialPrefPct >= 50 && socialPrefPct < 60
            let socialStatus = isSocialNormal ? "Typical" : (isSocialMonitor ? "Monitor" : "Outside range")
            let socialColor = isSocialNormal ? LiquidGlass.emeraldMint : (isSocialMonitor ? LiquidGlass.warmAmber : LiquidGlass.signalAtypical)

            riskMatrixCellRow(
                icon: "person.2.fill",
                title: "Social Visual Preference (GeoPref)",
                observed: "\(socialPrefPct)% Social",
                target: "≥ 60.0%",
                weight: "42% Weight",
                status: socialStatus,
                statusColor: socialColor,
                explanation: "Measures preferential gaze allocation between dynamic human social actors and moving geometric patterns. Compared against published age ranges (Pierce et al., 2011, 2016)."
            )

            // Row 2: Eye-Region Fixation Allocation
            let eyeDwellPct = (summary.eyeDwellRatio ?? 0) * 100
            let hasEye = summary.eyeDwellRatio != nil
            let isEyeNormal = hasEye && eyeDwellPct >= 25.0
            let isEyeMonitor = hasEye && eyeDwellPct >= 18.0 && eyeDwellPct < 25.0
            let eyeStatus = !hasEye ? "Not observed" : (isEyeNormal ? "Typical Fixation" : (isEyeMonitor ? "Borderline Dwell" : "Reduced Gaze"))
            let eyeColor = isEyeNormal ? LiquidGlass.emeraldMint : (isEyeMonitor ? LiquidGlass.warmAmber : LiquidGlass.signalAtypical)

            riskMatrixCellRow(
                icon: "eyes.inverse",
                title: "Eye-Region Fixation Allocation",
                observed: hasEye ? String(format: "%.1f%% Eye", eyeDwellPct) : "Not observed",
                target: "≥ 25.0%",
                weight: "24% Weight",
                status: eyeStatus,
                statusColor: eyeColor,
                explanation: "Quantifies ocular gaze focused on the upper eye region of the human face versus mouth or peripheral shapes. As published in Nature (Klin & Jones, 2013), typically developing infants sustain high eye fixation, whereas an early decline between 2 and 24 months is of research interest."
            )

            // Row 3: First Saccade Latency to Social Stimulus
            let latency = summary.firstEyeLatencyMs ?? 0
            let hasLatency = summary.firstEyeLatencyMs != nil
            let isLatencyNormal = hasLatency && latency < 550.0
            let isLatencyMonitor = hasLatency && latency >= 550.0 && latency < 750.0
            let latencyStatus = !hasLatency ? "Not observed" : (isLatencyNormal ? "Rapid Orienting" : (isLatencyMonitor ? "Moderate Latency" : "Delayed Orienting"))
            let latencyColor = isLatencyNormal ? LiquidGlass.emeraldMint : (isLatencyMonitor ? LiquidGlass.warmAmber : LiquidGlass.signalAtypical)

            riskMatrixCellRow(
                icon: "bolt.stop.fill",
                title: "First Saccade Latency to Face",
                observed: hasLatency ? String(format: "%.0f ms", latency) : "Not observed",
                target: "< 550 ms",
                weight: "14% Weight",
                status: latencyStatus,
                statusColor: latencyColor,
                explanation: "Measures the time in milliseconds from stimulus onset until the child's initial visual saccade lands on the social actor. Rapid orienting reflects automated subcortical social salience mechanisms; prolonged latency indicates delayed attentional capture."
            )

            // Row 4: Facial Affective Synchrony (Duchenne Smile)
            let smilePct = Int(summary.socialSmileIntensityMean * 100)
            let isSmileNormal = smilePct >= 20
            let smileStatus = isSmileNormal ? "Synchronous" : (smilePct >= 10 ? "Attenuated" : "Blunted")
            let smileColor = isSmileNormal ? LiquidGlass.emeraldMint : (smilePct >= 10 ? LiquidGlass.warmAmber : LiquidGlass.signalAtypical)

            riskMatrixCellRow(
                icon: "face.smiling.fill",
                title: "Facial Affective Synchrony",
                observed: "\(smilePct)% Intensity",
                target: "≥ 20.0%",
                weight: "12% Weight",
                status: smileStatus,
                statusColor: smileColor,
                explanation: "Evaluates emotional reciprocity via TrueDepth 52 Action Units, tracking synchronous activation of the Duchenne smile (AU6 cheek raiser + AU12 lip corner puller) in response to humorous video events. Flat affect is tracked as a research measure."
            )

            // Row 5: Sensor Telemetry Yield & Blink Integrity
            let validPct = summary.validRatio * 100
            let isValidNormal = validPct >= 75.0
            let validStatus = isValidNormal ? "Audit Passed" : "Low Telemetry Yield"
            let validColor = isValidNormal ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber

            riskMatrixCellRow(
                icon: "shield.checkered",
                title: "Sensor Telemetry & Tracking Integrity",
                observed: String(format: "%.1f%% Valid", validPct),
                target: "≥ 75.0%",
                weight: "8% Weight",
                status: validStatus,
                statusColor: validColor,
                explanation: "Confirms that 60 Hz TrueDepth infrared depth tracking remained locked within 38–55 cm viewing distance and that blink artifact rejection discarded eyelid closure plummets, supporting reliable measurements."
            )
        }
    }

    // MARK: - About Educational Cards

    private var aboutCardsSection: some View {
        VStack(spacing: 12) {
            aboutCard(
                title: "How the Multi-Variate Model Works",
                content: "Our CoreML classifier integrates multi-modal temporal gaze coordinates, ocular fixations, saccade latencies, and 52 ARKit facial expression coefficients. The experimental model computes an index from these inputs."
            )

            aboutCard(
                title: "On-Device Inference",
                content: "All inference executes locally on your iPhone using the Neural Engine with 16-bit floating point precision and sub-2ms latency. Under strict zero-egress architecture, no camera images, facial geometry, or patient identifiers leave the device."
            )

            aboutCard(
                title: "Research Context",
                content: "This output is experimental, has not been validated, and is not available in the released app."
            )
        }
    }

    // MARK: - Helper Cell Components

    private func riskMatrixCellRow(
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
                    .background(Capsule().fill(statusColor.opacity(LiquidGlass.tintOpacity)))
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
        .background(LiquidGlass.cardInset)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(LiquidGlass.dividerColor, lineWidth: 1)
        )
    }

    private func summaryTile(title: String, value: String, subtitle: String, color: Color) -> some View {
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
        .background(LiquidGlass.cardInset)
        .cornerRadius(8)
    }

    private func aboutCard(title: String, content: String) -> some View {
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
        .background(LiquidGlass.cardInset)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(LiquidGlass.dividerColor, lineWidth: 1)
        )
    }
}

#endif // ASD_RESEARCH_CLASSIFIER
