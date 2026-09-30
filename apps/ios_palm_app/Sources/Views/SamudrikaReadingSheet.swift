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
    /// 3D analysis inputs for the topography report (analysis → detail report).
    public let mountReliefMap: MountReliefMap?
    public let padDepths: [String: Float]
    public let landmarks: [CGPoint]
    public let capturedImage: UIImage?

    @Environment(\.dismiss) private var dismiss
    @State private var selectedReportMount: PalmMount?

    public init(
        result: PalmAnalysisResponse?,
        splines: [CreaseSpline],
        handSide: HandSide = .right,
        selectedLanguage: AppLanguage,
        onLanguageChange: @escaping (AppLanguage) -> Void,
        onRetake: @escaping () -> Void,
        mountReliefMap: MountReliefMap? = nil,
        padDepths: [String: Float] = [:],
        landmarks: [CGPoint] = [],
        capturedImage: UIImage? = nil
    ) {
        self.result = result
        self.splines = splines
        self.handSide = handSide
        self.selectedLanguage = selectedLanguage
        self.onLanguageChange = onLanguageChange
        self.onRetake = onRetake
        self.mountReliefMap = mountReliefMap
        self.padDepths = padDepths
        self.landmarks = landmarks
        self.capturedImage = capturedImage
    }

    /// The 3D analysis converted into the astrological detail report; present
    /// whenever any 3D relief data exists (relief map or pad-depth fallbacks).
    private var topographyReport: PalmReport? {
        guard mountReliefMap != nil || !padDepths.isEmpty else { return nil }
        return PalmReportGenerator.generate(
            mountReliefMap: mountReliefMap,
            padDepths: padDepths,
            landmarks: landmarks,
            handSide: handSide
        )
    }

    private var handSubtitle: String {
        handSide == .left ? "Left Palm • Inherited Potential" : "Right Palm • Active Manifestation"
    }

    // MARK: - 3D Topography Report (analysis → detail report)

    @ViewBuilder
    private func topographyReportSection(_ report: PalmReport) -> some View {
        Section {
            // Synthesis card
            VStack(alignment: .leading, spacing: 8) {
                Text(report.handContext)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)

                if let shape = report.handShapeLabel, let shapeReading = report.handShapeReading {
                    reportChip(label: shape, systemImage: "hand.draw")
                    Text(shapeReading)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let ratio = report.ratio2D4D, let ratioReading = report.ratio2D4DReading {
                    reportChip(label: String(format: "2D:4D %.2f", ratio), systemImage: "scalemass")
                    Text(ratioReading)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }

                Text(report.synthesis)
                    .font(.body)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 2)
            }
            .padding(.vertical, 4)
        } header: {
            Text("3D Topography Report")
        }

        Section {
            ForEach(report.mountReports) { mount in
                Button {
                    selectedReportMount = PalmMount(rawValue: mount.id)
                } label: {
                    mountReportRow(mount)
                }
                .buttonStyle(.plain)
            }

            if let note = report.dataQualityNote {
                Label(note, systemImage: "info.circle")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.vertical, 2)
            }
        } header: {
            Text("Mounts — Strongest First")
        } footer: {
            Text("Tap a mount for its region map, full relief data, and detailed reading.")
        }
    }

    private func mountReportRow(_ mount: PalmMountReport) -> some View {
        HStack(spacing: 10) {
            Text(mount.planetGlyph)
                .font(.title3)
                .frame(width: 30)
                .foregroundStyle(mount.development.map { $0.accentColor } ?? .secondary)

            VStack(alignment: .leading, spacing: 2) {
                Text(mount.displayName)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.primary)
                if let dev = mount.development {
                    Text(dev.label)
                        .font(.caption.weight(.bold))
                        .foregroundStyle(dev.accentColor)
                } else {
                    Text("Not measured")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            if let mm = mount.heightMm {
                Text(String(format: "%+.1f mm", mm))
                    .font(.subheadline.monospacedDigit().weight(.semibold))
                    .foregroundStyle(.primary)
            }
            if !mount.isReliable && mount.isMeasured {
                Image(systemName: "exclamationmark.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Low confidence")
            }
            Image(systemName: "chevron.right")
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 2)
    }

    private func reportChip(label: String, systemImage: String) -> some View {
        Label(label, systemImage: systemImage)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(Capsule().fill(Color(hex: "34C759").opacity(0.14)))
            .foregroundStyle(Color(hex: "34C759"))
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    public var body: some View {
        NavigationStack {
            List {
                Section {
                    PalmReadingSafetyNotice(selectedLanguage: selectedLanguage)
                }
                if let report = topographyReport {
                    topographyReportSection(report)
                }
                if splines.isEmpty && topographyReport == nil {
                    Section {
                        VStack(alignment: .leading, spacing: 8) {
                            Label(LocalizedStrings.tr("sheet.emptyTitle", lang: selectedLanguage), systemImage: "hand.raised")
                                .font(.headline)
                            Text(LocalizedStrings.tr("sheet.emptyBody", lang: selectedLanguage))
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .padding(.vertical, 4)
                    }
                }
                #if DEBUG
                if !splines.isEmpty, let summary = SimplePalmReading.scanSummary(from: .from(payload: result?.domainPayload)) {
                    Section {
                        Text(summary)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    } header: {
                        Text("3D Scan Summary")
                    }
                }
                #endif
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
            .sheet(item: $selectedReportMount) { mount in
                MountDetailSheet(
                    mount: mount,
                    entry: mountReliefMap?.entry(for: mount),
                    fallbackMm: padDepths[mount.rawValue],
                    capturedImage: capturedImage,
                    landmarks: landmarks,
                    handSide: handSide
                )
            }
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
                // English-only by product decision (user request 2026-09-29):
                // the language menu was removed so the reading can never
                // switch to Nepali/Hindi. The AppLanguage plumbing stays for
                // the shared strings file and a possible future setting.
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

/// Shared, visible before capture and before any interpretive reading.
struct PalmReadingSafetyNotice: View {
    let selectedLanguage: AppLanguage

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(LocalizedStrings.tr("safety.title", lang: selectedLanguage), systemImage: "info.circle")
                .font(.subheadline.weight(.semibold))
            Text(LocalizedStrings.tr("safety.disclaimer", lang: selectedLanguage))
                .font(.subheadline)
                .fixedSize(horizontal: false, vertical: true)
            if let url = URL(string: "https://astro-fusion.com/privacy") {
                Link(LocalizedStrings.tr("safety.privacy", lang: selectedLanguage), destination: url)
                    .font(.subheadline.weight(.semibold))
                    .padding(.vertical, 6)
            }
        }
        .foregroundStyle(.primary)
    }
}
