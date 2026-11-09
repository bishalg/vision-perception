import SwiftUI

public struct GazeTelemetryReviewCard: View {
    public let session: GazeTelemetrySession

    public init(session: GazeTelemetrySession) {
        self.session = session
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Gaze session")
                    .font(.headline)
                Spacer()
                Text(session.stimulusTag.rawValue)
                    .font(.caption.weight(.semibold))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(.thinMaterial, in: Capsule())
            }
            ClinicalTermTooltip(term: ClinicalGlossary.geopref)

            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                metric("Valid time", String(format: "%.0fs", locale: Locale.current, session.totalValidTimeSec))
                metric("Social target", session.socialTargetRatio.map { "\(Int($0 * 100))%" } ?? "Not scored")
                metric("Confidence", "\(Int(session.trackingConfidenceIndex * 100))%")
                metric("Saccade latency", session.saccadicLatencyMs.map { String(format: "%.0f ms", locale: Locale.current, $0) } ?? "Not observed")
            }
            Text(session.heatmapSummary)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func metric(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.title3.weight(.semibold).monospacedDigit())
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
