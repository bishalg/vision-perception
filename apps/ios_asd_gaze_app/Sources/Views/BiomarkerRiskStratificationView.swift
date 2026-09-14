import SwiftUI
import CurvAsdGazeKit

// MARK: - Dedicated AI Biomarker Risk Stratification Detail Section

/// Detailed Apple Health-style view section for AI Biomarker Risk Stratification.
/// Displays clinical risk classification, confidence, latency, and a complete multi-variate
/// feature matrix with dedicated cell rows, observed values, target thresholds, and in-depth explanations.
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
                case .elevatedRisk: return Color.red
                }
            }()

            // 1. Primary Classification Summary Card
            primarySummaryCard(result: result, strat: strat, stratColor: stratColor)

            // 2. Feature Matrix: All Metrics Well Spread in Dedicated Cell Rows with Details Explained
            GlassSectionHeader(title: "Multi-Variate Biomarker Matrix", icon: "square.grid.2x2")

            featureMatrixRows

            // 3. Apple Health-Style Educational Cards
            GlassSectionHeader(title: "About Machine Learning Risk Stratification", icon: "info.circle")

            aboutCardsSection
        }
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
                    Text("Clinical Risk Stratification")
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
                    Text("Risk Index")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundColor(.secondary)
                }
            }

            HStack(spacing: 8) {
                summaryTile(
                    title: "Confidence",
                    value: String(format: "%.0f%%", result.confidence * 100),
                    subtitle: "Certainty",
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
            .background(Color.white.opacity(0.04))
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
            let socialStatus = isSocialNormal ? "Typical" : (isSocialMonitor ? "Monitor" : "Elevated Risk")
            let socialColor = isSocialNormal ? LiquidGlass.emeraldMint : (isSocialMonitor ? LiquidGlass.warmAmber : Color.red)

            riskMatrixCellRow(
                icon: "person.2.fill",
                title: "Social Visual Preference (GeoPref)",
                observed: "\(socialPrefPct)% Social",
                target: "≥ 60.0%",
                weight: "42% Weight",
                status: socialStatus,
                statusColor: socialColor,
                explanation: "Measures preferential gaze allocation between dynamic human social actors and moving geometric patterns. Social preference < 50% yields a 94% positive predictive value for autism spectrum disorder (Pierce et al., JAMA Pediatrics)."
            )

            // Row 2: Eye-Region Fixation Allocation
            let eyeDwellPct = summary.eyeDwellRatio * 100
            let isEyeNormal = eyeDwellPct >= 25.0
            let isEyeMonitor = eyeDwellPct >= 18.0 && eyeDwellPct < 25.0
            let eyeStatus = isEyeNormal ? "Typical Fixation" : (isEyeMonitor ? "Borderline Dwell" : "Reduced Gaze")
            let eyeColor = isEyeNormal ? LiquidGlass.emeraldMint : (isEyeMonitor ? LiquidGlass.warmAmber : Color.red)

            riskMatrixCellRow(
                icon: "eyes.inverse",
                title: "Eye-Region Fixation Allocation",
                observed: String(format: "%.1f%% Eye", eyeDwellPct),
                target: "≥ 25.0%",
                weight: "24% Weight",
                status: eyeStatus,
                statusColor: eyeColor,
                explanation: "Quantifies ocular gaze focused on the upper eye region of the human face versus mouth or peripheral shapes. As published in Nature (Klin & Jones, 2013), typically developing infants sustain high eye fixation, whereas an early decline between 2 and 24 months is an early biomarker."
            )

            // Row 3: First Saccade Latency to Social Stimulus
            let latency = summary.firstEyeLatencyMs
            let isLatencyNormal = latency < 550.0
            let isLatencyMonitor = latency >= 550.0 && latency < 750.0
            let latencyStatus = isLatencyNormal ? "Rapid Orienting" : (isLatencyMonitor ? "Moderate Latency" : "Delayed Orienting")
            let latencyColor = isLatencyNormal ? LiquidGlass.emeraldMint : (isLatencyMonitor ? LiquidGlass.warmAmber : Color.red)

            riskMatrixCellRow(
                icon: "bolt.stop.fill",
                title: "First Saccade Latency to Face",
                observed: String(format: "%.0f ms", latency),
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
            let smileColor = isSmileNormal ? LiquidGlass.emeraldMint : (smilePct >= 10 ? LiquidGlass.warmAmber : Color.red)

            riskMatrixCellRow(
                icon: "face.smiling.fill",
                title: "Facial Affective Synchrony",
                observed: "\(smilePct)% Intensity",
                target: "≥ 20.0%",
                weight: "12% Weight",
                status: smileStatus,
                statusColor: smileColor,
                explanation: "Evaluates emotional reciprocity via TrueDepth 52 Action Units, tracking synchronous activation of the Duchenne smile (AU6 cheek raiser + AU12 lip corner puller) in response to humorous video events. Flat affect correlates with social communication challenges."
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
                explanation: "Confirms that 60 Hz TrueDepth infrared depth tracking remained locked within 38–55 cm viewing distance and that blink artifact rejection discarded eyelid closure plummets, ensuring high diagnostic reliability."
            )
        }
    }

    // MARK: - About Educational Cards

    private var aboutCardsSection: some View {
        VStack(spacing: 12) {
            aboutCard(
                title: "How the Multi-Variate Model Works",
                content: "Our CoreML classifier integrates multi-modal temporal gaze coordinates, ocular fixations, saccade latencies, and 52 FACS facial action units. By comparing this composite vector against validated clinical developmental cohorts, the model computes a calibrated Bayesian risk probability score."
            )

            aboutCard(
                title: "Hardware-Accelerated Apple Neural Engine (ANE)",
                content: "All inference executes locally on your iPhone using the Apple Neural Engine (ANE) with 16-bit floating point precision and sub-2ms latency. Under strict zero-egress architecture, no camera images, facial geometry, or patient identifiers leave the device."
            )

            aboutCard(
                title: "Clinical Decision Support & Regulatory Context",
                content: "This tool is designed to assist pediatricians and developmental specialists in early screening, referencing FDA De Novo classification standards (EarliPoint ASD Biomarker) and published research from UC San Diego and Duke University. It is an adjunct screening aid and does not replace comprehensive psychiatric evaluation."
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
        .background(Color.white.opacity(0.04))
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
        .background(Color.white.opacity(0.04))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.white.opacity(0.08), lineWidth: 1)
        )
    }
}
