import SwiftUI
import Charts
import CurvAsdGazeKit

// MARK: - Per-Stimulus Matrix (Expected vs Recorded)

/// Computes the per-video expected-vs-recorded score from stimulus-tagged frames and the
/// bundled provisional baseline. Recomputed on demand — extraction is linear over frames.
struct PerVideoMatrixModel {
    let sessionScore: PerVideoSessionScore?
    let baselineStatus: String?

    init(recordedFrames: [GazeFrame], childAgeMonths: Int) {
        guard let baseline = NormativeBaselineLibrary.loadBundled() else {
            sessionScore = nil
            baselineStatus = nil
            return
        }
        let metricSets = PerVideoMetricsExtractor.extract(from: recordedFrames)
        sessionScore = PerVideoScoringEngine.score(
            metricSets: metricSets,
            baseline: baseline,
            ageMonths: childAgeMonths,
            frames: recordedFrames
        )
        baselineStatus = baseline.status
    }

    var hasReport: Bool {
        guard let score = sessionScore else { return false }
        return score.videos.contains { $0.hasBaseline }
    }
}

extension AttentionFlag {
    var color: Color {
        switch self {
        case .green: return LiquidGlass.emeraldMint
        case .yellow: return LiquidGlass.warmAmber
        case .red: return LiquidGlass.signalAtypical
        }
    }

    var symbol: String {
        switch self {
        case .green: return "checkmark.circle.fill"
        case .yellow: return "exclamationmark.circle.fill"
        case .red: return "flag.fill"
        }
    }
}

struct AttentionFlagBanner: View {
    let assessment: AttentionFlagAssessment

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: assessment.flag.symbol)
                .font(.system(size: 22, weight: .bold))
                .foregroundStyle(assessment.flag.color)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text("\(assessment.flag.title) — \(assessment.headline)")
                    .font(.system(size: 15, weight: .bold, design: .rounded))
                    .foregroundStyle(.primary)
                Text(assessment.detail)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(assessment.flag.color.opacity(0.12))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(assessment.flag.color.opacity(0.45), lineWidth: 1)
        )
        .accessibilityElement(children: .combine)
    }
}

extension ExpectationStatus {
    var color: Color {
        switch self {
        case .onTarget: return LiquidGlass.emeraldMint
        case .monitor: return LiquidGlass.warmAmber
        case .atypical: return LiquidGlass.signalAtypical
        }
    }

    var label: String {
        switch self {
        case .onTarget: return "On Target"
        case .monitor: return "Monitor"
        case .atypical: return "Atypical"
        }
    }

    var symbol: String {
        switch self {
        case .onTarget: return "checkmark.circle.fill"
        case .monitor: return "exclamationmark.circle.fill"
        case .atypical: return "xmark.circle.fill"
        }
    }
}

/// Navigation payloads for the per-stimulus report stack: summary landing page and per-video detail pages.
enum PerVideoNavRoute: Hashable {
    case summary
    case video(String)
}

// MARK: - Summary Card (embedded in GazeSessionResultView)

/// Compact landing card: composite score, status, and entry point into the sub-report stack.
struct PerVideoMatrixCardView: View {
    let score: PerVideoSessionScore
    let baselineStatus: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Image(systemName: "rectangle.split.3x1")
                    .font(.system(size: 14, weight: .bold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                Text("Per-Stimulus Matrix")
                    .font(.system(size: 16, weight: .bold, design: .rounded))
                Spacer()
                provisionalBadge
            }

            HStack(spacing: 14) {
                ScoreRingView(score: score.compositeScore, size: 64)
                    .frame(width: 64, height: 64)

                VStack(alignment: .leading, spacing: 4) {
                    Text("Engagement vs Age Expectation")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(.secondary)
                    Text("Expectations for \(score.ageBracketId) months")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    HStack(spacing: 10) {
                        ForEach(statusTally(score), id: \.0) { tally in
                            HStack(spacing: 3) {
                                Circle().fill(tally.0.color).frame(width: 6, height: 6)
                                Text("\(tally.1)")
                                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                                    .foregroundColor(tally.0.color)
                            }
                        }
                    }
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundColor(.secondary)
            }
            .frame(minHeight: 44)
        }
        .padding(16)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
    }

