import SwiftUI
import UniformTypeIdentifiers
import CurvPalmOverlay

/**
 * Post-capture review: photo + editable crease overlays.
 * Supports drag vertices, reorder, and reassign labels so prediction can use the user-adjusted lines.
 * v2: language switch removed, bottom actions are [small Retake][large Use these lines], debug panel,
 * and rock-solid drag handling via named coordinate space.
 */
public struct CaptureResultView: View {
    public let image: UIImage?
    public let landmarks: [CGPoint]
    @Binding public var splines: [CreaseSpline]
    public let selectedLanguage: AppLanguage
    public let overlaySource: CreaseOverlaySource
    public let handSide: HandSide
    public let onRetake: () -> Void
    public let onConfirmAdjustedLines: () -> Void

    @State private var isEditing = false
    @State private var selectedSplineId: String?
    @State private var editMode: EditMode = .inactive
    @State private var showDebug = false

    public init(
        image: UIImage?,
        landmarks: [CGPoint],
        splines: Binding<[CreaseSpline]>,
        selectedLanguage: AppLanguage,
        overlaySource: CreaseOverlaySource,
        handSide: HandSide = .right,
        onLanguageChange: ((AppLanguage) -> Void)? = nil,
        onRetake: @escaping () -> Void,
        onConfirmAdjustedLines: @escaping () -> Void
    ) {
        self.image = image
        self.landmarks = landmarks
        self._splines = splines
        self.selectedLanguage = selectedLanguage
        self.overlaySource = overlaySource
        self.handSide = handSide
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
                            isFallback: overlaySource == .landmarkFallback,
                            onSelect: { selectedSplineId = $0 },
                            onMoveVertex: { id, index, point in
                                moveVertex(splineId: id, index: index, to: point)
                            }
                        )
                    }
                    .coordinateSpace(name: "creaseCanvas")
                }
                .frame(maxHeight: .infinity)

                VStack(spacing: 12) {
                    if overlaySource == .landmarkFallback {
                        HStack(spacing: 10) {
                            Image(systemName: "exclamationmark.triangle.fill").foregroundColor(Color.yellow).font(.system(size: 18))
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Fallback guides — drag dashed lines to align").font(.system(size: 12, weight: .bold)).foregroundColor(.white)
                                Text("Native Steger not linked or low quality. Dashed = editable scaffold.").font(.system(size: 10)).foregroundColor(.white.opacity(0.72)).fixedSize(horizontal: false, vertical: true)
                            }
                        }
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Color(red: 0.5, green: 0.22, blue: 0.07).opacity(0.92)).overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.yellow.opacity(0.55), lineWidth: 1.5)))
                    }
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
                    }

                    // Tools: edit
                    HStack(spacing: 8) {
                        Button {
                            withAnimation(.easeInOut(duration: 0.18)) { isEditing.toggle() }
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
                                systemImage: isEditing ? "checkmark.circle.fill" : "pencil.and.scribble"
                            )
                            .font(.system(size: 13, weight: .semibold))
                        }
                        .buttonStyle(ResultChipButtonStyle(emphasized: isEditing))
                        if isEditing, let sid = selectedSplineId, splines.first(where: { $0.id == sid }) != nil {
                            Button {
                                addPoint(to: sid)
                            } label: {
                                Label(LocalizedStrings.tr("result.addPoint", lang: selectedLanguage), systemImage: "plus.circle.fill")
                                    .font(.system(size: 13, weight: .semibold))
                            }
                            .buttonStyle(ResultChipButtonStyle(emphasized: false))
                            Button(role: .destructive) {
                                removeSelectedPoint()
                            } label: {
                                Label("Remove", systemImage: "trash")
                                    .font(.system(size: 13, weight: .semibold))
                            }
                            .buttonStyle(ResultChipButtonStyle(emphasized: false))
                        }
                        Spacer()
                        if isEditing {
                            Button { resetSelectedLine() } label: {
                                Text("Reset").font(.system(size: 12, weight: .semibold)).foregroundColor(.white.opacity(0.8))
                            }
                        }
                    }

                    if isEditing {
                        Text(LocalizedStrings.tr("result.editHint", lang: selectedLanguage) + " • Tap line to select. Drag white dots. Pinch to zoom photo not supported — use dots.")
                            .font(.system(size: 11))
                            .foregroundColor(.white.opacity(0.62))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }

                    if let sel = splines.first(where: { $0.id == selectedSplineId }) {
                        HStack(spacing: 8) {
                            Capsule().fill(Color(hex: sel.lineType.hexColor)).frame(width: 18, height: 18).overlay(Circle().stroke(Color.white.opacity(0.9), lineWidth: 1))
                            Text(LocalizedStrings.tr(sel.lineType.displayNameKey, lang: selectedLanguage)).font(.system(size: 14, weight: .bold)).foregroundColor(.white)
                            Text("\(sel.points.count) pts").font(.system(size: 11, weight: .medium, design: .monospaced)).foregroundColor(.white.opacity(0.6))
                            if sel.isUserAdjusted { Text("• edited").font(.system(size: 11, weight: .bold)).foregroundColor(Color(hex: "34C759")) }
                            Spacer()
                            Menu {
                                ForEach(CreaseLineType.allCases) { type in
                                    Button(LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage)) { reassign(splineId: sel.id, to: type) }
                                }
                            } label: {
                                HStack(spacing: 4) { Image(systemName: "tag.fill"); Text("Label") }.font(.system(size: 11, weight: .semibold)).foregroundColor(.white.opacity(0.85)).padding(.horizontal, 8).padding(.vertical, 6).background(Capsule().fill(Color.white.opacity(0.12)))
                            }
                        }
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Color.white.opacity(0.08)).overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.white.opacity(0.10), lineWidth: 1)))
                    }

                    List {
                        ForEach(splines) { spline in
                            lineRow(spline)
                                .listRowBackground(Color.clear)
                                .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
                                .contentShape(Rectangle())
                                .onTapGesture {
                                    withAnimation(.easeInOut(duration: 0.15)) { selectedSplineId = spline.id }
                                    if !isEditing { isEditing = true; editMode = .active }
                                }
                        }
                        .onMove(perform: moveLines)
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .environment(\.editMode, $editMode)
                    .frame(maxHeight: min(CGFloat(splines.count) * 48 + 8, 220))

                    // Samudrika Reading (Basic) — rule-based, no VLM. Visible when splines present; highlighted after "Use these lines" (showDebug).
                    if !splines.isEmpty {
                        VStack(alignment: .leading, spacing: 10) {
                            HStack(spacing: 8) {
                                Image(systemName: "sparkles").font(.system(size: 11, weight: .bold)).foregroundColor(Color(hex: "34C759"))
                                Text("Samudrika Reading (Basic)").font(.system(size: 12, weight: .bold, design: .rounded)).foregroundColor(.white)
                                Text("rule-based · no VLM").font(.system(size: 9, weight: .medium, design: .monospaced)).foregroundColor(.white.opacity(0.45)).padding(.horizontal, 6).padding(.vertical, 2).background(Capsule().fill(Color.white.opacity(0.08)))
                                Spacer()
                                Text(handSide == .left ? "Left" : handSide == .right ? "Right" : "Auto").font(.system(size: 10, weight: .semibold)).foregroundColor(.white.opacity(0.6))
                            }
                            Text("Local interpretation from geometry: tortuosity, length, confidence, points. Not a prediction claim.")
                                .font(.system(size: 10, weight: .regular)).foregroundColor(.white.opacity(0.55)).fixedSize(horizontal: false, vertical: true)
                            ForEach(splines) { s in
                                let reading = SimplePalmReading.readingText(for: s, handSide: handSide)
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack(spacing: 6) {
                                        Circle().fill(Color(hex: s.lineType.hexColor)).frame(width: 7, height: 7)
                                        Text(LocalizedStrings.tr(s.lineType.displayNameKey, lang: selectedLanguage)).font(.system(size: 11, weight: .bold)).foregroundColor(.white)
                                        Text(String(format: "conf %.0f%%  tort %.2f  %d pts", s.confidence*100, s.tortuosity, s.points.count)).font(.system(size: 10, weight: .medium, design: .monospaced)).foregroundColor(.white.opacity(0.55))
                                        if s.isUserAdjusted { Text("edited").font(.system(size: 9, weight: .bold)).foregroundColor(Color(hex: "34C759")).padding(.horizontal, 5).padding(.vertical, 2).background(Capsule().fill(Color(hex: "34C759").opacity(0.18))) }
                                    }
                                    Text(reading)
                                        .font(.system(size: 11, weight: .regular)).foregroundColor(.white.opacity(0.88)).fixedSize(horizontal: false, vertical: true)
                                    Text(String(format: "len:%.1fpx  start:(%.3f,%.3f)  end:(%.3f,%.3f)", s.arcLengthPx, s.points.first?.x ?? 0, s.points.first?.y ?? 0, s.points.last?.x ?? 0, s.points.last?.y ?? 0))
                                        .font(.system(size: 9, weight: .regular, design: .monospaced)).foregroundColor(.white.opacity(0.42))
                                }
                                .padding(8)
                                .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(0.06)))
                            }
                        }
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Color(hex: "1A1A1E").opacity(0.92)).overlay(RoundedRectangle(cornerRadius: 12).stroke(Color(hex: "34C759").opacity(showDebug ? 0.35 : 0.15), lineWidth: 1)))
                    }

                    // Debug/analysis panel (enhanced with readingText)
                    if showDebug && !splines.isEmpty {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack { Text("Debug analysis").font(.system(size: 12, weight: .bold, design: .monospaced)).foregroundColor(.white.opacity(0.85)); Spacer(); Button("Hide") { withAnimation { showDebug = false } }.font(.system(size: 11)).foregroundColor(.white.opacity(0.6)) }
                            ForEach(splines) { s in
                                let start = s.points.first ?? .zero
                                let end = s.points.last ?? .zero
                                let len = s.arcLengthPx
                                let straight = hypot(end.x - start.x, end.y - start.y) * 1000 // normalized -> approx px scale
                                let reading = SimplePalmReading.readingText(for: s, handSide: handSide)
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack(spacing: 6) {
                                        Circle().fill(Color(hex: s.lineType.hexColor)).frame(width: 7, height: 7)
                                        Text(s.lineType.rawValue).font(.system(size: 11, weight: .bold, design: .monospaced)).foregroundColor(.white)
                                        Text(String(format: "conf %.0f%%  tort %.2f", s.confidence*100, s.tortuosity)).font(.system(size: 10, weight: .medium, design: .monospaced)).foregroundColor(.white.opacity(0.6))
                                        if s.isUserAdjusted { Text("edited").font(.system(size: 9, weight: .bold)).foregroundColor(Color(hex: "34C759")).padding(.horizontal, 5).padding(.vertical, 2).background(Capsule().fill(Color(hex: "34C759").opacity(0.18))) }
                                    }
                                    Text(String(format: "pts:%d  len:%.1fpx  tort:%.2f  start:(%.3f,%.3f)  end:(%.3f,%.3f)", s.points.count, len, s.tortuosity, start.x, start.y, end.x, end.y))
                                        .font(.system(size: 10, weight: .regular, design: .monospaced)).foregroundColor(.white.opacity(0.72)).fixedSize(horizontal: false, vertical: true)
                                    Text(reading).font(.system(size: 10, weight: .regular)).foregroundColor(Color(hex: "34C759").opacity(0.9)).fixedSize(horizontal: false, vertical: true)
                                    Text(String(format: "chord ~%.1f  gap:%d (debug)", straight, 0)).font(.system(size: 9, weight: .regular, design: .monospaced)).foregroundColor(.white.opacity(0.45))
                                }
                                .padding(8)
                                .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(0.06)))
                            }
                        }
                        .padding(10)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Color.black.opacity(0.28)).overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.white.opacity(0.10), lineWidth: 1)))
                    }

                    // Bottom actions: small Retake left, large Use button right
                    HStack(spacing: 10) {
                        Button(action: onRetake) {
                            HStack(spacing: 6) {
                                Image(systemName: "arrow.counterclockwise")
                                Text(LocalizedStrings.tr("result.retake", lang: selectedLanguage))
                            }
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundColor(.white)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 12)
                            .background(Capsule().fill(Color.white.opacity(0.10)).overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 1)))
                        }
                        .frame(width: 112)

                        Button {
                            onConfirmAdjustedLines()
                            withAnimation(.easeInOut(duration: 0.22)) { showDebug = true }
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "sparkles")
                                Text(LocalizedStrings.tr("result.useAdjusted", lang: selectedLanguage))
                            }
                            .font(.system(size: 15, weight: .bold))
                            .foregroundColor(.black)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                            .background(Capsule().fill(Color(hex: "34C759")))
                            .shadow(color: Color(hex: "34C759").opacity(0.35), radius: 8, y: 2)
                        }
                        .disabled(splines.isEmpty)
                        .opacity(splines.isEmpty ? 0.5 : 1)
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
        HStack(spacing: 10) {
            Capsule()
                .fill(Color(hex: spline.lineType.hexColor))
                .frame(width: spline.id == selectedSplineId ? 22 : 18, height: 4)
                .shadow(color: Color(hex: spline.lineType.hexColor).opacity(spline.id == selectedSplineId ? 0.85 : 0.5), radius: 4)

            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                .font(.system(size: 15, weight: spline.id == selectedSplineId ? .bold : .semibold))
                .foregroundColor(.white)
            if spline.id == selectedSplineId {
                Text("●").font(.system(size: 8)).foregroundColor(Color(hex: spline.lineType.hexColor))
            }

            if spline.isUserAdjusted {
                Text("•").foregroundColor(Color(hex: "34C759"))
                Text(LocalizedStrings.tr("result.adjusted", lang: selectedLanguage))
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(Color(hex: "34C759"))
            }

            Spacer()

            Text("\(Int(spline.confidence * 100))%")
                .font(.system(size: 12, weight: .bold, design: .rounded))
                .foregroundColor(Color(hex: "34C759"))
                .padding(.horizontal, 7).padding(.vertical, 3)
                .background(Capsule().fill(Color(hex: "34C759").opacity(0.14)))
            Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundColor(.white.opacity(0.22))
        }
        .padding(.vertical, 8).padding(.horizontal, 8)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(spline.id == selectedSplineId ? Color(hex: spline.lineType.hexColor).opacity(0.14) : Color.white.opacity(0.04))
                .overlay(RoundedRectangle(cornerRadius: 10).stroke(spline.id == selectedSplineId ? Color(hex: spline.lineType.hexColor).opacity(0.55) : Color.clear, lineWidth: 1.2))
        )
    }

    private func moveVertex(splineId: String, index: Int, to point: CGPoint) {
        guard let i = splines.firstIndex(where: { $0.id == splineId }) else { return }
        guard splines[i].points.indices.contains(index) else { return }
        var pts = splines[i].points
        pts[index] = CGPoint(x: min(max(point.x, 0.02), 0.98), y: min(max(point.y, 0.02), 0.98))
        splines[i] = splines[i].replacing(points: pts)
    }

    private func addPoint(to splineId: String) {
        guard let i = splines.firstIndex(where: { $0.id == splineId }) else { return }
        var pts = splines[i].points
        if pts.count < 2 { pts.append(CGPoint(x: 0.5, y: 0.5)) }
        else {
            var best = 0; var bestLen: CGFloat = -1
            for s in 0..<(pts.count-1) {
                let len = hypot(pts[s+1].x - pts[s].x, pts[s+1].y - pts[s].y)
                if len > bestLen { bestLen = len; best = s }
            }
            let mid = CGPoint(x: (pts[best].x + pts[best+1].x)*0.5, y: (pts[best].y + pts[best+1].y)*0.5)
            pts.insert(mid, at: best+1)
        }
        splines[i] = splines[i].replacing(points: pts)
    }

    private func removeSelectedPoint() {
        guard let sid = selectedSplineId, let i = splines.firstIndex(where: { $0.id == sid }) else { return }
        var pts = splines[i].points
        guard pts.count > 2 else { return }
        // remove last point of selected (user can choose which via selection of handles is visual)
        pts.removeLast()
        splines[i] = splines[i].replacing(points: pts)
    }

    private func resetSelectedLine() {
        guard let sid = selectedSplineId, let i = splines.firstIndex(where: { $0.id == sid }) else { return }
        splines[i] = splines[i].replacing(points: splines[i].points, markAdjusted: false)
        splines[i].isUserAdjusted = false
    }

    private func moveLines(from source: IndexSet, to destination: Int) {
        splines.move(fromOffsets: source, toOffset: destination)
        for i in splines.indices { splines[i].isUserAdjusted = true }
    }

    private func reassign(splineId: String, to type: CreaseLineType) {
        guard let i = splines.firstIndex(where: { $0.id == splineId }) else { return }
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
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(
                Capsule()
                    .fill(emphasized ? Color(hex: "34C759") : Color.white.opacity(0.12))
                    .overlay(Capsule().stroke(Color.white.opacity(0.2), lineWidth: 1))
            )
            .opacity(configuration.isPressed ? 0.75 : 1)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
    }
}

