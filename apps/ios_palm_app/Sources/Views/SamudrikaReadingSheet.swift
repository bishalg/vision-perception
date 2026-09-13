import SwiftUI
import CurvPalmOverlay

/**
 * Samudrika Shastra palm interpretations.
 * Phase 3 of Palm Scanner workflow:
 * - Native grouped list: one section per crease line, headline + reading body.
 * - Presented as a native sheet (detents + drag indicator set by the presenter).
 * - Adaptive system colors: follows light/dark mode like any native document.
 * - Pure interpretive reading grounded in user-verified crease geometry.
 * - Zero raw CV telemetry on consumer screen.
 * - Localized and hand-side contextualized.
 */
public struct SamudrikaReadingSheet: View {
    public let result: PalmAnalysisResponse?
    public let splines: [CreaseSpline]
    public let handSide: HandSide
    public let selectedLanguage: AppLanguage
    public let onLanguageChange: (AppLanguage) -> Void
    public let onRetake: () -> Void

    @Environment(\.dismiss) private var dismiss

    public init(
        result: PalmAnalysisResponse?,
        splines: [CreaseSpline],
        handSide: HandSide = .right,
        selectedLanguage: AppLanguage,
        onLanguageChange: @escaping (AppLanguage) -> Void,
        onRetake: @escaping () -> Void
    ) {
        self.result = result
        self.splines = splines
        self.handSide = handSide
        self.selectedLanguage = selectedLanguage
        self.onLanguageChange = onLanguageChange
        self.onRetake = onRetake
    }

    private var handSubtitle: String {
        handSide == .left ? "Left Palm • Inherited Potential" : "Right Palm • Active Manifestation"
    }

    public var body: some View {
        NavigationStack {
            List {
                if let summary = SimplePalmReading.scanSummary(from: .from(payload: result?.domainPayload)) {
                    Section {
                        Text(summary)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    } header: {
                        Text("3D Scan Summary")
                    }
                }
                ForEach(splines) { spline in
                    Section {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 8) {
                                Circle()
                                    .fill(Color(hex: spline.lineType.hexColor))
                                    .frame(width: 12, height: 12)
                                Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                    .font(.headline)
                                Spacer()
                                Text("\(Int(spline.confidence * 100))% clear")
                                    .font(.subheadline.monospacedDigit())
                                    .foregroundStyle(.secondary)
                            }
                            Text(SimplePalmReading.readingText(
                                for: spline,
                                handSide: handSide,
                                reports: .from(payload: result.flatMap { $0.domainPayload })
                            ))
                                .font(.body)
                                .foregroundStyle(.primary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .padding(.vertical, 4)
                    } header: {
                        HStack {
                            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                            if spline.isUserAdjusted {
                                Spacer()
                                Text(LocalizedStrings.tr("result.verified", lang: selectedLanguage))
                                    .font(.caption)
                                    .foregroundStyle(Color(hex: "34C759"))
                                    .textCase(nil)
                            }
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 2) {
                        Text(LocalizedStrings.tr("sheet.title", lang: selectedLanguage))
                            .font(.headline)
                        Text(handSubtitle)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                ToolbarItem(placement: .primaryAction) {
                    Menu {
                        ForEach(AppLanguage.allCases) { lang in
                            Button(action: { onLanguageChange(lang) }) {
                                if selectedLanguage == lang {
                                    Label(lang.displayName, systemImage: "checkmark")
                                } else {
                                    Text(lang.displayName)
                                }
                            }
                        }
                    } label: {
                        Label(selectedLanguage.displayName, systemImage: "globe")
                    }
                }
            }
            .safeAreaInset(edge: .bottom) {
                HStack(spacing: 12) {
                    Button(action: {
                        dismiss()
                        onRetake()
                    }) {
                        Label(LocalizedStrings.tr("button.retake", lang: selectedLanguage), systemImage: "arrow.counterclockwise")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.large)

                    Button(action: { dismiss() }) {
                        Text(LocalizedStrings.tr("result.doneEdit", lang: selectedLanguage))
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .tint(Color(hex: "34C759"))
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 12)
                .background(.bar)
            }
        }
        .tint(Color(hex: "34C759"))
    }
}
