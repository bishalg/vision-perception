import SwiftUI
import CurvPalmOverlay

/**
 * Scientific anthropometric report view.
 *
 * Displays objective biological biometric measurements:
 * - 2D:4D, 2D:3D, 3D:4D digit ratios with statistical context.
 * - Linear phalanx dimensions and total digit lengths (mm).
 * - Palmar morphometry (breadth, length, aspect ratio, shape classification).
 * - 3D anatomical pad elevations.
 * - Raw scientific JSON data export for clinical research (e.g., ASD phenotyping).
 */
public struct AnthropometrySummaryView: View {
    public let landmarks: [CGPoint]
    public let padDepths: [String: Float]
    public let referencePalmWidthMm: Float
    public let handSide: HandSide

    @State private var showingShareSheet: Bool = false
    @State private var exportedJsonString: String = ""

    public init(
        landmarks: [CGPoint],
        padDepths: [String: Float] = [:],
        referencePalmWidthMm: Float = 80.0,
        handSide: HandSide = .right
    ) {
        self.landmarks = landmarks
        self.padDepths = padDepths
        self.referencePalmWidthMm = referencePalmWidthMm
        self.handSide = handSide
    }

    private var report: HandBiometricsReport? {
        HandBiometricsCalculator.compute(
            landmarks: landmarks.map { [Float($0.x), Float($0.y)] },
            referencePalmWidthMm: referencePalmWidthMm,
            padDepthMm: padDepths,
            handSide: handSide.rawValue
        )
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                if let report {
                    digitRatiosCard(report.ratios)
                    palmarMorphometryCard(report.palm)
                    digitKinematicsCard(report.digits)
                    anatomicalPadsCard(report.pads)
                    palmarCreasesCard(report.creases)
                    exportSection(report)
                } else {
                    insufficientDataView
                }
            }
            .padding(16)
        }
        .background(Color(uiColor: .systemGroupedBackground).ignoresSafeArea())
        .sheet(isPresented: $showingShareSheet) {
            ShareSheet(activityItems: [exportedJsonString])
        }
    }

    // MARK: - 1. Digit Ratios (Biomarkers)

    @ViewBuilder
    private func digitRatiosCard(_ ratios: DigitRatios) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Digit Ratios & Biomarkers", systemImage: "chart.bar.xaxis")
                    .font(.headline)
                    .foregroundStyle(Color.primary)
                Spacer()
                Text("2D:4D Biomarker")
                    .font(.caption)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color(hex: "34C759").opacity(0.2))
                    .foregroundStyle(Color(hex: "34C759"))
                    .clipShape(Capsule())
            }

            HStack(alignment: .lastTextBaseline, spacing: 8) {
                Text(String(format: "%.3f", ratios.ratio2D4D))
                    .font(.system(size: 38, weight: .bold, design: .rounded))
                    .foregroundStyle(Color.primary)
                Text("Index (2D) / Ring (4D)")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            // Reference distribution context
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text("Human Range (0.85 – 1.15)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Text("Median ~0.97")
                        .font(.caption2.bold())
                        .foregroundStyle(Color(hex: "34C759"))
                }
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        Capsule()
                            .fill(Color(uiColor: .tertiarySystemFill))
                            .frame(height: 6)

                        // Median tick
                        Rectangle()
                            .fill(Color(hex: "34C759"))
                            .frame(width: 2, height: 10)
                            .offset(x: geo.size.width * 0.40)

                        // Current value marker
                        let normVal = CGFloat(max(0.0, min(1.0, (ratios.ratio2D4D - 0.85) / 0.30)))
                        Circle()
                            .fill(Color.primary)
                            .frame(width: 10, height: 10)
                            .offset(x: max(0, min(geo.size.width - 10, normVal * geo.size.width - 5)))
                    }
                }
                .frame(height: 10)
            }

            Divider()

            // Secondary Ratios Grid
            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 8) {
                GridRow {
                    ratioItem(label: "2D:3D (Index / Middle)", value: ratios.ratio2D3D)
                    ratioItem(label: "3D:4D (Middle / Ring)", value: ratios.ratio3D4D)
                }
                GridRow {
                    ratioItem(label: "4D:5D (Ring / Pinky)", value: ratios.ratio4D5D)
                    ratioItem(label: "Thumb : Index", value: ratios.thumbToIndex)
                }
            }
        }
        .padding(16)
        .background(Color(uiColor: .secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    private func ratioItem(label: String, value: Float) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text(String(format: "%.3f", value))
                .font(.subheadline.monospacedDigit().bold())
                .foregroundStyle(Color.primary)
        }
    }

    // MARK: - 2. Palmar Morphometry

    @ViewBuilder
    private func palmarMorphometryCard(_ palm: PalmDimensions) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Palmar Morphometry", systemImage: "hand.raised")
                .font(.headline)
                .foregroundStyle(Color.primary)

            HStack(spacing: 12) {
                metricCell(
                    title: "Breadth",
                    value: String(format: "%.1f mm", palm.metacarpalBreadthMm),
                    subtitle: "Metacarpal (II–V)"
                )
                metricCell(
                    title: "Palm Length",
                    value: String(format: "%.1f mm", palm.palmLengthMm),
                    subtitle: "Wrist to Middle MCP"
                )
                metricCell(
                    title: "Hand Length",
                    value: String(format: "%.1f mm", palm.totalHandLengthMm),
                    subtitle: "Wrist to D3 Tip"
                )
            }

            HStack(spacing: 12) {
                metricCell(
                    title: "Aspect Ratio",
                    value: String(format: "%.2f", palm.aspectRatio),
                    subtitle: "Breadth / Length"
                )
                metricCell(
                    title: "Classification",
                    value: palm.handShapeType.capitalized,
                    subtitle: "Morphotype"
                )
                metricCell(
                    title: "Surface Area",
                    value: String(format: "%.1f cm²", palm.palmSurfaceAreaCm2),
                    subtitle: "2D Planar Area"
                )
            }
        }
        .padding(16)
        .background(Color(uiColor: .secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    private func metricCell(title: String, value: String, subtitle: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.subheadline.monospacedDigit().bold())
                .foregroundStyle(Color.primary)
            Text(subtitle)
                .font(.system(size: 9))
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(Color(uiColor: .tertiarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - 3. Digit Kinematics

    @ViewBuilder
    private func digitKinematicsCard(_ digits: [DigitMeasurement]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Digit Kinematics & Phalanges", systemImage: "ruler")
                .font(.headline)
                .foregroundStyle(Color.primary)

            ForEach(digits) { d in
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(d.name.capitalized)
                            .font(.subheadline.bold())
                            .foregroundStyle(Color.primary)
                        Spacer()
                        Text(String(format: "%.1f mm", d.totalLengthMm))
                            .font(.subheadline.monospacedDigit().bold())
                            .foregroundStyle(Color(hex: "34C759"))
                        Text(String(format: "(%.0f%%)", d.lengthRelativeToHand * 100))
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }

                    // Phalanges bar
                    HStack(spacing: 2) {
                        Text(String(format: "P: %.1f", d.proximalPhalanxMm))
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(.secondary)
                        Spacer()
                        if d.intermediatePhalanxMm > 0 {
                            Text(String(format: "I: %.1f", d.intermediatePhalanxMm))
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundStyle(.secondary)
                            Spacer()
                        }
                        Text(String(format: "D: %.1f", d.distalPhalanxMm))
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(10)
                .background(Color(uiColor: .tertiarySystemGroupedBackground))
                .clipShape(RoundedRectangle(cornerRadius: 10))
            }
        }
        .padding(16)
        .background(Color(uiColor: .secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    // MARK: - 4. Anatomical Pads

    @ViewBuilder
    private func anatomicalPadsCard(_ pads: [AnatomicalPadMetric]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Anatomical Palmar Pads (3D Topography)", systemImage: "mountain.2")
                .font(.headline)
                .foregroundStyle(Color.primary)

            ForEach(pads) { pad in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(pad.anatomicalPad?.scientificDisplayName ?? pad.padName)
                            .font(.subheadline)
                            .foregroundStyle(Color.primary)
                        if let astro = pad.anatomicalPad?.astrologicalMountName {
                            Text("Traditional: \(astro.capitalized)")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text(String(format: "%+.1f mm", pad.heightMm))
                            .font(.subheadline.monospacedDigit().bold())
                            .foregroundStyle(pad.heightMm >= 0 ? Color(hex: "34C759") : Color(hex: "007AFF"))
                        Text(String(format: "z: %+.2f", pad.relativeProminence))
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(8)
                .background(Color(uiColor: .tertiarySystemGroupedBackground))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
        .padding(16)
        .background(Color(uiColor: .secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    // MARK: - 5. Palmar Flexion Creases (Anatomical)

    @ViewBuilder
    private func palmarCreasesCard(_ creases: [PalmarCreaseMetric]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Palmar Flexure Creases (Anatomical)", systemImage: "waveform.path")
                .font(.headline)
                .foregroundStyle(Color.primary)

            ForEach(creases) { crease in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(crease.scientificDisplayName)
                            .font(.subheadline)
                            .foregroundStyle(Color.primary)
                        Text("Traditional: \(crease.traditionalAlias.capitalized) Line")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text(String(format: "%.1f mm", crease.arcLengthMm))
                            .font(.subheadline.monospacedDigit().bold())
                            .foregroundStyle(Color(hex: "34C759"))
                        Text(String(format: "Tortuosity: %.2f", crease.tortuosity))
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(8)
                .background(Color(uiColor: .tertiarySystemGroupedBackground))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
        .padding(16)
        .background(Color(uiColor: .secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    // MARK: - 6. Export

    @ViewBuilder
    private func exportSection(_ report: HandBiometricsReport) -> some View {
        VStack(spacing: 8) {
            Button {
                if let data = try? JSONEncoder().encode(report),
                   let str = String(data: data, encoding: .utf8) {
                    exportedJsonString = str
                    showingShareSheet = true
                }
            } label: {
                Label("Export Scientific Biometrics (JSON)", systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .tint(Color(hex: "007AFF"))

            Text("Standardized JSON payload for developmental biology, genetics, and clinical research.")
                .font(.caption2)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(.vertical, 8)
    }

    private var insufficientDataView: some View {
        VStack(spacing: 12) {
            Image(systemName: "hand.raised.slash")
                .font(.system(size: 40))
                .foregroundStyle(.secondary)
            Text("Insufficient tracking landmarks to compute anthropometric report.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .padding(32)
    }
}

private struct ShareSheet: UIViewControllerRepresentable {
    let activityItems: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
