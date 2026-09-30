import SwiftUI
import CurvPalmOverlay

/**
 * Mount detail sheet: region identification + measured relief + matching
 * Samudrika reading for one selected mount.
 *
 * Presented from the 3D Topography inspector's mount chips. Shows:
 * - Region: the captured palm photo (or a schematic fallback) with every mount
 *   polygon faintly drawn and the selected mount highlighted — the polygons
 *   come from `PalmRegionGeometry.regions(from:)` in the same normalized
 *   landmark space the capture pipeline uses, so identification is exact.
 * - Data: absolute relief (mm), relative height, prominence classification,
 *   estimator confidence, and how many orbit views contributed.
 * - Reading: development level derived from the measured values, matched to
 *   the per-mount Samudrika interpretation in `MountAstrology`.
 */
public struct MountDetailSheet: View {
    public let mount: PalmMount
    public let entry: MountReliefEntry?
    /// Fallback depth in mm (the padDepths map) when no relief entry exists.
    public let fallbackMm: Float?
    public let capturedImage: UIImage?
    public let landmarks: [CGPoint]
    public let handSide: HandSide

    @Environment(\.dismiss) private var dismiss

    public init(
        mount: PalmMount,
        entry: MountReliefEntry?,
        fallbackMm: Float? = nil,
        capturedImage: UIImage? = nil,
        landmarks: [CGPoint] = [],
        handSide: HandSide = .right
    ) {
        self.mount = mount
        self.entry = entry
        self.fallbackMm = fallbackMm
        self.capturedImage = capturedImage
        self.landmarks = landmarks
        self.handSide = handSide
    }

    private var profile: MountProfile? { MountAstrology.profile(for: mount.rawValue) }

    private var heightMm: Float? { entry?.heightMm ?? fallbackMm }

    private var development: MountDevelopment? {
        MountAstrology.development(
            heightMm: heightMm,
            prominence: entry?.prominence,
            heightRelative: entry.map { $0.heightRelative }
        )
    }

