import SwiftUI
import UIKit
import CurvPalmOverlay

/**
 * Dedicated full-screen precision editor for palm crease lines.
 * Phase 2b of Palm Scanner workflow:
 * - Each crease line is represented by ONE continuous Catmull-Rom curve
 *   controlled by exactly 4 points: Start (S), Curve Mid 1, Curve Mid 2, and End (E).
 * - Zero clutter: no hundreds of small circles.
 * - Unified 5-tool bottom bar (icon above text caption) with equal spacing.
 * - Dynamic state-aware coaching system: guides the user on every interaction,
 *   auto-dismissing after activity to leave canvas completely clean.
 * - Zoom & Pan (1.0x to 4.0x) with Apple-style 2.4x Magnifying Loupe.
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

    // Editing mode: Adjust 4 handles vs Freehand Trace
    @State private var isDrawMode: Bool = false
    @State private var freehandCanvasPoints: [CGPoint] = []

    // Dynamic State Coaching System
    @State private var dynamicHint: String = ""
    @State private var showCoachingPill: Bool = true
    @State private var hintDismissTimer: DispatchWorkItem? = nil

    // Zoom & Pan state
    @State private var zoomScale: CGFloat = 1.0
    @State private var lastZoomScale: CGFloat = 1.0
    @State private var panOffset: CGSize = .zero
    @State private var lastPanOffset: CGSize = .zero

    // Loupe / Magnifier state
    @State private var isDraggingVertex: Bool = false
    @State private var loupeLocation: CGPoint = .zero
    @State private var loupeNormPoint: CGPoint = .zero

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

        // Guarantee all lines are clean 4-point control splines
        let clean = splines.map { s in
            s.points.count > 5
                ? s.replacing(points: CreaseSpline.resampleToControlPoints(s.points, targetCount: 4))
                : s
        }
        self._workingSplines = State(initialValue: clean)
        self.initialSplines = clean
        self._selectedSplineId = State(initialValue: clean.first?.id)
        if let first = clean.first, !first.points.isEmpty {
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
                    .padding(.top, 50)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 8)
                    .background(Color.black.opacity(0.85).background(.ultraThinMaterial))
                    .zIndex(20)

                // Main Canvas Area
                ZStack(alignment: .top) {
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

                            // Scaled Container
                            ZStack {
                                if let image {
                                    Image(uiImage: image)
                                        .resizable()
                                        .frame(width: baseDrawRect.width, height: baseDrawRect.height)
                                        .position(x: baseDrawRect.midX, y: baseDrawRect.midY)
                                }

                                // Render All Crease Splines as Smooth Continuous Curves
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
                                            with: .color(color.opacity(isSelected ? 0.38 : 0.12)),
                                            style: StrokeStyle(lineWidth: isSelected ? 10 : 5, lineCap: .round, lineJoin: .round)
                                        )
                                        // Solid continuous curve
                                        context.stroke(
                                            path,
                                            with: .color(color.opacity(isSelected ? 1.0 : 0.35)),
                                            style: StrokeStyle(lineWidth: isSelected ? 3.8 : 2.2, lineCap: .round, lineJoin: .round)
                                        )
                                    }

                                    // In-progress freehand trace stroke
                                    if !freehandCanvasPoints.isEmpty {
                                        var trace = Path()
                                        trace.move(to: freehandCanvasPoints[0])
                                        for pt in freehandCanvasPoints.dropFirst() {
                                            trace.addLine(to: pt)
                                        }
                                        let activeColor = activeSpline.map { Color(hex: $0.lineType.hexColor) } ?? .white
                                        context.stroke(
                                            trace,
                                            with: .color(activeColor),
                                            style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round)
                                        )
                                    }
                                }
                                .allowsHitTesting(false)

                                // Handles: Shown ONLY for the active spline in Adjust Mode
                                if !isDrawMode, let spline = activeSpline {
                                    ForEach(Array(spline.points.enumerated()), id: \.offset) { index, norm in
                                        let canvasPt = ImageCoordinateMapper.map(norm, into: baseDrawRect)
                                        let isVertexSelected = (index == selectedVertexIndex)
                                        let isStart = (index == 0)
                                        let isEnd = (index == spline.points.count - 1)
                                        let color = Color(hex: spline.lineType.hexColor)

                                        controlHandleView(
                                            color: color,
                                            isStart: isStart,
                                            isEnd: isEnd,
                                            isSelected: isVertexSelected
                                        )
                                        .position(canvasPt)
                                        .gesture(
                                            DragGesture(minimumDistance: 0, coordinateSpace: .named("editorCanvas"))
                                                .onChanged { value in
                                                    if !isDraggingVertex {
                                                        pushHistory()
                                                        isDraggingVertex = true
                                                        lightHaptic.impactOccurred()

                                                        if isStart {
                                                            updateCoaching(text: LocalizedStrings.tr("editor.hintMovingStart", lang: selectedLanguage), autoDismissAfter: nil)
                                                        } else if isEnd {
                                                            updateCoaching(text: LocalizedStrings.tr("editor.hintMovingEnd", lang: selectedLanguage), autoDismissAfter: nil)
                                                        } else {
                                                            updateCoaching(text: LocalizedStrings.tr("editor.hintMovingCurve", lang: selectedLanguage), autoDismissAfter: nil)
                                                        }
                                                    }
                                                    selectedSplineId = spline.id
                                                    selectedVertexIndex = index

                                                    let unmapped = ImageCoordinateMapper.unmap(value.location, from: baseDrawRect)
                                                    let clamped = CGPoint(
                                                        x: min(max(unmapped.x, 0.01), 0.99),
                                                        y: min(max(unmapped.y, 0.01), 0.99)
                                                    )
                                                    moveVertex(splineId: spline.id, index: index, to: clamped)

                                                    loupeLocation = value.location
                                                    loupeNormPoint = clamped
                                                }
                                                .onEnded { _ in
                                                    isDraggingVertex = false
                                                    mediumHaptic.impactOccurred()
                                                    updateCoaching(text: "Curve updated", autoDismissAfter: 2.0)
                                                }
                                        )
                                    }
                                }

                                // Freehand Trace Capture Layer
                                if isDrawMode {
                                    Color.clear
                                        .contentShape(Rectangle())
                                        .gesture(
                                            DragGesture(minimumDistance: 2, coordinateSpace: .named("editorCanvas"))
                                                .onChanged { value in
                                                    if freehandCanvasPoints.isEmpty {
                                                        updateCoaching(text: LocalizedStrings.tr("editor.hintTracing", lang: selectedLanguage), autoDismissAfter: nil)
                                                    }
                                                    freehandCanvasPoints.append(value.location)
                                                }
                                                .onEnded { _ in
                                                    finishFreehandDraw(baseDrawRect: baseDrawRect)
                                                }
                                        )
                                }
                            }
                            .scaleEffect(zoomScale)
                            .offset(panOffset)
                            .gesture(
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
                                DragGesture(minimumDistance: 12)
                                    .onChanged { value in
                                        guard !isDraggingVertex && !isDrawMode && zoomScale > 1.05 else { return }
                                        panOffset = CGSize(
                                            width: lastPanOffset.width + value.translation.width,
                                            height: lastPanOffset.height + value.translation.height
                                        )
                                    }
                                    .onEnded { _ in
                                        guard !isDraggingVertex && !isDrawMode else { return }
                                        lastPanOffset = panOffset
                                    }
                            )

                            // Floating 2.4x Magnifying Loupe
                            if isDraggingVertex, let img = image {
                                MagnifyingLoupeView(
                                    image: img,
                                    focusPointNorm: loupeNormPoint,
                                    lineColor: activeSpline.map { Color(hex: $0.lineType.hexColor) } ?? .white
                                )
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

                    // Dynamic State Coaching Pill (floating, auto-updating, auto-dismissing)
                    if showCoachingPill && !dynamicHint.isEmpty {
                        HStack(spacing: 6) {
                            Image(systemName: "sparkle")
                                .font(.system(size: 11, weight: .bold))
                                .foregroundColor(Color(hex: "34C759"))
                            Text(dynamicHint)
                                .font(.system(size: 12, weight: .medium))
                                .foregroundColor(.white.opacity(0.95))
                        }
                        .padding(.horizontal, 14)
                        .padding(.vertical, 6)
                        .background(
                            Capsule()
                                .fill(.ultraThinMaterial)
                                .overlay(Capsule().stroke(Color.white.opacity(0.20), lineWidth: 0.8))
                        )
                        .shadow(color: .black.opacity(0.35), radius: 6, y: 2)
                        .padding(.top, 8)
                        .transition(.opacity.combined(with: .scale(scale: 0.95)))
                        .zIndex(15)
                    }
                }

                // Bottom Floating Control Studio
                bottomToolbar
                    .padding(.horizontal, 14)
                    .padding(.top, 12)
                    .padding(.bottom, 22)
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
            updateCoaching(text: LocalizedStrings.tr("editor.hintAdjust", lang: selectedLanguage), autoDismissAfter: 4.0)
        }
    }

    // MARK: - Dynamic Coaching Helper
    private func updateCoaching(text: String, autoDismissAfter: Double? = 3.5) {
        withAnimation(.easeInOut(duration: 0.2)) {
            dynamicHint = text
            showCoachingPill = true
        }
        hintDismissTimer?.cancel()
        if let delay = autoDismissAfter {
            let item = DispatchWorkItem {
                withAnimation(.easeInOut(duration: 0.3)) {
                    showCoachingPill = false
                }
            }
            hintDismissTimer = item
            DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: item)
        }
    }

    // MARK: - Control Handle Component (Clean 4 Handles: Start, Curve 1, Curve 2, End)
    @ViewBuilder
    private func controlHandleView(color: Color, isStart: Bool, isEnd: Bool, isSelected: Bool) -> some View {
        let diameter: CGFloat = (isStart || isEnd) ? 28 : 22

        ZStack {
            Color.clear
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())

            if isSelected {
                Circle()
                    .stroke(Color.white, lineWidth: 2)
                    .frame(width: diameter + 8, height: diameter + 8)
            }

            Circle()
                .fill(color)
                .frame(width: diameter, height: diameter)
                .overlay(Circle().stroke(Color.white, lineWidth: 2))
                .shadow(color: .black.opacity(0.6), radius: 4, y: 2)

            if isStart {
                Text("S")
                    .font(.system(size: 11, weight: .black, design: .rounded))
                    .foregroundColor(.white)
            } else if isEnd {
                Text("E")
                    .font(.system(size: 11, weight: .black, design: .rounded))
                    .foregroundColor(.white)
            } else {
                Circle().fill(Color.white).frame(width: 5, height: 5)
            }
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
                    Text("\(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage)) • 4 points")
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

    // MARK: - Bottom Toolbar (Unified 5-Tool Bar: Icon Over Text)
    private var bottomToolbar: some View {
        VStack(spacing: 12) {
            // Line Selection Tabs (Heart, Head, Life, Fate)
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
                            updateCoaching(text: "Selected \(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))", autoDismissAfter: 2.0)
                        } label: {
                            HStack(spacing: 6) {
                                Circle().fill(color).frame(width: 8, height: 8)
                                Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                    .font(.system(size: 13, weight: isSelected ? .bold : .medium))
                                    .foregroundColor(isSelected ? .white : .white.opacity(0.65))
                            }
                            .padding(.horizontal, 14)
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

            // Clean 5-Column Equal Tool Row: [Adjust] [Draw] [Undo] [Reset] [Label]
            HStack(spacing: 0) {
                // 1. Adjust Tool
                toolButton(
                    icon: "slider.horizontal.3",
                    title: LocalizedStrings.tr("editor.toolAdjust", lang: selectedLanguage),
                    isActive: !isDrawMode
                ) {
                    isDrawMode = false
                    lightHaptic.impactOccurred()
                    updateCoaching(text: LocalizedStrings.tr("editor.hintAdjust", lang: selectedLanguage), autoDismissAfter: 3.5)
                }

                // 2. Draw Tool
                toolButton(
                    icon: "pencil.line",
                    title: LocalizedStrings.tr("editor.toolDraw", lang: selectedLanguage),
                    isActive: isDrawMode
                ) {
                    isDrawMode = true
                    lightHaptic.impactOccurred()
                    updateCoaching(text: LocalizedStrings.tr("editor.hintDraw", lang: selectedLanguage), autoDismissAfter: 4.0)
                }

                // 3. Undo Tool
                toolButton(
                    icon: "arrow.uturn.backward",
                    title: LocalizedStrings.tr("editor.toolUndo", lang: selectedLanguage),
                    isDisabled: history.isEmpty
                ) {
                    undo()
                    updateCoaching(text: "Action undone", autoDismissAfter: 2.0)
                }

                // 4. Reset Tool
                toolButton(
                    icon: "arrow.counterclockwise",
                    title: LocalizedStrings.tr("editor.toolReset", lang: selectedLanguage)
                ) {
                    resetActiveLine()
                    updateCoaching(text: "Line reset to original", autoDismissAfter: 2.0)
                }

                // 5. Line Type Menu Tool
                if let spline = activeSpline {
                    Menu {
                        ForEach(CreaseLineType.allCases) { type in
                            Button(LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage)) {
                                pushHistory()
                                reassign(splineId: spline.id, to: type)
                                updateCoaching(text: "Reassigned to \(LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage))", autoDismissAfter: 2.5)
                            }
                        }
                    } label: {
                        VStack(spacing: 4) {
                            Image(systemName: "tag.fill")
                                .font(.system(size: 18, weight: .semibold))
                            Text(LocalizedStrings.tr("editor.toolLabel", lang: selectedLanguage))
                                .font(.system(size: 10, weight: .medium))
                        }
                        .foregroundColor(.white.opacity(0.85))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 6)
                        .contentShape(Rectangle())
                    }
                }
            }
        }
    }

    // MARK: - Equal-Width Tool Button Component
    @ViewBuilder
    private func toolButton(
        icon: String,
        title: String,
        isActive: Bool = false,
        isDisabled: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            VStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 18, weight: isActive ? .bold : .semibold))
                Text(title)
                    .font(.system(size: 10, weight: isActive ? .bold : .medium))
            }
            .foregroundColor(
                isDisabled
                    ? .white.opacity(0.25)
                    : (isActive ? Color(hex: "34C759") : .white.opacity(0.85))
            )
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
            .background(
                isActive
                    ? RoundedRectangle(cornerRadius: 12).fill(Color(hex: "34C759").opacity(0.14))
                    : RoundedRectangle(cornerRadius: 12).fill(Color.clear)
            )
            .contentShape(Rectangle())
        }
        .disabled(isDisabled)
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

    private func finishFreehandDraw(baseDrawRect: CGRect) {
        guard freehandCanvasPoints.count >= 4,
              let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }) else {
            freehandCanvasPoints.removeAll()
            return
        }
        pushHistory()

        let normDrawn = freehandCanvasPoints.map { canvasPt in
            let unmapped = ImageCoordinateMapper.unmap(canvasPt, from: baseDrawRect)
            return CGPoint(
                x: min(max(unmapped.x, 0.01), 0.99),
                y: min(max(unmapped.y, 0.01), 0.99)
            )
        }

        let fourPoints = CreaseSpline.resampleToControlPoints(normDrawn, targetCount: 4)
        workingSplines[sIndex] = workingSplines[sIndex].replacing(points: fourPoints, markAdjusted: true)
        freehandCanvasPoints.removeAll()
        mediumHaptic.impactOccurred()
        updateCoaching(text: "New curve created", autoDismissAfter: 2.5)
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

            Circle()
                .stroke(lineColor, lineWidth: 2.5)
                .frame(width: diameter, height: diameter)
                .shadow(color: lineColor.opacity(0.6), radius: 8)

            Circle()
                .fill(lineColor)
                .frame(width: 8, height: 8)
                .overlay(Circle().stroke(Color.white, lineWidth: 1.5))
        }
        .frame(width: diameter, height: diameter)
        .shadow(color: .black.opacity(0.6), radius: 10, y: 4)
    }
}