    private var provisionalBadge: some View {
        Text(baselineStatus == "provisional-literature-seeded" ? "Provisional" : (baselineStatus ?? ""))
            .font(.system(size: 9, weight: .bold, design: .rounded))
            .foregroundColor(.secondary)
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .background(Capsule().fill(Color.secondary.opacity(0.12)))
    }

    private func statusTally(_ score: PerVideoSessionScore) -> [(ExpectationStatus, Int)] {
        let rows = score.videos.flatMap { $0.rows }
        return [(.onTarget, rows), (.monitor, rows), (.atypical, rows)].map { status, rows in
            (status, rows.filter { $0.status == status }.count)
        }
    }
}

// MARK: - Sub-Report Landing Page

/// High-level summary page listing one sub-report per stimulus clip (navigation stack root for the matrix).
struct PerVideoSummaryListView: View {
    let score: PerVideoSessionScore

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                compositeHeader
                if score.videos.contains(where: { !$0.hasBaseline }) {
                    unattributedNote
                }
                ForEach(score.videos, id: \.stimulusId) { report in
                    if report.hasBaseline {
                        NavigationLink(value: PerVideoNavRoute.video(report.stimulusId)) {
                            PerVideoListRow(report: report)
                        }
                        .buttonStyle(.plain)
                    } else {
                        PerVideoListRow(report: report)
                    }
                }
                provisionalFooter
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .background(
            LinearGradient(
                colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                startPoint: .top, endPoint: .bottom
            )
            .ignoresSafeArea()
        )
        .navigationTitle("Sub-Reports")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var compositeHeader: some View {
        VStack(spacing: 10) {
            ScoreRingView(score: score.compositeScore, size: 92)
            Text("Composite Engagement Score")
                .font(.system(size: 14, weight: .bold, design: .rounded))
            Text("Age bracket \(score.ageBracketId) months • \(score.videos.filter { $0.hasBaseline }.count) scored clips")
                .font(.system(size: 11))
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 18)
        .padding(.horizontal, 16)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
    }

    private var unattributedNote: some View {
        HStack(spacing: 8) {
            Image(systemName: "info.circle")
                .font(.system(size: 12))
                .foregroundColor(LiquidGlass.warmAmber)
            Text("Some frames were captured outside a tagged video battery and are excluded from the composite.")
                .font(.system(size: 11))
                .foregroundColor(.secondary)
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 12).fill(LiquidGlass.warmAmber.opacity(0.08)))
    }

    private var provisionalFooter: some View {
        Text("Baseline \(score.baselineSchemaVersion) • \(score.baselineStatus). Values are literature-seeded and not a diagnosis.")
            .font(.system(size: 10))
            .foregroundColor(.secondary)
            .multilineTextAlignment(.center)
            .padding(.top, 6)
    }
}

/// One sub-report row: ≥44 pt touch target, score ring, status tally.
struct PerVideoListRow: View {
    let report: PerVideoScoreReport

    var body: some View {
        HStack(spacing: 12) {
            ScoreRingView(score: report.hasBaseline ? report.subscore : 0, size: 46)
                .frame(width: 46, height: 46)

            VStack(alignment: .leading, spacing: 3) {
                Text(report.title)
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    Text(report.category)
                        .font(.system(size: 10, weight: .medium))
                        .foregroundColor(.secondary)
                    if report.lowConfidence {
                        Label("Low quality", systemImage: "exclamationmark.triangle.fill")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundColor(LiquidGlass.warmAmber)
                    }
                }
                if report.hasBaseline {
                    HStack(spacing: 8) {
                        ForEach([ExpectationStatus.onTarget, .monitor, .atypical], id: \.self) { status in
                            let count = report.rows.filter { $0.status == status }.count
                            if count > 0 {
                                HStack(spacing: 2) {
                                    Circle().fill(status.color).frame(width: 5, height: 5)
                                    Text("\(count)")
                                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                                        .foregroundColor(status.color)
                                }
                            }
                        }
                    }
                }
            }

            Spacer()

            if report.hasBaseline {
                Image(systemName: "chevron.right")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundColor(.secondary)
            }
        }
        .padding(12)
        .frame(minHeight: 68)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
    }
}

