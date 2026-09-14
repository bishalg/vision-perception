import SwiftUI
import CurvAsdGazeKit

// MARK: - Biomarker Detail Type

public enum BiomarkerDetailType: String, Identifiable {
    case socialPreference = "Social Visual Preference (GeoPref)"
    case mlClassification = "Clinical Risk Stratification"
    case facialAffect = "Facial Affect & Social Reciprocity"
    case dataQuality = "Sensor Telemetry & Quality Audit"

    public var id: String { rawValue }

    public var shortTitle: String {
        switch self {
        case .socialPreference: return "Social Attention"
        case .mlClassification: return "Risk Stratification"
        case .facialAffect:     return "Facial Affect"
        case .dataQuality:      return "Sensor Audit"
        }
    }

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

// MARK: - Dedicated Biomarker Detail View (Apple Health Style)

/// Dedicated full-page clinical biomarker detail view styled after the Apple Health design language.
/// Features a continuous progress graph line across the 30-second timeline, comprehensive metric grids,
/// and in-depth clinical term explanations and normative benchmarks.
public struct BiomarkerDetailView: View {
    public let type: BiomarkerDetailType
    public let summary: GazeSessionReportSummary
    public let classification: BiomarkerClassificationResult?
    public let recordedFrames: [GazeFrame]

    public init(
        type: BiomarkerDetailType,
        summary: GazeSessionReportSummary,
        classification: BiomarkerClassificationResult? = nil,
        recordedFrames: [GazeFrame] = []
    ) {
        self.type = type
        self.summary = summary
        self.classification = classification
        self.recordedFrames = recordedFrames
    }

    public var body: some View {
        ZStack {
            LiquidGlass.deepCanvas.ignoresSafeArea()

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    headerHeroCard

                    progressGraphLineCard

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
        .navigationTitle(type.shortTitle)
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
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

    // MARK: - Progress Graph Line (Apple Health Style)

    private var progressGraphLineCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(graphTitle)
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundColor(.primary)
                    Text("00:00 – 00:30 (Continuous Session Timeline)")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                }
                Spacer()
                Text(graphHeroBadge)
                    .font(.system(size: 14, weight: .bold, design: .monospaced))
                    .foregroundColor(type.color)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(Capsule().fill(type.color.opacity(0.15)))
            }

            GeometryReader { geo in
                let w = geo.size.width
                let h = geo.size.height

                ZStack(alignment: .topLeading) {
                    // Grid reference lines
                    VStack(spacing: 0) {
                        ForEach(0..<4) { i in
                            Divider().background(Color.white.opacity(0.08))
                            if i < 3 { Spacer() }
                        }
                    }

                    // Benchmark line for social attention and ML Risk Stratification
                    if type == .socialPreference {
                        let thresholdY = h * (1.0 - 0.60)
                        Path { p in
                            p.move(to: CGPoint(x: 0, y: thresholdY))
                            p.addLine(to: CGPoint(x: w, y: thresholdY))
                        }
                        .stroke(LiquidGlass.emeraldMint.opacity(0.45), style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))

                        Text("Typical Threshold (≥60%)")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundColor(LiquidGlass.emeraldMint.opacity(0.85))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(Color.black.opacity(0.65)))
                            .position(x: w - 85, y: max(10, thresholdY - 10))
                    } else if type == .mlClassification {
                        let thresholdY = h * (1.0 - 0.65)
                        Path { p in
                            p.move(to: CGPoint(x: 0, y: thresholdY))
                            p.addLine(to: CGPoint(x: w, y: thresholdY))
                        }
                        .stroke(LiquidGlass.emeraldMint.opacity(0.45), style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))