/**
 * Soft hand silhouette outline from the 21 capture landmarks.
 */
public struct HandOutlineOverlay: View {
    public let landmarks: [CGPoint]
    public let imageRect: CGRect?
    public init(landmarks: [CGPoint], imageRect: CGRect? = nil) { self.landmarks = landmarks; self.imageRect = imageRect }
    public var body: some View {
        Canvas { context, size in
            guard landmarks.count >= 21 else { return }
            let rect = imageRect ?? CGRect(origin: .zero, size: size)
            let mapped = landmarks.map { ImageCoordinateMapper.map($0, into: rect) }
            let hullIdx = [0, 1, 2, 4, 8, 12, 16, 20, 17, 0]
            var hull = Path(); for (i, idx) in hullIdx.enumerated() { let pt = mapped[idx]; if i == 0 { hull.move(to: pt) } else { hull.addLine(to: pt) } }
            context.stroke(hull, with: .color(Color.white.opacity(0.38)), style: StrokeStyle(lineWidth: 1.7, lineCap: .round, lineJoin: .round))
            let chains: [[Int]] = [[0,1,2,3,4],[0,5,6,7,8],[0,9,10,11,12],[0,13,14,15,16],[0,17,18,19,20]]
            for chain in chains { var path = Path(); for (i, idx) in chain.enumerated() { let pt = mapped[idx]; if i == 0 { path.move(to: pt) } else { path.addLine(to: pt) } }; context.stroke(path, with: .color(Color.white.opacity(0.20)), style: StrokeStyle(lineWidth: 1.1, lineCap: .round, lineJoin: .round)) }
            for pt in mapped { let r = CGRect(x: pt.x - 2.8, y: pt.y - 2.8, width: 5.6, height: 5.6); context.fill(Path(ellipseIn: r), with: .color(Color.white.opacity(0.58))) }
        }.allowsHitTesting(false)
    }
}