// MARK: - Per-Video Detail Page

/// Full sub-report for one stimulus clip: what is shown when, expected vs recorded
/// for each biomarker, deviation chart, and citation footnotes.
struct PerVideoResultDetailView: View {
    let score: PerVideoSessionScore
    let route: PerVideoNavRoute

    private var report: PerVideoScoreReport? {
        guard case .video(let stimulusId) = route else { return nil }
        return score.videos.first { $0.stimulusId == stimulusId }
    }

    var body: some View {
        ScrollView {
            if let report = report {
                VStack(spacing: 14) {
                    detailHeader(report)
                    if !report.rows.isEmpty {
                        deviationChart(report)
                        expectationRows(report)
                    }
                    citationFootnotes(report)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
            } else {
                Text("No report data for this clip.")
                    .foregroundColor(.secondary)
            }
        }
        .background(
            LinearGradient(
                colors: [LiquidGlass.deepCanvas, LiquidGlass.deepCanvasSecondary],
                startPoint: .top, endPoint: .bottom
            )
            .ignoresSafeArea()
        )
        .navigationTitle(report?.title ?? "Detail")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func detailHeader(_ report: PerVideoScoreReport) -> some View {
        VStack(spacing: 10) {
            HStack(spacing: 14) {
                ScoreRingView(score: report.subscore, size: 76)
                VStack(alignment: .leading, spacing: 4) {
                    Text(report.category)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(LiquidGlass.celestialTeal)
                    Text("Expectations for \(score.ageBracketId) months")
                        .font(.system(size: 12, weight: .semibold))
                    Text(String(format: "Clip quality: %.0f%% valid", report.validSampleRatio * 100))
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    if report.lowConfidence {
                        Label("Low sample quality — treat scores as indicative only", systemImage: "exclamationmark.triangle.fill")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundColor(LiquidGlass.warmAmber)
                    }
                }
                Spacer()
            }
        }
        .padding(16)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
    }

    /// Deviation bars: one bar per expectation, colored by status.
    private func deviationChart(_ report: PerVideoScoreReport) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Deviation from Expectation")
                .font(.system(size: 13, weight: .bold, design: .rounded))
            Chart(Array(report.rows.enumerated()), id: \.offset) { index, row in
                BarMark(
                    x: .value("Score", row.score),
                    y: .value("Metric", shortLabel(row))
                )
                .foregroundStyle(row.status.color.opacity(0.85))
                .cornerRadius(4)
                .annotation(position: .trailing) {
                    Text("\(Int(row.score * 100))")
                        .font(.system(size: 9, weight: .bold, design: .monospaced))
                        .foregroundColor(row.status.color)
                }
            }
            .chartXAxis {
                AxisMarks(values: [0, 0.5, 1.0])
            }
            .frame(height: max(64, CGFloat(report.rows.count) * 34))
        }
        .padding(14)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
    }

    private func shortLabel(_ row: PerVideoScoreRow) -> String {
        var label = row.biomarker.replacingOccurrences(of: "_", with: " ")
        if let aoi = row.targetAOI {
            label += " (\(aoi.replacingOccurrences(of: "_", with: " ")))"
        }
        return label
    }

