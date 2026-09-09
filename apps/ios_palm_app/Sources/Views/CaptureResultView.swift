import SwiftUI
import UniformTypeIdentifiers

/**
 * Post-capture review: photo + editable crease overlays.
 * Supports drag vertices, reorder, and reassign labels so prediction can use the user-adjusted lines.
 */
public struct CaptureResultView: View {
    public let image: UIImage?
    public let landmarks: [CGPoint]
    @Binding public var splines: [CreaseSpline]
    public let selectedLanguage: AppLanguage
    public let overlaySource: CreaseOverlaySource
    public let onLanguageChange: (AppLanguage) -> Void
    public let onRetake: () -> Void
    public let onConfirmAdjustedLines: () -> Void

    @State private var isEditing = false
    @State private var selectedSplineId: String?
    @State private var editMode: EditMode = .inactive

    public init(
        image: UIImage?,
        landmarks: [CGPoint],
        splines: Binding<[CreaseSpline]>,
        selectedLanguage: AppLanguage,
        overlaySource: CreaseOverlaySource,
        onLanguageChange: @escaping (AppLanguage) -> Void,
        onRetake: @escaping () -> Void,
        onConfirmAdjustedLines: @escaping () -> Void
    ) {
        self.image = image
        self.landmarks = landmarks
        self._splines = splines
        self.selectedLanguage = selectedLanguage
        self.overlaySource = overlaySource
        self.onLanguageChange = onLanguageChange
        self.onRetake = onRetake
        self.onConfirmAdjustedLines = onConfirmAdjustedLines
    }

    private var sourceSubtitle: String {
        switch overlaySource {
        case .nativeEngine:
            return LocalizedStrings.tr("result.native", lang: selectedLanguage)
        case .landmarkFallback:
            return LocalizedStrings.tr("result.landmark", lang: selectedLanguage)
        }
    }

    public var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                GeometryReader { proxy in
                    let imageSize = image?.size ?? proxy.size
                    let drawRect = ImageCoordinateMapper.imageRect(
                        imageSize: imageSize,
                        viewSize: proxy.size,
                        mode: .fit
                    )

                    ZStack {
                        Color.black

                        if let image {
                            Image(uiImage: image)
                                .resizable()
                                .frame(width: drawRect.width, height: drawRect.height)
                                .position(x: drawRect.midX, y: drawRect.midY)
                        } else {
                            Text("No photo")
                                .foregroundColor(.white.opacity(0.5))
                        }

                        LinearGradient(
                            colors: [Color.black.opacity(0.25), Color.clear, Color.black.opacity(0.35)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        .allowsHitTesting(false)

                        HandOutlineOverlay(landmarks: landmarks, imageRect: drawRect)
                            .allowsHitTesting(false)

                        CreaseSplineOverlayView(
                            splines: splines,
                            imageRect: drawRect,
                            selectedSplineId: selectedSplineId,
                            isEditing: isEditing,
                            onSelect: { selectedSplineId = $0 },
                            onMoveVertex: { id, index, point in
                                moveVertex(splineId: id, index: index, to: point)
                            }
                        )
                    }
                }
                .frame(maxHeight: .infinity)

                VStack(spacing: 12) {
                    HStack(alignment: .top) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(LocalizedStrings.tr("result.title", lang: selectedLanguage))
                                .font(.system(size: 18, weight: .bold, design: .rounded))
                                .foregroundColor(.white)
                            Text(sourceSubtitle)
                                .font(.system(size: 12, weight: .regular))
                                .foregroundColor(.white.opacity(0.55))
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer()
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

                    // Tools: edit
                    HStack(spacing: 8) {
                        Button {
                            isEditing.toggle()
                            if isEditing {
                                selectedSplineId = selectedSplineId ?? splines.first?.id
                                editMode = .active
                            } else {
                                editMode = .inactive
                            }
                        } label: {
                            Label(
                                isEditing
                                    ? LocalizedStrings.tr("result.doneEdit", lang: selectedLanguage)
                                    : LocalizedStrings.tr("result.edit", lang: selectedLanguage),
                                systemImage: isEditing ? "checkmark" : "slider.horizontal.3"
                            )
                            .font(.system(size: 13, weight: .semibold))
                        }
                        .buttonStyle(ResultChipButtonStyle(emphasized: isEditing))
                    }

                    if isEditing {
                        Text(LocalizedStrings.tr("result.editHint", lang: selectedLanguage))
                            .font(.system(size: 11))
                            .foregroundColor(.white.opacity(0.5))
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }

                    List {
                        ForEach(splines) { spline in
                            lineRow(spline)
                                .listRowBackground(Color.clear)
                                .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
                                .contentShape(Rectangle())
                                .onTapGesture {
                                    selectedSplineId = spline.id
                                    if !isEditing { isEditing = true }
                                }
                        }
                        .onMove(perform: moveLines)
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .environment(\.editMode, $editMode)
                    .frame(maxHeight: min(CGFloat(splines.count) * 44 + 8, 200))

                    if splines.contains(where: \.isUserAdjusted) || overlaySource != .landmarkFallback {
                        Button(action: onConfirmAdjustedLines) {
                            HStack {
                                Image(systemName: "sparkles")
                                Text(LocalizedStrings.tr("result.useAdjusted", lang: selectedLanguage))
                            }
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(.black)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                            .background(
                                Capsule().fill(Color(hex: "34C759"))
                            )
                        }
                    }

                    Button(action: onRetake) {
                        HStack {
                            Image(systemName: "arrow.counterclockwise")
                            Text(LocalizedStrings.tr("result.retake", lang: selectedLanguage))
                        }
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(.white)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(
                            Capsule()
                                .fill(Color.black.opacity(0.55))
                                .overlay(Capsule().stroke(Color.white.opacity(0.35), lineWidth: 1))
                        )
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 14)
                .padding(.bottom, 28)
                .background(
                    RoundedRectangle(cornerRadius: 24, style: .continuous)
                        .fill(Color(white: 0.12))
                        .ignoresSafeArea(edges: .bottom)
                )
            }
        }
        }

    @ViewBuilder
    private func lineRow(_ spline: CreaseSpline) -> some View {
        HStack(spacing: 12) {
            Capsule()
                .fill(Color(hex: spline.lineType.hexColor))
                .frame(width: 18, height: 4)
                .shadow(color: Color(hex: spline.lineType.hexColor).opacity(0.7), radius: 4)

            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                .font(.system(size: 15, weight: .semibold))
                .foregroundColor(.white)

            if spline.isUserAdjusted {
                Text("•")
                    .foregroundColor(Color(hex: "34C759"))
                Text(LocalizedStrings.tr("result.adjusted", lang: selectedLanguage))
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(Color(hex: "34C759"))
            }

            Spacer()

            if isEditing {
                Menu {
                    ForEach(CreaseLineType.allCases) { type in
                        Button(LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage)) {
                            reassign(splineId: spline.id, to: type)
                        }
                    }
                } label: {
                    Image(systemName: "tag")
                        .foregroundColor(.white.opacity(0.8))
                }
            }

            Text("\(Int(spline.confidence * 100))%")
                .font(.system(size: 14, weight: .bold, design: .rounded))
                .foregroundColor(Color(hex: "34C759"))
        }
        .padding(.vertical, 4)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(spline.id == selectedSplineId ? Color.white.opacity(0.08) : Color.clear)
        )
    }

    private func moveVertex(splineId: String, index: Int, to point: CGPoint) {
        guard let i = splines.firstIndex(where: { $0.id == splineId }) else { return }
        guard splines[i].points.indices.contains(index) else { return }
        var pts = splines[i].points
        pts[index] = point
        splines[i] = splines[i].replacing(points: pts)
    }

    private func moveLines(from source: IndexSet, to destination: Int) {
        splines.move(fromOffsets: source, toOffset: destination)
        for i in splines.indices {
            splines[i].isUserAdjusted = true
        }
    }

    private func reassign(splineId: String, to type: CreaseLineType) {
        guard let i = splines.firstIndex(where: { $0.id == splineId }) else { return }
        // Keep geometry; swap label with any existing line of that type.
        if let j = splines.firstIndex(where: { $0.lineType == type && $0.id != splineId }) {
            let oldType = splines[i].lineType
            splines[i] = splines[i].replacing(lineType: type)
            splines[j] = splines[j].replacing(lineType: oldType)
        } else {
            splines[i] = splines[i].replacing(lineType: type)
        }
    }
}