    public var body: some View {
        NavigationStack {
            List {
                if let profile {
                    regionSection(profile)
                    dataSection(profile)
                    readingSection(profile)
                    safetySection
                } else {
                    Text("No profile for \(mount.rawValue).")
                }
            }
            .listStyle(.insetGrouped)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    HStack(spacing: 6) {
                        if let glyph = profile?.planetGlyph {
                            Text(glyph)
                                .font(.title3)
                                .foregroundStyle(Color(hex: "FF9500"))
                        }
                        Text(profile?.displayName ?? mount.rawValue)
                            .font(.headline)
                    }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { dismiss() }) {
                        Text("Done")
                    }
                }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }

    // MARK: - Region identification

    private func regionSection(_ profile: MountProfile) -> some View {
        Section {
            VStack(alignment: .leading, spacing: 10) {
                MountRegionOverlay(
                    selected: mount,
                    landmarks: landmarks,
                    capturedImage: capturedImage
                )
                .frame(height: 260)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))

                Label(profile.anatomicalLocation, systemImage: "scope")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .padding(.vertical, 4)
        } header: {
            Text("Region Identified")
        }
    }

    // MARK: - Measured relief

    private func dataSection(_ profile: MountProfile) -> some View {
        Section("Measured Relief") {
            HStack {
                dataLabel("Absolute height")
                Spacer()
                if let mm = heightMm {
                    Text(String(format: "%+.1f mm", mm))
                        .font(.body.monospacedDigit().weight(.semibold))
                } else {
                    Text("not measured")
                        .font(.body.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }

            if let mm = heightMm {
                ReliefGauge(mm: mm)
                    .frame(height: 14)
                    .padding(.vertical, 2)
            }

            if let entry {
                HStack {
                    dataLabel("Relative height")
                    Spacer()
                    Text("\(Int((entry.heightRelative * 100).rounded()))%")
                        .font(.body.monospacedDigit())
                }
                HStack {
                    dataLabel("Prominence")
                    Spacer()
                    Text(entry.prominence.rawValue.capitalized)
                        .font(.body.weight(.semibold))
                        .foregroundStyle(prominenceColor(entry.prominence))
                }
                HStack {
                    dataLabel("Confidence")
                    Spacer()
                    Text("\(Int((entry.confidence * 100).rounded()))%")
                        .font(.body.monospacedDigit())
                }
                HStack {
                    dataLabel("Views analyzed")
                    Spacer()
                    Text("\(entry.viewCount)")
                        .font(.body.monospacedDigit())
                }
            }

            HStack {
                dataLabel("Development")
                Spacer()
                if let dev = development {
                    Text(dev.label)
                        .font(.body.weight(.semibold))
                        .foregroundStyle(developmentColor(dev))
                } else {
                    Text("insufficient data")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    // MARK: - Matching reading

    private func readingSection(_ profile: MountProfile) -> some View {
        Section {
            VStack(alignment: .leading, spacing: 8) {
                if let dev = development {
                    HStack(spacing: 8) {
                        Text(dev.label)
                            .font(.caption.weight(.bold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Capsule().fill(developmentColor(dev).opacity(0.16)))
                            .foregroundStyle(developmentColor(dev))
                        Text(profile.planetGlyph)
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(.secondary)
                    }
                }
                Text(profile.reading(for: development ?? .balanced))
                    .font(.body)
                    .fixedSize(horizontal: false, vertical: true)
                Text("Governs: \(profile.governs)")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.top, 2)
            }
            .padding(.vertical, 4)
        } header: {
            Text("Samudrika Reading")
        }
    }

    private var safetySection: some View {
        Section {
            PalmReadingSafetyNotice(selectedLanguage: .english)
        }
    }

    // MARK: - Helpers

    private func dataLabel(_ text: String) -> some View {
        Text(text).font(.subheadline).foregroundStyle(.secondary)
    }

    private func developmentColor(_ level: MountDevelopment) -> Color {
        level.accentColor
    }

    private func prominenceColor(_ p: MountProminence) -> Color {
        switch p {
        case .deficient: return Color(hex: "007AFF")
        case .average: return Color(hex: "34C759")
        case .elevated: return Color(hex: "FF9500")
        }
    }
}

// MARK: - Shared development color (used by report rows + detail sheet)

extension MountDevelopment {
    var accentColor: Color {
        switch self {
        case .deficient: return Color(hex: "007AFF")
        case .balanced: return Color(hex: "34C759")
        case .developed: return Color(hex: "FF9500")
        case .prominent: return Color(hex: "FF2D55")
        }
    }
}

// MARK: - Relief gauge

/// Horizontal scale from -2.5 mm (recessed) to +4.0 mm (high), marker at the
/// measured value. Matches the clamp domain used by the mesh builder.
struct ReliefGauge: View {
    let mm: Float

    private let domain: ClosedRange<Float> = -2.5...4.0

    var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let markerX = width * CGFloat((mm - domain.lowerBound) / (domain.upperBound - domain.lowerBound))
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(LinearGradient(
                        colors: [Color(hex: "007AFF"), Color(hex: "34C759"), Color(hex: "FF9500"), Color(hex: "FF2D55")],
                        startPoint: .leading, endPoint: .trailing
                    ))
                    .opacity(0.35)
                Capsule()
                    .fill(Color.primary)
                    .frame(width: 3, height: 16)
                    .offset(x: max(0, min(width - 3, markerX)))
            }
        }
    }
}

// MARK: - Region overlay

/// Draws all mount polygons faintly and the selected mount highlighted, over
/// the captured photo when available. Uses the same normalized landmark space
/// as the capture pipeline, mapped into the view with ImageCoordinateMapper.
struct MountRegionOverlay: View {
    let selected: PalmMount
    let landmarks: [CGPoint]
    let capturedImage: UIImage?

    var body: some View {
        GeometryReader { proxy in
            let imageRect = ImageCoordinateMapper.imageRect(
                imageSize: capturedImage?.size ?? proxy.size,
                viewSize: proxy.size,
                mode: .fit
            )
            ZStack {
                Color.black
                if let capturedImage {
                    Image(uiImage: capturedImage)
                        .resizable()
                        .frame(width: imageRect.width, height: imageRect.height)
                        .position(x: imageRect.midX, y: imageRect.midY)
                        .saturation(0.75)
                }
                MountPolygonCanvas(
                    selected: selected,
                    landmarks: landmarks,
                    drawRect: imageRect
                )
            }
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
    }
}

private struct MountPolygonCanvas: View {
    let selected: PalmMount
    let landmarks: [CGPoint]
    let drawRect: CGRect

    var body: some View {
        Canvas { context, _ in
            guard let regions = PalmRegionGeometry.regions(from: landmarks) else { return }

            // All mounts: faint outline. Selected: filled highlight.
            for (mount, polygon) in regions.mounts {
                var path = Path()
                let mapped = polygon.map { ImageCoordinateMapper.map($0, into: drawRect) }
                guard let first = mapped.first else { continue }
                path.move(to: first)
                mapped.dropFirst().forEach { path.addLine(to: $0) }
                path.closeSubpath()

                if mount == selected {
                    context.fill(path, with: .color(Color(hex: "FF9500").opacity(0.28)))
                    context.stroke(path, with: .color(Color(hex: "FF9500")), lineWidth: 2.2)
                } else {
                    context.stroke(path, with: .color(Color.white.opacity(0.22)),
                                   style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
                }
            }

            // Landmark dots for context.
            for pt in landmarks.prefix(21) {
                let mapped = ImageCoordinateMapper.map(pt, into: drawRect)
                let r = CGRect(x: mapped.x - 2, y: mapped.y - 2, width: 4, height: 4)
                context.fill(Path(ellipseIn: r), with: .color(Color.white.opacity(0.5)))
            }

            // Name label at the selected mount's centroid.
            let polygon = regions.mountPolygon(selected)
            guard !polygon.isEmpty else { return }
            let centroid = polygon.map { ImageCoordinateMapper.map($0, into: drawRect) }
                .reduce(CGPoint.zero) { CGPoint(x: $0.x + $1.x / CGFloat(polygon.count),
                                                y: $0.y + $1.y / CGFloat(polygon.count)) }
            let label = MountAstrology.profile(for: selected.rawValue)?.displayName ?? selected.rawValue
            context.draw(
                Text(label).font(.caption.weight(.bold)).foregroundColor(.white),
                at: centroid
            )
        }
        .allowsHitTesting(false)
    }
}
