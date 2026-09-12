import SwiftUI
import CurvPalmOverlay

/**
 * Liquid Glass bottom sheet presenting sacred Samudrika Shastra palm interpretations.
 * Phase 3 of Palm Scanner workflow:
 * - Pure interpretive reading grounded in user-verified crease geometry
 * - Zero raw CV telemetry on consumer screen
 * - Localized and hand-side contextualized
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

    public var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Drag handle indicator
                Capsule()
                    .fill(Color.white.opacity(0.3))
                    .frame(width: 44, height: 5)
                    .padding(.top, 12)

                // Header & Language Picker
                HStack(alignment: .center) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(LocalizedStrings.tr("sheet.title", lang: selectedLanguage))
                            .font(.system(size: 20, weight: .bold, design: .rounded))
                            .foregroundColor(.white)

                        Text(handSide == .left ? "Left Palm • Inherited Potential" : "Right Palm • Active Manifestation")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundColor(.white.opacity(0.60))
                    }

                    Spacer()

                    // Language Selector
                    Menu {
                        ForEach(AppLanguage.allCases) { lang in
                            Button(action: { onLanguageChange(lang) }) {
                                Text(lang.displayName)
                            }
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "globe")
                            Text(selectedLanguage.displayName)
                        }
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(.white)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(
                            Capsule()
                                .fill(.ultraThinMaterial)
                                .overlay(Capsule().stroke(Color.white.opacity(0.2), lineWidth: 1))
                        )
                    }
                }
                .padding(.horizontal, 20)

                // Crease Line Interpretations (Liquid Glass cards)
                VStack(spacing: 14) {
                    ForEach(splines) { spline in
                        let reading = SimplePalmReading.readingText(for: spline, handSide: handSide)

                        GlassCard {
                            VStack(alignment: .leading, spacing: 10) {
                                HStack(spacing: 10) {
                                    Circle()
                                        .fill(Color(hex: spline.lineType.hexColor))
                                        .frame(width: 12, height: 12)
                                        .shadow(color: Color(hex: spline.lineType.hexColor).opacity(0.6), radius: 6)

                                    Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                        .font(.system(size: 16, weight: .bold, design: .rounded))
                                        .foregroundColor(.white)

                                    if spline.isUserAdjusted {
                                        Text("verified")
                                            .font(.system(size: 10, weight: .bold))
                                            .foregroundColor(Color(hex: "34C759"))
                                            .padding(.horizontal, 6)
                                            .padding(.vertical, 2)
                                            .background(Capsule().fill(Color(hex: "34C759").opacity(0.18)))
                                    }

                                    Spacer()

                                    Text("\(Int(spline.confidence * 100))% clear")
                                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                        .foregroundColor(Color(hex: "34C759"))
                                }

                                Text(reading)
                                    .font(.system(size: 14, weight: .regular))
                                    .foregroundColor(.white.opacity(0.92))
                                    .lineSpacing(4)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
                .padding(.horizontal, 20)

                // Actions: Done and Retake
                VStack(spacing: 10) {
                    Button(action: { dismiss() }) {
                        Text(LocalizedStrings.tr("result.doneEdit", lang: selectedLanguage))
                            .font(.system(size: 16, weight: .bold))
                            .foregroundColor(.black)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                            .background(Capsule().fill(Color(hex: "34C759")))
                            .shadow(color: Color(hex: "34C759").opacity(0.35), radius: 8, y: 2)
                    }

                    Button(action: {
                        dismiss()
                        onRetake()
                    }) {
                        HStack(spacing: 6) {
                            Image(systemName: "arrow.counterclockwise")
                            Text(LocalizedStrings.tr("button.retake", lang: selectedLanguage))
                        }
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(.white.opacity(0.8))
                        .padding(.vertical, 8)
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 10)
                .padding(.bottom, 36)
            }
        }
        .background(
            Color.black.opacity(0.88)
                .background(.ultraThinMaterial)
                .ignoresSafeArea()
        )
    }
}

/**
 * Reusable Liquid Glass card component.
 */
public struct GlassCard<Content: View>: View {
    let content: Content

    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        content
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .fill(Color.white.opacity(0.06))
                    .background(.ultraThinMaterial)
                    .overlay(
                        RoundedRectangle(cornerRadius: 20, style: .continuous)
                            .stroke(
                                LinearGradient(
                                    colors: [Color.white.opacity(0.22), Color.white.opacity(0.05)],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                ),
                                lineWidth: 1
                            )
                    )
            )
    }
}
