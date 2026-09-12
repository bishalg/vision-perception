import SwiftUI
import UIKit
import CurvPalmOverlay

/**
 * Dedicated full-screen precision editor for palm crease lines.
 * Provides a pro studio editing experience:
 * - Zoom & Pan canvas (1.0x to 4.0x) with quick 1.0x reset
 * - Magnifying Loupe floating above touch point to prevent finger occlusion
 * - 44x44pt touch targets with haptic feedback
 * - Insert vertex, delete vertex, reset line, and undo stack
 * - Line selection and type reclassification
 */
public struct CreaseLineEditorView: View {
    public let image: UIImage?
    public let selectedLanguage: AppLanguage
    public let onDismiss: () -> Void
    public let onDone: ([CreaseSpline]) -> Void

    @State private var workingSplines: [CreaseSpline]
    private let initialSplines: [CreaseSpline]
    @State private var history: [[CreaseSpline]] = []
    @State private var selectedSplineId: String?
    @State private var selectedVertexIndex: Int?

    // Zoom & Pan state
    @State private var zoomScale: CGFloat = 1.0
    @State private var lastZoomScale: CGFloat = 1.0
    @State private var panOffset: CGSize = .zero
    @State private var lastPanOffset: CGSize = .zero

    // Loupe / Magnifier state
    @State private var isDraggingVertex: Bool = false
    @State private var loupeLocation: CGPoint = .zero // in local coordinates
    @State private var loupeNormPoint: CGPoint = .zero // normalized in image (0..1)

    private let lightHaptic = UIImpactFeedbackGenerator(style: .light)
    private let mediumHaptic = UIImpactFeedbackGenerator(style: .medium)

    public init(
        image: UIImage?,
        splines: [CreaseSpline],
        selectedLanguage: AppLanguage = .english,
        onDismiss: @escaping () -> Void,
        onDone: @escaping ([CreaseSpline]) -> Void
    ) {
        self.image = image
        self.selectedLanguage = selectedLanguage
        self.onDismiss = onDismiss
        self.onDone = onDone
        self._workingSplines = State(initialValue: splines)
        self.initialSplines = splines
        self._selectedSplineId = State(initialValue: splines.first?.id)
        if let first = splines.first, !first.points.isEmpty {
            self._selectedVertexIndex = State(initialValue: 0)
        }
    }

    private var activeSpline: CreaseSpline? {
        workingSplines.first(where: { $0.id == selectedSplineId })
    }

    public var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                // Top Navigation Bar
                navigationHeader
                    .padding(.top, 50) // Safe area top inset
                    .padding(.horizontal, 16)
                    .padding(.bottom, 8)
                    .background(Color.black.opacity(0.85).background(.ultraThinMaterial))
                    .zIndex(20)

