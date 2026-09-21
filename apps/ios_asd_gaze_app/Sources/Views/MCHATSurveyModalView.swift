import SwiftUI

/// Liquid Glass presentation of the standardized 20-item M-CHAT-R/F toddler autism screening survey
/// administered as an Apple ResearchKit `ORKQuestionStep` sequence.
public struct MCHATSurveyModalView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var answers: [Int: Bool] = [:]
    @State private var currentQuestionIndex: Int = 0
    @State private var evaluatedResult: MCHATScoreResult?

    private let questions = MCHATSurveyEngine.questions
    private let onCompleted: ((MCHATScoreResult) -> Void)?

    public init(onCompleted: ((MCHATScoreResult) -> Void)? = nil) {
        self.onCompleted = onCompleted
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        headerSection
                        progressBarSection

                        if let result = evaluatedResult {
                            resultCard(result: result)
                        } else {
                            activeQuestionCard
                            navigationButtons
                        }
                    }
                    .padding(20)
                }
            }
            .navigationTitle("M-CHAT-R/F Survey")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .foregroundColor(.secondary)
                }
            }
        }
    }

    // MARK: - Header & Progress

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Label("Clinical Research Instrument", systemImage: "stethoscope")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundColor(LiquidGlass.celestialTeal)
                Spacer()
                Text("20 Questions")
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundColor(.secondary)
            }

            Text("Modified Checklist for Autism in Toddlers")
                .font(.system(size: 18, weight: .bold, design: .rounded))
                .foregroundColor(.primary)

            Text("Standardized developmental screening validated for toddlers 16–30 months.")
                .font(.system(size: 12))
                .foregroundColor(.secondary)
        }
    }

    private var progressBarSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Question \(currentQuestionIndex + 1) of \(questions.count)")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(.secondary)
                Spacer()
                Text("\(Int((Double(answers.count) / Double(questions.count)) * 100))% Complete")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundColor(LiquidGlass.celestialTeal)
            }

            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.gray.opacity(0.2))
                    Capsule()
                        .fill(LiquidGlass.celestialTeal)
                        .frame(width: geo.size.width * CGFloat(answers.count) / CGFloat(questions.count))
                        .animation(.easeInOut(duration: 0.2), value: answers.count)
                }
            }
            .frame(height: 6)
        }
    }

    // MARK: - Active Question Card

    private var activeQuestionCard: some View {
        let q = questions[currentQuestionIndex]
        let currentAnswer = answers[q.id]

        return VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top) {
                Text("Q\(q.id)")
                    .font(.system(size: 14, weight: .black, design: .monospaced))
                    .foregroundColor(LiquidGlass.warmAmber)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(LiquidGlass.warmAmber.opacity(0.15)))

                Spacer()

                if q.isCritical {
                    Text("Critical Item")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundColor(LiquidGlass.celestialTeal)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Capsule().fill(LiquidGlass.celestialTeal.opacity(0.12)))
                }
            }

            Text(q.text)
                .font(.system(size: 17, weight: .semibold))
                .foregroundColor(.primary)
                .lineSpacing(3)

            HStack(spacing: 12) {
                Button {
                    selectAnswer(questionId: q.id, value: true)
                } label: {
                    HStack {
                        Image(systemName: currentAnswer == true ? "checkmark.circle.fill" : "circle")
                        Text("Yes")
                            .fontWeight(.bold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(
                        currentAnswer == true
                            ? LiquidGlass.celestialTeal.opacity(0.25)
                            : LiquidGlass.cardBackground
                    )
                    .foregroundColor(currentAnswer == true ? LiquidGlass.celestialTeal : .primary)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(
                                currentAnswer == true ? LiquidGlass.celestialTeal : LiquidGlass.cardBorder,
                                lineWidth: 1.5
                            )
                    )
                }

                Button {
                    selectAnswer(questionId: q.id, value: false)
                } label: {
                    HStack {
                        Image(systemName: currentAnswer == false ? "checkmark.circle.fill" : "circle")
                        Text("No")
                            .fontWeight(.bold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(
                        currentAnswer == false
                            ? LiquidGlass.warmAmber.opacity(0.25)
                            : LiquidGlass.cardBackground
                    )
                    .foregroundColor(currentAnswer == false ? LiquidGlass.warmAmber : .primary)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(
                                currentAnswer == false ? LiquidGlass.warmAmber : LiquidGlass.cardBorder,
                                lineWidth: 1.5
                            )
                    )
                }
            }
        }
        .glassCard(cornerRadius: 18, padding: 18)
    }

    // MARK: - Navigation Buttons

    private var navigationButtons: some View {
        HStack(spacing: 12) {
            Button("Previous") {
                if currentQuestionIndex > 0 {
                    currentQuestionIndex -= 1
                }
            }
            .disabled(currentQuestionIndex == 0)
            .foregroundColor(currentQuestionIndex == 0 ? .gray.opacity(0.5) : .primary)
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(Capsule().fill(LiquidGlass.cardBackground))

            Spacer()

            if answers.count == questions.count {
                Button(action: calculateAndReview) {
                    HStack {
                        Image(systemName: "checkmark.seal.fill")
                        Text("Review Score")
                            .fontWeight(.bold)
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 12)
                    .background(LiquidGlass.emeraldMint)
                    .foregroundColor(.black) // theme:fixed-media
                    .clipShape(Capsule())
                }
            } else {
                Button("Next") {
                    if currentQuestionIndex < questions.count - 1 {
                        currentQuestionIndex += 1
                    }
                }
                .disabled(answers[questions[currentQuestionIndex].id] == nil)
                .foregroundColor(answers[questions[currentQuestionIndex].id] == nil ? .gray.opacity(0.5) : LiquidGlass.celestialTeal)
                .padding(.horizontal, 20)
                .padding(.vertical, 12)
                .background(Capsule().fill(LiquidGlass.cardBackground))
            }
        }
    }

    // MARK: - Result Review Card

    private func resultCard(result: MCHATScoreResult) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Image(systemName: "chart.bar.doc.horizontal.fill")
                    .font(.system(size: 24))
                    .foregroundColor(riskColor(result.riskLevel))

                VStack(alignment: .leading, spacing: 2) {
                    Text(result.riskLevel.rawValue)
                        .font(.system(size: 18, weight: .bold))
                        .foregroundColor(riskColor(result.riskLevel))
                    Text("Total At-Risk Items: \(result.totalFailed) of 20")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                }
                Spacer()
            }

            Divider().background(LiquidGlass.dividerColor)

            Text(result.clinicalRecommendation)
                .font(.system(size: 13))
                .foregroundColor(.primary)
                .lineSpacing(3)

            // M-CHAT-R/F protocol: medium scores must complete the structured
            // Follow-Up interview before any referral decision is made.
            if result.requiresFollowUpInterview {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundColor(LiquidGlass.warmAmber)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Follow-Up Interview Needed")
                            .font(.system(size: 13, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.warmAmber)
                        Text("Ask the clinician for the M-CHAT Follow-Up questions for the \(result.failedQuestionIds.count) flagged items.")
                            .font(.system(size: 12))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .background(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(LiquidGlass.warmAmber.opacity(0.12))
                        )
                )
            }

            HStack(alignment: .top, spacing: 8) {
                Image(systemName: "info.circle.fill")
                    .foregroundColor(LiquidGlass.textSecondary)
                    .font(.system(size: 13))
                Text("The M-CHAT-R/F is a parent-reported screening tool, not a clinical diagnosis. Always discuss results with your pediatrician.")
                    .font(.system(size: 11))
                    .foregroundColor(LiquidGlass.textSecondary)
            }
            .padding(.vertical, 4)

            Button(action: {
                // Save to CareKit & ResearchKit
                CarePlanStore.shared.recordMCHATSurvey(answers: answers)
                onCompleted?(result)
                dismiss()
            }) {
                HStack {
                    Image(systemName: "square.and.arrow.down.fill")
                    Text("Save to CareKit & Research Vault")
                        .fontWeight(.bold)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(LiquidGlass.celestialTeal)
                .foregroundColor(.black) // theme:fixed-media
                .clipShape(RoundedRectangle(cornerRadius: 14))
            }
        }
        .glassCard(cornerRadius: 18, padding: 18)
    }

    // MARK: - Helpers

    private func selectAnswer(questionId: Int, value: Bool) {
        answers[questionId] = value
        if currentQuestionIndex < questions.count - 1 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
                currentQuestionIndex += 1
            }
        }
    }

    private func calculateAndReview() {
        evaluatedResult = MCHATSurveyEngine.evaluate(answers: answers)
    }

    private func riskColor(_ risk: MCHATRiskLevel) -> Color {
        switch risk {
        case .low: return LiquidGlass.emeraldMint
        case .medium: return LiquidGlass.warmAmber
        case .high: return Color.red
        }
    }
}

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
                                Text("Completed M-CHAT-R/F surveys will appear here with their risk levels.")
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
                    Text(survey.riskLevel.rawValue)
                        .font(.system(size: 14, weight: .bold, design: .rounded))
                        .foregroundColor(riskColor(survey.riskLevel))
                    if survey.requiresFollowUpInterview {
                        Text("Follow-Up Due")
                            .font(.system(size: 10, weight: .bold, design: .rounded))
                            .foregroundColor(LiquidGlass.warmAmber)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 2)
                            .background(Capsule().fill(LiquidGlass.warmAmber.opacity(0.15)))
                    }
                }
                Text("\(survey.totalFailed) of 20 failed • \(survey.completionDate.formatted(date: .abbreviated, time: .shortened))")
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
        case .high: return Color.red
        }
    }
}

extension MCHATScoreResult: Identifiable {
    public var id: String {
        "\(completionDate.timeIntervalSince1970)-\(totalFailed)"
    }
}
