import Charts
import SwiftUI

public struct AttentionTrendChartView: View {
    public let points: [AttentionTrendPoint]
    public let average: Double?

    public init(points: [AttentionTrendPoint], average: Double?) {
        self.points = points
        self.average = average
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("7-day social attention")
                    .font(.headline)
                Spacer()
                if let average {
                    Text("\(Int(average * 100))% avg")
                        .font(.subheadline.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }
            Chart(points) { point in
                AreaMark(
                    x: .value("Day", point.dayLabel),
                    yStart: .value("Low", 0.45),
                    yEnd: .value("High", 0.70)
                )
                .foregroundStyle(.green.opacity(0.12))

                BarMark(
                    x: .value("Day", point.dayLabel),
                    y: .value("Social", point.socialPreferenceScore)
                )
                .foregroundStyle(point.isLatest ? Color.accentColor : Color.secondary.opacity(0.45))
                .cornerRadius(4)

                RuleMark(y: .value("Target", 0.50))
                    .lineStyle(StrokeStyle(lineWidth: 1, dash: [4, 3]))
                    .foregroundStyle(.green)
                    .annotation(position: .trailing, alignment: .leading) {
                        Text("50%")
                            .font(.caption2)
                            .foregroundStyle(.green)
                    }
            }
            .chartYScale(domain: 0...1)
            .frame(height: 180)
            .accessibilityLabel("Seven day social attention bar chart with 50 percent target")

            if let latest = points.last {
                HStack {
                    Label("\(Int(latest.sessionDurationSec))s", systemImage: "timer")
                    Text(latest.stimulusTag.rawValue)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(.thinMaterial, in: Capsule())
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
        }
    }
}