                // Main Interactive Canvas
                GeometryReader { proxy in
                    let canvasSize = proxy.size
                    let imageSize = image?.size ?? canvasSize
                    let baseDrawRect = ImageCoordinateMapper.imageRect(
                        imageSize: imageSize,
                        viewSize: canvasSize,
                        mode: .fit
                    )

                    ZStack {
                        Color.black

                        // Zoomable & Pannable Container
                        ZStack {
                            if let image {
                                Image(uiImage: image)
                                    .resizable()
                                    .frame(width: baseDrawRect.width, height: baseDrawRect.height)
                                    .position(x: baseDrawRect.midX, y: baseDrawRect.midY)
                            }

                            // Crease Splines Path Render
                            Canvas { context, _ in
                                for spline in workingSplines {
                                    guard spline.points.count >= 2 else { continue }
                                    let canvasPoints = spline.points.map { ImageCoordinateMapper.map($0, into: baseDrawRect) }
                                    let path = Path.catmullRom(points: canvasPoints)
                                    let isSelected = spline.id == selectedSplineId
                                    let color = Color(hex: spline.lineType.hexColor)

                                    // Outer soft glow
                                    context.stroke(
                                        path,
                                        with: .color(color.opacity(isSelected ? 0.35 : 0.12)),
                                        style: StrokeStyle(lineWidth: isSelected ? 12 : 6, lineCap: .round, lineJoin: .round)
                                    )
                                    // Main core line
                                    context.stroke(
                                        path,
                                        with: .color(color.opacity(isSelected ? 1.0 : 0.40)),
                                        style: StrokeStyle(lineWidth: isSelected ? 3.6 : 2.4, lineCap: .round, lineJoin: .round)
                                    )
                                    // Dash pattern when selected
                                    if isSelected {
                                        context.stroke(
                                            path,
                                            with: .color(Color.white.opacity(0.4)),
                                            style: StrokeStyle(lineWidth: 1.2, lineCap: .round, dash: [6, 4])
                                        )
                                    }
                                }
                            }
                            .allowsHitTesting(false)

                            // Interactive Vertex Handles for Active Spline
                            if let spline = activeSpline {
                                ForEach(Array(spline.points.enumerated()), id: \.offset) { index, norm in
                                    let canvasPt = ImageCoordinateMapper.map(norm, into: baseDrawRect)
                                    let isVertexSelected = (index == selectedVertexIndex)
                                    let isEnd = (index == 0 || index == spline.points.count - 1)
                                    let color = Color(hex: spline.lineType.hexColor)

                                    ZStack {
                                        // Large 44x44 invisible touch target
                                        Color.clear
                                            .frame(width: 44, height: 44)
                                            .contentShape(Rectangle())

                                        // Selection halo ring
                                        if isVertexSelected {
                                            Circle()
                                                .stroke(Color.white, lineWidth: 2)
                                                .frame(width: isEnd ? 34 : 30, height: isEnd ? 34 : 30)
                                        }

                                        // Core visible handle
                                        Circle()
                                            .fill(color)
                                            .frame(width: isEnd ? 26 : 22, height: isEnd ? 26 : 22)
                                            .overlay(Circle().stroke(Color.white, lineWidth: 2))
                                            .shadow(color: .black.opacity(0.6), radius: 4, y: 2)

                                        if isEnd {
                                            Circle().fill(Color.white).frame(width: 6, height: 6)
                                        }
                                    }
                                    .position(canvasPt)
                                    .gesture(
                                        DragGesture(minimumDistance: 0, coordinateSpace: .named("editorCanvas"))
                                            .onChanged { value in
                                                if !isDraggingVertex {
                                                    pushHistory()
                                                    isDraggingVertex = true
                                                    lightHaptic.impactOccurred()
                                                }
                                                selectedSplineId = spline.id
                                                selectedVertexIndex = index

                                                // Update position in image normalized coordinates
                                                let unmapped = ImageCoordinateMapper.unmap(value.location, from: baseDrawRect)
                                                let clamped = CGPoint(
                                                    x: min(max(unmapped.x, 0.01), 0.99),
                                                    y: min(max(unmapped.y, 0.01), 0.99)
                                                )
                                                moveVertex(splineId: spline.id, index: index, to: clamped)

                                                // Update Loupe position
                                                loupeLocation = value.location
                                                loupeNormPoint = clamped
                                            }
                                            .onEnded { _ in
                                                isDraggingVertex = false
                                                mediumHaptic.impactOccurred()
                                            }
                                    )
                                }
                            }
                        }
                        .scaleEffect(zoomScale)
                        .offset(panOffset)
                        .gesture(
                            // 2-finger zoom and pan
                            MagnificationGesture()
                                .onChanged { scale in
                                    let newScale = lastZoomScale * scale
                                    zoomScale = min(max(newScale, 1.0), 4.0)
                                }
                                .onEnded { _ in
                                    lastZoomScale = zoomScale
                                }
                        )
                        .simultaneousGesture(
                            DragGesture(minimumDistance: 10)
                                .onChanged { value in
                                    guard !isDraggingVertex, zoomScale > 1.05 else { return }
                                    panOffset = CGSize(
                                        width: lastPanOffset.width + value.translation.width,
                                        height: lastPanOffset.height + value.translation.height
                                    )
                                }
                                .onEnded { _ in
                                    guard !isDraggingVertex else { return }
                                    lastPanOffset = panOffset
                                }
                        )

                        // Floating Magnifying Loupe (appears above the dragged finger)
                        if isDraggingVertex, let img = image {
                            MagnifyingLoupeView(
                                image: img,
                                focusPointNorm: loupeNormPoint,
                                lineColor: activeSpline.map { Color(hex: $0.lineType.hexColor) } ?? .white
                            )
                            // Position 85pt above the touch point on screen
                            .position(
                                x: min(max(loupeLocation.x + panOffset.width, 60), canvasSize.width - 60),
                                y: max(loupeLocation.y + panOffset.height - 85, 60)
                            )
                            .transition(.scale(scale: 0.7).combined(with: .opacity))
                            .zIndex(30)
                        }

                        // Floating 1.0x Reset Zoom Button
                        if zoomScale > 1.05 {
                            VStack {
                                HStack {
                                    Spacer()
                                    Button(action: resetZoom) {
                                        HStack(spacing: 4) {
                                            Image(systemName: "arrow.down.right.and.arrow.up.left")
                                            Text(LocalizedStrings.tr("editor.resetZoom", lang: selectedLanguage))
                                        }
                                        .font(.system(size: 12, weight: .bold, design: .monospaced))
                                        .foregroundColor(.white)
                                        .padding(.horizontal, 10)
                                        .padding(.vertical, 6)
                                        .background(Capsule().fill(.ultraThinMaterial).overlay(Capsule().stroke(Color.white.opacity(0.3), lineWidth: 1)))
                                        .shadow(color: .black.opacity(0.4), radius: 6)
                                    }
                                    .padding(.trailing, 16)
                                    .padding(.top, 16)
                                }
                                Spacer()
                            }
                        }
                    }
                    .coordinateSpace(name: "editorCanvas")
                    .clipped()
                }