private struct ResultChipButtonStyle: ButtonStyle {
    var emphasized: Bool = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundColor(emphasized ? .black : .white)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(
                Capsule()
                    .fill(emphasized ? Color(hex: "34C759") : Color.white.opacity(0.12))
                    .overlay(Capsule().stroke(Color.white.opacity(0.2), lineWidth: 1))
            )
            .opacity(configuration.isPressed ? 0.75 : 1)
    }
}

/**
 * Soft hand silhouette outline from the 21 capture landmarks.
 */
public struct HandOutlineOverlay: View {
    public let landmarks: [CGPoint]
    public let imageRect: CGRect?

    public init(landmarks: [CGPoint], imageRect: CGRect? = nil) {
        self.landmarks = landmarks
        self.imageRect = imageRect
    }

    public var body: some View {
        Canvas { context, size in
            guard landmarks.count >= 21 else { return }

            let rect = imageRect ?? CGRect(origin: .zero, size: size)
            let mapped = landmarks.map { ImageCoordinateMapper.map($0, into: rect) }

            let hullIdx = [0, 1, 2, 4, 8, 12, 16, 20, 17, 0]
            var hull = Path()
            for (i, idx) in hullIdx.enumerated() {
                let pt = mapped[idx]
                if i == 0 { hull.move(to: pt) } else { hull.addLine(to: pt) }
            }
            context.stroke(
                hull,
                with: .color(Color.white.opacity(0.35)),
                style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round)
            )

            let chains: [[Int]] = [
                [0, 1, 2, 3, 4],
                [0, 5, 6, 7, 8],
                [0, 9, 10, 11, 12],
                [0, 13, 14, 15, 16],
                [0, 17, 18, 19, 20]
            ]
            for chain in chains {
                var path = Path()
                for (i, idx) in chain.enumerated() {
                    let pt = mapped[idx]
                    if i == 0 { path.move(to: pt) } else { path.addLine(to: pt) }
                }
                context.stroke(
                    path,
                    with: .color(Color.white.opacity(0.18)),
                    style: StrokeStyle(lineWidth: 1.0, lineCap: .round, lineJoin: .round)
                )
            }

            for pt in mapped {
                let r = CGRect(x: pt.x - 2.5, y: pt.y - 2.5, width: 5, height: 5)
                context.fill(Path(ellipseIn: r), with: .color(Color.white.opacity(0.55)))
            }
        }
        .allowsHitTesting(false)
    }
}