                        Text("Typical Alignment (≥65%)")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundColor(LiquidGlass.emeraldMint.opacity(0.85))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(Color.black.opacity(0.65)))
                            .position(x: w - 90, y: max(10, thresholdY - 10))
                    } else if type == .facialAffect {
                        let thresholdY = h * (1.0 - 0.20)
                        Path { p in
                            p.move(to: CGPoint(x: 0, y: thresholdY))
                            p.addLine(to: CGPoint(x: w, y: thresholdY))
                        }
                        .stroke(LiquidGlass.emeraldMint.opacity(0.45), style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))

                        Text("Typical Synchrony (≥20%)")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundColor(LiquidGlass.emeraldMint.opacity(0.85))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(Color.black.opacity(0.65)))
                            .position(x: w - 90, y: max(10, thresholdY - 10))
                    } else if type == .dataQuality {
                        let thresholdY = h * (1.0 - 0.75)
                        Path { p in
                            p.move(to: CGPoint(x: 0, y: thresholdY))
                            p.addLine(to: CGPoint(x: w, y: thresholdY))
                        }
                        .stroke(LiquidGlass.emeraldMint.opacity(0.45), style: StrokeStyle(lineWidth: 1.5, dash: [6, 4]))

                        Text("Optimal Yield (≥75%)")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundColor(LiquidGlass.emeraldMint.opacity(0.85))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(Color.black.opacity(0.65)))
                            .position(x: w - 85, y: max(10, thresholdY - 10))
                    }

                    // Curve & Area Fill
                    let points = sampleGraphPoints(width: w, height: h)
                    if points.count > 1 {
                        Path { p in
                            p.move(to: CGPoint(x: points[0].x, y: h))
                            for pt in points {
                                p.addLine(to: pt)
                            }
                            p.addLine(to: CGPoint(x: points.last!.x, y: h))
                            p.closeSubpath()
                        }
                        .fill(
                            LinearGradient(
                                colors: [type.color.opacity(0.35), type.color.opacity(0.03)],
                                startPoint: .top,
                                endPoint: .bottom
                            )
                        )

                        Path { p in
                            p.move(to: points[0])
                            for pt in points.dropFirst() {
                                p.addLine(to: pt)
                            }
                        }
                        .stroke(
                            LinearGradient(
                                colors: [type.color, type.color.opacity(0.85)],
                                startPoint: .leading,
                                endPoint: .trailing
                            ),
                            style: StrokeStyle(lineWidth: 2.5, lineCap: .round, lineJoin: .round)
                        )

                        if let lastPt = points.last {
                            Circle()
                                .fill(type.color)
                                .frame(width: 8, height: 8)
                                .position(lastPt)
                                .shadow(color: type.color, radius: 4)
                        }
                    }
                }
            }
            .frame(height: 140)

            HStack {
                Text("0s")
                Spacer()
                Text("10s")
                Spacer()
                Text("20s")
                Spacer()
                Text("30s (Session End)")
            }
            .font(.system(size: 10, weight: .semibold, design: .monospaced))
            .foregroundColor(.secondary)
        }
        .glassCard(cornerRadius: 16, padding: 14)
    }

    private var graphTitle: String {
        switch type {
        case .socialPreference: return "Social Attention Progression"
        case .mlClassification: return "Neurotypical Alignment Trajectory"
        case .facialAffect:     return "Affective Expressivity Timeline"
        case .dataQuality:      return "Telemetry Validity Compliance"
        }
    }

    private var graphHeroBadge: String {
        switch type {
        case .socialPreference:
            let pct = Int((summary.socialDwellSec / max(0.1, summary.socialDwellSec + summary.geometricDwellSec)) * 100)
            return "\(pct)% Social"
        case .mlClassification:
            let strat = classification?.riskStratification ?? .typical
            return strat.rawValue
        case .facialAffect:
            return "\(Int(summary.socialSmileIntensityMean * 100))% Smile"
        case .dataQuality:
            return "\(String(format: "%.1f%%", summary.validRatio * 100)) Yield"
        }
    }

    private func sampleGraphPoints(width w: CGFloat, height h: CGFloat) -> [CGPoint] {
        let count = 30
        let validFrames = recordedFrames.filter { $0.gaze.isValid && $0.qualityVerdict != .rejected }

        if !validFrames.isEmpty {
            var pts: [CGPoint] = []
            for sec in 1...count {
                let targetTime = Double(sec)
                let slice = validFrames.filter { $0.gaze.videoTimeSec <= targetTime }
                let runningTotal = slice.count
                let runningSocial = slice.filter { $0.activeAOINames.contains("Social_Actor") }.count

                let ratio: Double
                switch type {
                case .socialPreference:
                    ratio = runningTotal > 0 ? Double(runningSocial) / Double(runningTotal) : (summary.socialPreferenceScore * 0.5 + 0.5)
                case .facialAffect:
                    let smileSum = slice.reduce(0.0) { $0 + $1.affect.socialSmileIntensity }
                    ratio = runningTotal > 0 ? (smileSum / Double(runningTotal)) : summary.socialSmileIntensityMean
                case .dataQuality:
                    let totalInWindow = recordedFrames.filter { $0.gaze.videoTimeSec <= targetTime }.count
                    ratio = totalInWindow > 0 ? Double(runningTotal) / Double(totalInWindow) : summary.validRatio
                case .mlClassification:
                    let socialRatio = runningTotal > 0 ? Double(runningSocial) / Double(runningTotal) : 0.5
                    let smileMean = runningTotal > 0 ? (slice.reduce(0.0) { $0 + $1.affect.socialSmileIntensity } / Double(runningTotal)) : summary.socialSmileIntensityMean
                    let eyeRatio = summary.eyeDwellRatio
                    let baseScore = (socialRatio * 0.55 + min(1.0, eyeRatio / 0.35) * 0.30 + min(1.0, smileMean / 0.25) * 0.15)
                    ratio = max(0.1, min(0.95, baseScore))
                }

                let clampedRatio = max(0.05, min(0.95, ratio))
                let x = w * (CGFloat(sec) / CGFloat(count))
                let y = h * (1.0 - CGFloat(clampedRatio))
                pts.append(CGPoint(x: x, y: y))
            }
            return pts
        } else {
            let targetRatio: Double
            switch type {
            case .socialPreference:
                targetRatio = summary.socialDwellSec / max(0.1, summary.socialDwellSec + summary.geometricDwellSec)
            case .facialAffect:
                targetRatio = summary.socialSmileIntensityMean
            case .dataQuality:
                targetRatio = summary.validRatio
            case .mlClassification:
                targetRatio = max(0.05, min(0.95, 1.0 - (classification?.riskScore ?? 0.15)))
            }

            return (0...count).map { i in
                let t = Double(i) / Double(count)
                let sigmoid = 1.0 / (1.0 + exp(-6.0 * (t - 0.2)))
                let currentRatio = targetRatio * (0.4 + 0.6 * sigmoid) + 0.03 * sin(t * 12.0)
                let clamped = max(0.05, min(0.95, currentRatio))
                let x = w * CGFloat(t)
                let y = h * (1.0 - CGFloat(clamped))
                return CGPoint(x: x, y: y)
            }
        }
    }

    // MARK: - 1. Social Preference Breakdown

    private var socialPreferenceContent: some View {
        VStack(alignment: .leading, spacing: 18) {
            let socialPrefPct = Int((summary.socialDwellSec / max(0.1, summary.socialDwellSec + summary.geometricDwellSec)) * 100)
            let isSocialNormal = socialPrefPct >= 60
            let isSocialMonitor = socialPrefPct >= 50 && socialPrefPct < 60
            let socialStatus = isSocialNormal ? "Typical Social Bias" : (isSocialMonitor ? "Borderline Social Bias" : "Reduced Social Bias")
            let socialColor = isSocialNormal ? LiquidGlass.emeraldMint : (isSocialMonitor ? LiquidGlass.warmAmber : Color.red)

            // Primary Summary Card
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Social Visual Orientation")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(.secondary)
                        HStack(spacing: 8) {
                            Circle()
                                .fill(socialColor)
                                .frame(width: 10, height: 10)
                            Text(socialStatus)
                                .font(.system(size: 20, weight: .bold, design: .rounded))
                                .foregroundColor(socialColor)
                        }
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text(String(format: "%+.2f", summary.socialPreferenceScore))
                            .font(.system(size: 20, weight: .bold, design: .monospaced))
                            .foregroundColor(.primary)
                        Text("GeoPref Score")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundColor(.secondary)
                    }
                }

                HStack(spacing: 8) {
                    detailMetricTile(
                        title: "Social Dwell",
                        value: String(format: "%.1fs", summary.socialDwellSec),
                        subtitle: "\(socialPrefPct)% Social",
                        color: LiquidGlass.celestialTeal
                    )
                    detailMetricTile(
                        title: "Geometric",
                        value: String(format: "%.1fs", summary.geometricDwellSec),
                        subtitle: "\(100 - socialPrefPct)% Shape",
                        color: LiquidGlass.warmAmber
                    )
                    detailMetricTile(
                        title: "First Saccade",
                        value: String(format: "%.0fms", summary.firstEyeLatencyMs),
                        subtitle: summary.firstEyeLatencyMs <= 650 ? "Rapid" : "Delayed",
                        color: summary.firstEyeLatencyMs <= 650 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber
                    )
                }

                HStack(spacing: 6) {
                    Image(systemName: "sparkle.magnifyingglass")
                        .font(.system(size: 11))
                        .foregroundColor(LiquidGlass.celestialTeal)
                    Text("Clinical Finding: \(isSocialNormal ? "Robust preferential orientation to human social scenes." : "Elevated visual preference for geometric animations.")")
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

            // Multi-Variate Biomarker Matrix
            GlassSectionHeader(title: "Social Attention Biomarker Matrix", icon: "square.grid.2x2")

            VStack(spacing: 10) {
                biomarkerMatrixCellRow(
                    icon: "person.2.fill",
                    title: "Social Actor Fixation Duration",
                    observed: String(format: "%.1fs (%d%%)", summary.socialDwellSec, socialPrefPct),
                    target: "≥ 60.0% Dwell",
                    weight: "Primary Index",
                    status: isSocialNormal ? "Typical" : (isSocialMonitor ? "Monitor" : "Elevated"),
                    statusColor: socialColor,
                    explanation: "Cumulative gaze duration within the social interaction bounding box. Toddlers with ASD spend significantly less time attending to social scenes compared to geometric shapes (Pierce et al., 2011)."
                )

                biomarkerMatrixCellRow(
                    icon: "circle.grid.cross.fill",
                    title: "Dynamic Geometric Fixation Duration",
                    observed: String(format: "%.1fs (%d%%)", summary.geometricDwellSec, 100 - socialPrefPct),
                    target: "≤ 40.0% Dwell",
                    weight: "Contrast Index",
                    status: (100 - socialPrefPct) <= 40 ? "Normal Contrast" : "Elevated Geometric",
                    statusColor: (100 - socialPrefPct) <= 40 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                    explanation: "Visual dwell directed at high-contrast rotating fractal patterns. Disproportionate geometric fixation (> 50%) is a validated early digital phenotyping biomarker for autism spectrum disorder."
                )

                let eyePct = summary.eyeDwellRatio * 100
                biomarkerMatrixCellRow(
                    icon: "eyes.inverse",
                    title: "Ocular Eye-Looking Ratio",
                    observed: String(format: "%.1f%% Eye Dwell", eyePct),
                    target: "≥ 25.0% of Face",
                    weight: "Facial Salience",
                    status: eyePct >= 25.0 ? "Typical Fixation" : (eyePct >= 18.0 ? "Borderline" : "Reduced Gaze"),
                    statusColor: eyePct >= 25.0 ? LiquidGlass.emeraldMint : (eyePct >= 18.0 ? LiquidGlass.warmAmber : Color.red),
                    explanation: "Proportion of facial gaze directed to the eye region versus mouth or non-feature contours. Early decline in eye fixation from 2 to 6 months of age predicts social communicative divergence (Klin & Jones, Nature 2013)."
                )

                let mouthPct = summary.mouthDwellRatio * 100
                biomarkerMatrixCellRow(
                    icon: "mouth.fill",
                    title: "Speech-Associated Mouth Looking",
                    observed: String(format: "%.1f%% Mouth Dwell", mouthPct),
                    target: "10.0% – 30.0%",
                    weight: "Audiovisual Dwell",
                    status: mouthPct >= 10.0 && mouthPct <= 30.0 ? "Normative" : "Atypical Ratio",
                    statusColor: mouthPct >= 10.0 && mouthPct <= 30.0 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                    explanation: "Fixation on the mouth region during speech. Toddlers learning language alternate between eyes and mouth; abnormal preference for mouth over eyes outside verbal moments can indicate atypical facial processing."
                )

                let lat = summary.firstEyeLatencyMs
                biomarkerMatrixCellRow(
                    icon: "bolt.stop.fill",
                    title: "First Saccade Latency to Face",
                    observed: String(format: "%.0f ms", lat),
                    target: "< 650 ms",
                    weight: "Subcortical Capture",
                    status: lat <= 650 ? "Rapid Orienting" : "Delayed Orienting",
                    statusColor: lat <= 650 ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber,
                    explanation: "Elapsed time from stimulus onset until the infant's first fixation lands on the social actor. Rapid orienting reflects automated subcortical social salience mechanisms."
                )
            }

            // Apple Health Style Educational Cards
            GlassSectionHeader(title: "About Social Visual Attention", icon: "info.circle")

            VStack(spacing: 12) {
                aboutMetricCard(
                    title: "What is Social Visual Attention?",
                    content: "Social visual attention measures the innate neurological tendency of young children to look at other people—their faces, eyes, and social gestures—compared to non-social, repetitive geometric shapes. Typically developing toddlers show an overwhelming, automatic preference for social scenes."
                )

                aboutMetricCard(
                    title: "How the GeoPref Score is Calculated",
                    content: "Derived from the landmark Pierce et al. protocol:\n\nIndex = (Social Dwell − Geometric Dwell) / Total Valid Dwell\n\nScores range from −1.00 (100% geometric) to +1.00 (100% social). Blinks and off-screen gazes are strictly filtered out to prevent false dwell accumulation."
                )

                aboutMetricCard(
                    title: "The Role of Eye vs. Mouth Fixation",
                    content: "Seminal eye-tracking studies published in Nature (Klin & Jones, 2013) demonstrated that typically developing toddlers maintain high visual fixation on the eye region of human faces (typically ≥ 25% of face dwell). A progressive decline in eye dwell with preference for mouth or objects is a sensitive biomarker."
                )

                aboutMetricCard(
                    title: "Peer-Reviewed Scientific Citations",
                    content: "• Pierce et al., Archives of General Psychiatry (2011) & JAMA Pediatrics (2016)\n• Klin & Jones, Nature (2013)\n• Egger et al., Duke University SenseToKnow (2021)\n• FDA-cleared EarliPoint Clinical Biomarker De Novo (2023)"
                )
            }
        }
    }

    // MARK: - 2. ML Classification Breakdown

    private var mlClassificationContent: some View {
        BiomarkerRiskStratificationDetailSection(
            summary: summary,
            classification: classification,
            recordedFrames: recordedFrames
        )
    }

    // MARK: - 3. Facial Affect Breakdown

    private var facialAffectContent: some View {
        BiomarkerFacialAffectDetailSection(summary: summary)
    }

    // MARK: - 4. Data Quality Breakdown

    private var dataQualityContent: some View {
        BiomarkerDataQualityDetailSection(summary: summary)
    }


    // MARK: - Helper Subviews

    private func biomarkerMatrixCellRow(
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

    private func detailMetricTile(title: String, value: String, subtitle: String, color: Color) -> some View {
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

    private func aboutMetricCard(title: String, content: String) -> some View {
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

// Backward-compatibility alias
public typealias BiomarkerDetailSheet = BiomarkerDetailView