                // Bottom Floating Control Studio
                bottomToolbar
                    .padding(.horizontal, 16)
                    .padding(.top, 10)
                    .padding(.bottom, 24)
                    .background(
                        RoundedRectangle(cornerRadius: 24, style: .continuous)
                            .fill(Color(white: 0.10))
                            .ignoresSafeArea(edges: .bottom)
                    )
                    .zIndex(20)
            }
        }
        .onAppear {
            lightHaptic.prepare()
            mediumHaptic.prepare()
        }
    }

    // MARK: - Navigation Header
    private var navigationHeader: some View {
        HStack(alignment: .center) {
            Button(action: onDismiss) {
                Text(LocalizedStrings.tr("editor.cancel", lang: selectedLanguage))
                    .font(.system(size: 15, weight: .regular))
                    .foregroundColor(.white.opacity(0.85))
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .background(Capsule().fill(Color.white.opacity(0.12)))
            }

            Spacer()

            VStack(spacing: 2) {
                Text(LocalizedStrings.tr("editor.title", lang: selectedLanguage))
                    .font(.system(size: 16, weight: .bold, design: .rounded))
                    .foregroundColor(.white)

                if let spline = activeSpline {
                    Text("\(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage)) • \(spline.points.count) pts")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(Color(hex: spline.lineType.hexColor))
                }
            }

            Spacer()

            Button(action: {
                mediumHaptic.impactOccurred()
                onDone(workingSplines)
            }) {
                HStack(spacing: 5) {
                    Image(systemName: "checkmark")
                        .font(.system(size: 13, weight: .bold))
                    Text(LocalizedStrings.tr("editor.done", lang: selectedLanguage))
                        .font(.system(size: 15, weight: .bold))
                }
                .foregroundColor(.black)
                .padding(.horizontal, 16)
                .padding(.vertical, 7)
                .background(Capsule().fill(Color(hex: "34C759")))
                .shadow(color: Color(hex: "34C759").opacity(0.35), radius: 6)
            }
        }
    }

    // MARK: - Bottom Toolbar
    private var bottomToolbar: some View {
        VStack(spacing: 12) {
            // Line Selection Tabs
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(workingSplines) { spline in
                        let isSelected = spline.id == selectedSplineId
                        let color = Color(hex: spline.lineType.hexColor)

                        Button {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                selectedSplineId = spline.id
                                selectedVertexIndex = 0
                            }
                            lightHaptic.impactOccurred()
                        } label: {
                            HStack(spacing: 6) {
                                Circle().fill(color).frame(width: 8, height: 8)
                                Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                    .font(.system(size: 13, weight: isSelected ? .bold : .medium))
                                    .foregroundColor(isSelected ? .white : .white.opacity(0.65))
                                Text("\(spline.points.count)")
                                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                    .foregroundColor(isSelected ? .white.opacity(0.9) : .white.opacity(0.4))
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 7)
                            .background(
                                Capsule()
                                    .fill(isSelected ? color.opacity(0.28) : Color.white.opacity(0.08))
                                    .overlay(
                                        Capsule().stroke(isSelected ? color : Color.white.opacity(0.15), lineWidth: 1)
                                    )
                            )
                        }
                    }
                }
                .padding(.horizontal, 4)
            }

            // Action Buttons Row: [Undo] [+ Point] [- Delete] [Reset] [Label]
            HStack(spacing: 8) {
                // Undo
                Button(action: undo) {
                    Label(LocalizedStrings.tr("editor.undo", lang: selectedLanguage), systemImage: "arrow.uturn.backward")
                        .font(.system(size: 12, weight: .semibold))
                }
                .buttonStyle(ToolbarActionButtonStyle(isDisabled: history.isEmpty))
                .disabled(history.isEmpty)

                // Add Point
                Button(action: addPointToActiveSpline) {
                    Label(LocalizedStrings.tr("editor.addPoint", lang: selectedLanguage), systemImage: "plus.circle.fill")
                        .font(.system(size: 12, weight: .semibold))
                }
                .buttonStyle(ToolbarActionButtonStyle(isDisabled: activeSpline == nil))
                .disabled(activeSpline == nil)

                // Delete Point
                Button(action: deleteSelectedPoint) {
                    Label(LocalizedStrings.tr("editor.removePoint", lang: selectedLanguage), systemImage: "trash")
                        .font(.system(size: 12, weight: .semibold))
                }
                .buttonStyle(ToolbarActionButtonStyle(isDisabled: (activeSpline?.points.count ?? 0) <= 2))
                .disabled((activeSpline?.points.count ?? 0) <= 2)

                // Reset Line
                Button(action: resetActiveLine) {
                    Text(LocalizedStrings.tr("editor.resetLine", lang: selectedLanguage))
                        .font(.system(size: 12, weight: .semibold))
                }
                .buttonStyle(ToolbarActionButtonStyle(isDisabled: false))

                Spacer()

                // Reassign Label
                if let spline = activeSpline {
                    Menu {
                        ForEach(CreaseLineType.allCases) { type in
                            Button(LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage)) {
                                pushHistory()
                                reassign(splineId: spline.id, to: type)
                            }
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "tag.fill")
                            Text("Type")
                        }
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(.white.opacity(0.85))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(Capsule().fill(Color.white.opacity(0.12)))
                    }
                }
            }

            // Informative hint
            Text(LocalizedStrings.tr("editor.hint", lang: selectedLanguage))
                .font(.system(size: 11))
                .foregroundColor(.white.opacity(0.55))
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    // MARK: - Logic & Actions

    private func pushHistory() {
        history.append(workingSplines)
        if history.count > 20 { history.removeFirst() }
    }

    private func undo() {
        guard let prev = history.popLast() else { return }
        workingSplines = prev
        lightHaptic.impactOccurred()
    }

    private func resetZoom() {
        withAnimation(.spring(response: 0.35, dampingFraction: 0.75)) {
            zoomScale = 1.0
            lastZoomScale = 1.0
            panOffset = .zero
            lastPanOffset = .zero
        }
    }

    private func moveVertex(splineId: String, index: Int, to normPoint: CGPoint) {
        guard let sIndex = workingSplines.firstIndex(where: { $0.id == splineId }) else { return }
        var pts = workingSplines[sIndex].points
        guard pts.indices.contains(index) else { return }
        pts[index] = normPoint
        workingSplines[sIndex] = workingSplines[sIndex].replacing(points: pts, markAdjusted: true)
    }

    private func addPointToActiveSpline() {
        guard let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }) else { return }
        pushHistory()
        var pts = workingSplines[sIndex].points
        let cur = selectedVertexIndex ?? 0
        if cur + 1 < pts.count {
            let mid = CGPoint(
                x: (pts[cur].x + pts[cur + 1].x) * 0.5,
                y: (pts[cur].y + pts[cur + 1].y) * 0.5
            )
            pts.insert(mid, at: cur + 1)
            selectedVertexIndex = cur + 1
        } else if let last = pts.last {
            pts.append(CGPoint(x: min(last.x + 0.03, 0.98), y: min(last.y + 0.03, 0.98)))
            selectedVertexIndex = pts.count - 1
        }
        workingSplines[sIndex] = workingSplines[sIndex].replacing(points: pts, markAdjusted: true)
        lightHaptic.impactOccurred()
    }

    private func deleteSelectedPoint() {
        guard let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }) else { return }
        var pts = workingSplines[sIndex].points
        guard pts.count > 2, let cur = selectedVertexIndex, pts.indices.contains(cur) else { return }
        pushHistory()
        pts.remove(at: cur)
        selectedVertexIndex = max(0, min(cur, pts.count - 1))
        workingSplines[sIndex] = workingSplines[sIndex].replacing(points: pts, markAdjusted: true)
        lightHaptic.impactOccurred()
    }

    private func resetActiveLine() {
        guard let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }),
              let initial = initialSplines.first(where: { $0.id == sid }) else { return }
        pushHistory()
        workingSplines[sIndex] = initial
        selectedVertexIndex = 0
        lightHaptic.impactOccurred()
    }

    private func reassign(splineId: String, to newType: CreaseLineType) {
        guard let idx = workingSplines.firstIndex(where: { $0.id == splineId }) else { return }
        workingSplines[idx] = workingSplines[idx].replacing(lineType: newType, markAdjusted: true)
        lightHaptic.impactOccurred()
    }
}

