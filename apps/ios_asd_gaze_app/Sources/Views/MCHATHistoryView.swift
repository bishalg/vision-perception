import SwiftUI

// MARK: - M-CHAT Survey History (data review)

/// Lists every persisted M-CHAT-R/F result (newest first) with risk and
/// follow-up badges. Reads the ResearchKit vault; no new storage.
public struct MCHATHistoryView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var surveys: [MCHATScoreResult] = []

    public init() {}

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        if surveys.isEmpty {
                            VStack(spacing: 10) {
                                Image(systemName: "list.clipboard")
                                    .font(.system(size: 34))
                                    .foregroundColor(.secondary)
                                Text("No surveys yet")
                                    .font(.system(size: 15, weight: .bold, design: .rounded))
                                    .foregroundColor(.primary)
                                Text("Completed M-CHAT-R/F surveys will appear here with their M-CHAT-R/F categories.")
                                    .font(.system(size: 13))
                                    .foregroundColor(.secondary)
                                    .multilineTextAlignment(.center)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 40)
                        } else {
                            ForEach(surveys.reversed()) { survey in
                                historyRow(survey)
                            }
                        }
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Survey History")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                        .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
            .onAppear {
                surveys = ResearchKitStudyStore.shared.fetchCompletedSurveys()
            }
        }
    }

    private func historyRow(_ survey: MCHATScoreResult) -> some View {
        HStack(spacing: 12) {
            Circle()
                .fill(riskColor(survey.riskLevel))
                .frame(width: 10, height: 10)
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(ClaimsCopy.mchatLine(for: survey.riskLevel))
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundColor(riskColor(survey.riskLevel))
                    if survey.requiresFollowUpInterview {
                        Text("Follow-Up Due")
                            .font(.system(size: 10, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.warmAmber)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(LiquidGlass.warmAmber.opacity(LiquidGlass.tintOpacity)))
                    }
                }
                Text("\(survey.totalFailed) of 20 flagged • \(survey.completionDate.formatted(date: .abbreviated, time: .shortened))")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(cornerRadius: 12, padding: 12)
    }

    private func riskColor(_ risk: MCHATRiskLevel) -> Color {
        switch risk {
        case .low: return LiquidGlass.emeraldMint
        case .medium: return LiquidGlass.warmAmber
        case .high: return LiquidGlass.signalAtypical
        }
    }
}

extension MCHATScoreResult: Identifiable {
    public var id: String {
        "\(completionDate.timeIntervalSince1970)-\(totalFailed)"
    }
}