    /// Expected-vs-recorded rows grouped by segment, with the segment timeline first.
    private func expectationRows(_ report: PerVideoScoreReport) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(Array(groupBySegment(report).keys.sorted()), id: \.self) { segmentId in
                let rows = groupBySegment(report)[segmentId] ?? []
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 6) {
                        Circle().fill(LiquidGlass.celestialTeal).frame(width: 7, height: 7)
                        Text(rows.first?.segmentDescription ?? segmentId)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundColor(.secondary)
                    }
                    ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                        ExpectationRowView(row: row)
                    }
                }
                .padding(14)
                .background(LiquidGlass.cardBackground)
                .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
            }
        }
    }

    private func groupBySegment(_ report: PerVideoScoreReport) -> [String: [PerVideoScoreRow]] {
        Dictionary(grouping: report.rows, by: { $0.segmentId })
    }

    private func citationFootnotes(_ report: PerVideoScoreReport) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Basis")
                .font(.system(size: 13, weight: .bold, design: .rounded))
            ForEach(Array(uniqueCitations(report).enumerated()), id: \.offset) { _, citation in
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "text.quote")
                        .font(.system(size: 9))
                        .foregroundColor(.secondary)
                        .padding(.top, 2)
                    Text(citation)
                        .font(.system(size: 10))
                        .foregroundColor(.secondary)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(LiquidGlass.cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(LiquidGlass.cardBorder, lineWidth: 1))
    }

    private func uniqueCitations(_ report: PerVideoScoreReport) -> [String] {
        var seen = Set<String>()
        return report.rows.compactMap { row in
            let entry = "[\(row.source)] \(row.citation)"
            guard seen.insert(entry).inserted else { return nil }
            return entry
        }
    }
}

/// One expected-vs-recorded row: metric name, observed vs expected numbers, status pill, deviation bar.
struct ExpectationRowView: View {
    let row: PerVideoScoreRow

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: row.status.symbol)
                    .font(.system(size: 13))
                    .foregroundColor(row.status.color)
                VStack(alignment: .leading, spacing: 2) {
                    Text(displayName)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundColor(.primary)
                    Text("\(formatValue(row.observed)) recorded • expected \(formatTarget)")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundColor(.secondary)
                }
                Spacer()
                Text(row.status.label)
                    .font(.system(size: 10, weight: .bold, design: .rounded))
                    .foregroundColor(row.status.color)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(row.status.color.opacity(0.12)))
            }
            .padding(.top, 4)

            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.secondary.opacity(0.12))
                    Capsule()
                        .fill(row.status.color.opacity(0.85))
                        .frame(width: max(6, geo.size.width * CGFloat(row.score)))
                }
            }
            .frame(height: 5)

            if row.imputed {
                Text("Target region never fixated — worst case assumed")
                    .font(.system(size: 10, weight: .medium))
                    .foregroundColor(LiquidGlass.warmAmber)
            }
        }
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }

    private var displayName: String {
        var name = row.biomarker.replacingOccurrences(of: "_", with: " ").capitalized
        if let aoi = row.targetAOI {
            name += " — \(aoi.replacingOccurrences(of: "_", with: " "))"
        }
        return name
    }

    private var formatTarget: String {
        if let low = row.acceptableLow, let high = row.acceptableHigh {
            return "\(formatValue(low))–\(formatValue(high))"
        }
        if let expected = row.expected {
            return formatValue(expected)
        }
        return "—"
    }

    private func formatValue(_ value: Double) -> String {
        if row.biomarker.contains("latency") {
            return "\(Int(value)) ms"
        }
        if row.biomarker == "fixation_count" {
            return String(format: "%.0f", value)
        }
        return String(format: "%.0f%%", value * 100)
    }
}

// MARK: - Shared Pieces

/// Score ring 0–100 with status coloring.
struct ScoreRingView: View {
    let score: Double
    let size: CGFloat

    private var statusColor: Color {
        if score >= 0.65 { return ExpectationStatus.onTarget.color }
        if score >= 0.35 { return ExpectationStatus.monitor.color }
        return ExpectationStatus.atypical.color
    }

    var body: some View {
        ZStack {
            Circle().stroke(Color.secondary.opacity(0.15), lineWidth: size * 0.09)
            Circle()
                .trim(from: 0, to: max(0.02, CGFloat(score)))
                .stroke(statusColor, style: StrokeStyle(lineWidth: size * 0.09, lineCap: .round))
                .rotationEffect(.degrees(-90))
            VStack(spacing: -2) {
                Text("\(Int((score * 100).rounded()))")
                    .font(.system(size: size * 0.30, weight: .bold, design: .rounded))
                    .foregroundColor(.primary)
                Text("/100")
                    .font(.system(size: size * 0.14, weight: .semibold, design: .monospaced))
                    .foregroundColor(.secondary)
            }
        }
        .accessibilityLabel("Score \(Int((score * 100).rounded())) out of 100")
    }
}
