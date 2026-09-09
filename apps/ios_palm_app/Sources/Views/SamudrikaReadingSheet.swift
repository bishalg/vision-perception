import SwiftUI

/**
 * Liquid Glass bottom sheet presenting Samudrika Shastra palm metrics and Vastu insights.
 */
public struct SamudrikaReadingSheet: View {
    public let result: PalmAnalysisResponse?
    public let splines: [CreaseSpline]
    public let selectedLanguage: AppLanguage
    public let onLanguageChange: (AppLanguage) -> Void
    public let onRetake: () -> Void

    public init(
        result: PalmAnalysisResponse?,
        splines: [CreaseSpline],
        selectedLanguage: AppLanguage,
        onLanguageChange: @escaping (AppLanguage) -> Void,
        onRetake: @escaping () -> Void
    ) {
        self.result = result
        self.splines = splines
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
                    .padding(.top, 10)

                // Header & Language Picker
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(LocalizedStrings.tr("sheet.title", lang: selectedLanguage))
                            .font(.system(size: 20, weight: .bold, design: .rounded))
                            .foregroundColor(.white)
                        if let dur = result?.totalDurationUs {
                            Text("Engine Latency: \(dur / 1000) ms")
                                .font(.system(size: 12, weight: .regular, design: .monospaced))
                                .foregroundColor(.white.opacity(0.6))
                        }
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
                .padding(.horizontal)

                // Crease Line Analysis Cards (Liquid Glass cards)
                VStack(spacing: 12) {
                    ForEach(splines) { spline in
                        GlassCard {
                            HStack(spacing: 14) {
                                Circle()
                                    .fill(Color(hex: spline.lineType.hexColor))
                                    .frame(width: 14, height: 14)
                                    .shadow(color: Color(hex: spline.lineType.hexColor).opacity(0.6), radius: 6)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                        .font(.system(size: 16, weight: .semibold))
                                        .foregroundColor(.white)

                                    HStack(spacing: 10) {
                                        Text("Length: \(String(format: "%.1f", spline.arcLengthPx)) px")
                                        Text("Tortuosity: \(String(format: "%.2f", spline.tortuosity))")
                                    }
                                    .font(.system(size: 12, weight: .regular, design: .monospaced))
                                    .foregroundColor(.white.opacity(0.65))
                                }

                                Spacer()

                                Text("\(Int(spline.confidence * 100))%")
                                    .font(.system(size: 15, weight: .bold, design: .rounded))
                                    .foregroundColor(Color(red: 0.20, green: 0.84, blue: 0.29))
                            }
                        }
                    }
                }
                .padding(.horizontal)

                // Retake Button
                Button(action: onRetake) {
                    HStack {
                        Image(systemName: "arrow.counterclockwise")
                        Text(LocalizedStrings.tr("button.retake", lang: selectedLanguage))
                    }
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .fill(.ultraThinMaterial)
                            .overlay(
                                RoundedRectangle(cornerRadius: 18, style: .continuous)
                                    .stroke(Color.white.opacity(0.25), lineWidth: 1)
                            )
                    )
                }
                .padding(.horizontal)
                .padding(.top, 10)
                .padding(.bottom, 30)
            }
        }
        .background(
            Color.black.opacity(0.85)
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
            .padding(14)
            .background(
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .fill(Color.white.opacity(0.06))
                    .background(.ultraThinMaterial)
                    .overlay(
                        RoundedRectangle(cornerRadius: 20, style: .continuous)
                            .stroke(
                                LinearGradient(
                                    colors: [Color.white.opacity(0.25), Color.white.opacity(0.05)],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                ),
                                lineWidth: 1
                            )
                    )
            )
    }
}