// MARK: - Magnifying Loupe View (2.4x circular zoom floating over touch)
private struct MagnifyingLoupeView: View {
    let image: UIImage
    let focusPointNorm: CGPoint
    let lineColor: Color
    private let diameter: CGFloat = 104

    var body: some View {
        ZStack {
            // Magnified crop of the photo
            GeometryReader { _ in
                let imgSize = image.size
                let focusX = focusPointNorm.x * imgSize.width
                let focusY = focusPointNorm.y * imgSize.height
                let zoomFactor: CGFloat = 2.4

                Image(uiImage: image)
                    .resizable()
                    .scaledToFill()
                    .frame(width: imgSize.width * zoomFactor, height: imgSize.height * zoomFactor)
                    .position(
                        x: diameter * 0.5 - (focusX * zoomFactor - diameter * 0.5),
                        y: diameter * 0.5 - (focusY * zoomFactor - diameter * 0.5)
                    )
            }
            .frame(width: diameter, height: diameter)
            .clipShape(Circle())

            // Precision crosshair
            Circle()
                .stroke(lineColor, lineWidth: 2.5)
                .frame(width: diameter, height: diameter)
                .shadow(color: lineColor.opacity(0.6), radius: 8)

            // Center target dot
            Circle()
                .fill(lineColor)
                .frame(width: 8, height: 8)
                .overlay(Circle().stroke(Color.white, lineWidth: 1.5))
        }
        .frame(width: diameter, height: diameter)
        .shadow(color: .black.opacity(0.6), radius: 10, y: 4)
    }
}

// MARK: - Button Style Helper
private struct ToolbarActionButtonStyle: ButtonStyle {
    let isDisabled: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundColor(isDisabled ? .white.opacity(0.3) : (configuration.isPressed ? .white.opacity(0.6) : .white.opacity(0.9)))
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(
                Capsule()
                    .fill(Color.white.opacity(isDisabled ? 0.04 : (configuration.isPressed ? 0.20 : 0.10)))
                    .overlay(Capsule().stroke(Color.white.opacity(isDisabled ? 0.08 : 0.18), lineWidth: 1))
            )
    }
}
