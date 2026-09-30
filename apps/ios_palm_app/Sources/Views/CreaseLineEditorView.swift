import SwiftUI
import UIKit
import CurvPalmOverlay

/**
 * Dedicated full-screen precision editor for palm crease lines.
 * Phase 2b of Palm Scanner workflow:
 * - Robust coordinate system: transforms the draw rect directly, eliminating scaleEffect desync.
 * - Each crease line is ONE continuous Catmull-Rom curve controlled by 4 points: Start (S), Mid 1, Mid 2, End (E).
 * - Multi-action touch & move gesture engine:
 *     1. Direct Handle Drag: drag any of the 4 control points with zero jitter.
 *     2. Whole Line Drag: touch and drag the curve line directly to translate the entire crease.
 *     3. Zero-Occlusion Magnifying Loupe: 2.2x Apple-style floating loupe above finger during all drags.
 *     4. Zoom & Pan (1.0x to 4.0x): two-finger pinch, one-finger background pan, double-tap toggle, reset button.
 *     5. Smooth Freehand Draw: 5-tap Gaussian smoothing filter for silky-smooth redrawing at any zoom level.
 * - Unified 5-tool bottom bar with state-aware coaching pill.
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

    // Editing mode: Adjust handles / line vs Freehand Trace
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

    // Drag & Move Interaction State
    @State private var isDraggingVertex: Bool = false
    @State private var isDraggingWholeLine: Bool = false
    @State private var wholeLineDragStartNorm: CGPoint = .zero
    @State private var wholeLineInitialPoints: [CGPoint] = []

    // Magnifying Loupe State
    @State private var showLoupe: Bool = false
    @State private var loupeTouchPoint: CGPoint = .zero
    @State private var loupeTargetNorm: CGPoint = .zero

    private let lightHaptic = UIImpactFeedbackGenerator(style: .light)
    private let mediumHaptic = UIImpactFeedbackGenerator(style: .medium)

    public init(
        image: UIImage?,
        splines: [CreaseSpline],
        selectedLanguage: AppLanguage = .english,
        initialSelectedSplineId: String? = nil,
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
        let initialId = (initialSelectedSplineId != nil && clean.contains(where: { $0.id == initialSelectedSplineId }))
            ? initialSelectedSplineId
            : clean.first?.id
        self._selectedSplineId = State(initialValue: initialId)
        self._selectedVertexIndex = State(initialValue: nil)
    }

    private var activeSpline: CreaseSpline? {
        workingSplines.first(where: { $0.id == selectedSplineId })
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                Color(uiColor: .systemBackground).ignoresSafeArea()

                VStack(spacing: 0) {
                    // Segmented line selector (Heart / Head / Life / Fate)
                    linePicker

                    // Main Canvas Area
                    GeometryReader { proxy in
                        let canvasSize = proxy.size
                        let imageSize = image?.size ?? canvasSize
                        let drawRect = ImageCoordinateMapper.transformedImageRect(
                            imageSize: imageSize,
                            viewSize: canvasSize,
                            mode: .fit,
                            zoomScale: zoomScale,
                            panOffset: panOffset
                        )

                        ZStack {
                            Color.black

                            // 1. Palm Photo Layer (positioned via transformed drawRect)
                            if let image {
                                Image(uiImage: image)
                                    .resizable()
                                    .frame(width: drawRect.width, height: drawRect.height)
                                    .position(x: drawRect.midX, y: drawRect.midY)
                            }

                            // 2. Continuous Catmull-Rom Splines Canvas Layer
                            Canvas { context, _ in
                                for spline in workingSplines {
                                    guard spline.points.count >= 2 else { continue }
                                    let canvasPoints = spline.points.map { ImageCoordinateMapper.map($0, into: drawRect) }
                                    let path = Path.catmullRom(points: canvasPoints)
                                    let isSelected = spline.id == selectedSplineId
                                    let color = Color(hex: spline.lineType.hexColor)

                                    // Active whole-line drag highlight halo
                                    if isSelected && isDraggingWholeLine {
                                        context.stroke(
                                            path,
                                            with: .color(color.opacity(0.40)),
                                            style: StrokeStyle(lineWidth: 12, lineCap: .round, lineJoin: .round)
                                        )
                                    }

                                    // Outer soft glow
                                    context.stroke(
                                        path,
                                        with: .color(color.opacity(isSelected ? 0.28 : 0.09)),
                                        style: StrokeStyle(lineWidth: isSelected ? 6 : 2.5, lineCap: .round, lineJoin: .round)
                                    )
                                    // Precision continuous curve
                                    context.stroke(
                                        path,
                                        with: .color(color.opacity(isSelected ? 0.95 : 0.40)),
                                        style: StrokeStyle(lineWidth: isSelected ? 2.6 : 1.6, lineCap: .round, lineJoin: .round)
                                    )
                                }

                                // In-progress freehand trace stroke (Draw mode)
                                if !freehandCanvasPoints.isEmpty {
                                    var trace = Path()
                                    trace.move(to: freehandCanvasPoints[0])
                                    for pt in freehandCanvasPoints.dropFirst() {
                                        trace.addLine(to: pt)
                                    }
                                    let activeColor = activeSpline.map { Color(hex: $0.lineType.hexColor) } ?? .white
                                    // Subtle trace glow
                                    context.stroke(
                                        trace,
                                        with: .color(activeColor.opacity(0.3)),
                                        style: StrokeStyle(lineWidth: 7, lineCap: .round, lineJoin: .round)
                                    )
                                    context.stroke(
                                        trace,
                                        with: .color(activeColor),
                                        style: StrokeStyle(lineWidth: 3.5, lineCap: .round, lineJoin: .round)
                                    )
                                }
                            }
                            .allowsHitTesting(false)

                            // 3. Canvas Background Touch Layer (Pan, Whole-Line Move, Line Selection, Freehand Draw)
                            canvasInteractionLayer(drawRect: drawRect)

                            // 4. Precision Control Handles Layer (Adjust Mode)
                            if !isDrawMode, let spline = activeSpline {
                                handlesLayer(spline: spline, drawRect: drawRect)
                            }

                            // 5. Zero-Occlusion Magnifying Loupe
                            if showLoupe {
                                MagnifyingLoupeView(
                                    image: image,
                                    splines: workingSplines,
                                    activeSplineId: selectedSplineId,
                                    targetNormPoint: loupeTargetNorm,
                                    touchLocation: loupeTouchPoint,
                                    canvasSize: canvasSize,
                                    accentColor: activeSpline.map { Color(hex: $0.lineType.hexColor) } ?? .green
                                )
                            }

                            // 6. Floating 1.0x Reset Zoom Button
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
                                            .background(
                                                Capsule()
                                                    .fill(Color.black.opacity(0.62))
                                                    .overlay(Capsule().stroke(Color.white.opacity(0.3), lineWidth: 1))
                                            )
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
                        // Pinch to zoom (two-finger gesture)
                        .simultaneousGesture(
                            MagnificationGesture()
                                .onChanged { scale in
                                    let newScale = min(max(lastZoomScale * scale, 1.0), 4.0)
                                    zoomScale = newScale
                                }
                                .onEnded { _ in
                                    lastZoomScale = zoomScale
                                    if zoomScale <= 1.02 {
                                        withAnimation(.spring(response: 0.3, dampingFraction: 0.75)) {
                                            panOffset = .zero
                                            lastPanOffset = .zero
                                        }
                                    }
                                }
                        )
                        // Double-tap to toggle zoom (1.0x <-> 2.2x)
                        .onTapGesture(count: 2) {
                            withAnimation(.spring(response: 0.35, dampingFraction: 0.75)) {
                                if zoomScale > 1.05 {
                                    zoomScale = 1.0
                                    lastZoomScale = 1.0
                                    panOffset = .zero
                                    lastPanOffset = .zero
                                } else {
                                    zoomScale = 2.2
                                    lastZoomScale = 2.2
                                }
                            }
                        }
                    }

                    // Dynamic State Coaching Pill
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
                                .fill(Color.black.opacity(0.62))
                                .overlay(Capsule().stroke(Color.white.opacity(0.20), lineWidth: 0.8))
                        )
                        .shadow(color: .black.opacity(0.35), radius: 6, y: 2)
                        .padding(.top, 8)
                        .transition(.opacity.combined(with: .scale(scale: 0.95)))
                        .zIndex(15)
                    }
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.ultraThinMaterial, for: .navigationBar)
            .toolbarBackground(.ultraThinMaterial, for: .bottomBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(action: onDismiss) {
                        Text(LocalizedStrings.tr("editor.cancel", lang: selectedLanguage))
                    }
                }
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 2) {
                        Text(LocalizedStrings.tr("editor.title", lang: selectedLanguage))
                            .font(.headline)
                        if let spline = activeSpline {
                            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                .font(.caption)
                                .foregroundStyle(Color(hex: spline.lineType.hexColor))
                        }
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(action: {
                        mediumHaptic.impactOccurred()
                        onDone(workingSplines)
                    }) {
                        Text(LocalizedStrings.tr("editor.done", lang: selectedLanguage))
                            .bold()
                    }
                }
                ToolbarItemGroup(placement: .bottomBar) {
                    Button {
                        isDrawMode = false
                        lightHaptic.impactOccurred()
                        updateCoaching(text: LocalizedStrings.tr("editor.hintAdjust", lang: selectedLanguage), autoDismissAfter: 3.5)
                    } label: {
                        Label(LocalizedStrings.tr("editor.toolAdjust", lang: selectedLanguage), systemImage: "slider.horizontal.3")
                    }
                    .tint(!isDrawMode ? Color(hex: "34C759") : Color.primary)

                    Spacer()

                    Button {
                        isDrawMode = true
                        lightHaptic.impactOccurred()
                        updateCoaching(text: LocalizedStrings.tr("editor.hintDraw", lang: selectedLanguage), autoDismissAfter: 4.0)
                    } label: {
                        Label(LocalizedStrings.tr("editor.toolDraw", lang: selectedLanguage), systemImage: "pencil.line")
                    }
                    .tint(isDrawMode ? Color(hex: "34C759") : Color.primary)

                    Spacer()

                    Button {
                        undo()
                        updateCoaching(text: LocalizedStrings.tr("editor.coachingUndone", lang: selectedLanguage), autoDismissAfter: 2.0)
                    } label: {
                        Label(LocalizedStrings.tr("editor.toolUndo", lang: selectedLanguage), systemImage: "arrow.uturn.backward")
                    }
                    .disabled(history.isEmpty)

                    Spacer()

                    Button {
                        resetActiveLine()
                        updateCoaching(text: LocalizedStrings.tr("editor.coachingReset", lang: selectedLanguage), autoDismissAfter: 2.0)
                    } label: {
                        Label(LocalizedStrings.tr("editor.toolReset", lang: selectedLanguage), systemImage: "arrow.counterclockwise")
                    }

                    Spacer()

                    if let spline = activeSpline {
                        Menu {
                            ForEach(CreaseLineType.allCases) { type in
                                Button(LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage)) {
                                    pushHistory()
                                    reassign(splineId: spline.id, to: type)
                                    updateCoaching(text: coachingReassigned(name: LocalizedStrings.tr(type.displayNameKey, lang: selectedLanguage)), autoDismissAfter: 2.5)
                                }
                            }
                        } label: {
                            Label(LocalizedStrings.tr("editor.toolLabel", lang: selectedLanguage), systemImage: "tag.fill")
                        }
                    }
                }
            }
        }
        .onAppear {
            lightHaptic.prepare()
            mediumHaptic.prepare()
            updateCoaching(text: LocalizedStrings.tr("editor.hintAdjust", lang: selectedLanguage), autoDismissAfter: 4.0)
        }
    }

    // MARK: - Canvas Interaction Layer

    @ViewBuilder
    private func canvasInteractionLayer(drawRect: CGRect) -> some View {
        if isDrawMode {
            // Freehand Draw Layer
            Color.clear
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 1, coordinateSpace: .named("editorCanvas"))
                        .onChanged { value in
                            if freehandCanvasPoints.isEmpty {
                                updateCoaching(text: LocalizedStrings.tr("editor.hintTracing", lang: selectedLanguage), autoDismissAfter: nil)
                            }
                            if let last = freehandCanvasPoints.last {
                                let d = hypot(value.location.x - last.x, value.location.y - last.y)
                                if d >= 2.0 {
                                    freehandCanvasPoints.append(value.location)
                                }
                            } else {
                                freehandCanvasPoints.append(value.location)
                            }
                        }
                        .onEnded { _ in
                            finishFreehandDraw(drawRect: drawRect)
                        }
                )
        } else {
            // Adjust Mode: Whole-Line Drag, Line Selection, and Pan
            Color.clear
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 3, coordinateSpace: .named("editorCanvas"))
                        .onChanged { value in
                            guard !isDraggingVertex else { return }

                            if !isDraggingWholeLine {
                                // Test if touch began on or very close to the active curve
                                if let spline = activeSpline,
                                   ImageCoordinateMapper.distanceToSpline(point: value.startLocation, normPoints: spline.points, in: drawRect) < 28 {
                                    pushHistory()
                                    isDraggingWholeLine = true
                                    wholeLineInitialPoints = spline.points
                                    wholeLineDragStartNorm = ImageCoordinateMapper.unmap(value.startLocation, from: drawRect)
                                    lightHaptic.impactOccurred()
                                    updateCoaching(text: LocalizedStrings.tr("editor.hintMovingLine", lang: selectedLanguage), autoDismissAfter: nil)
                                }
                            }

                            if isDraggingWholeLine, let sid = selectedSplineId {
                                let currNorm = ImageCoordinateMapper.unmap(value.location, from: drawRect)
                                let dx = currNorm.x - wholeLineDragStartNorm.x
                                let dy = currNorm.y - wholeLineDragStartNorm.y

                                translateActiveLine(splineId: sid, initialPoints: wholeLineInitialPoints, dx: dx, dy: dy)

                                loupeTouchPoint = value.location
                                loupeTargetNorm = currNorm
                                withAnimation(.easeOut(duration: 0.15)) {
                                    showLoupe = true
                                }
                            } else if zoomScale > 1.02 {
                                // Pan canvas when zoomed
                                panOffset = CGSize(
                                    width: lastPanOffset.width + value.translation.width,
                                    height: lastPanOffset.height + value.translation.height
                                )
                            }
                        }
                        .onEnded { value in
                            if isDraggingWholeLine {
                                isDraggingWholeLine = false
                                withAnimation(.easeOut(duration: 0.2)) {
                                    showLoupe = false
                                }
                                mediumHaptic.impactOccurred()
                                updateCoaching(text: LocalizedStrings.tr("editor.coachingUpdated", lang: selectedLanguage), autoDismissAfter: 2.0)
                            } else if zoomScale > 1.02 {
                                lastPanOffset = panOffset
                            } else {
                                // Quick tap to select another line
                                if hypot(value.translation.width, value.translation.height) < 10 {
                                    if let nearestId = nearestSplineId(at: value.startLocation, in: drawRect) {
                                        selectedSplineId = nearestId
                                        selectedVertexIndex = nil
                                        lightHaptic.impactOccurred()
                                    }
                                }
                            }
                        }
                )
        }
    }

    // MARK: - Handles Layer (4 control points per spline)

    @ViewBuilder
    private func handlesLayer(spline: CreaseSpline, drawRect: CGRect) -> some View {
        ForEach(Array(spline.points.enumerated()), id: \.offset) { index, norm in
            let canvasPt = ImageCoordinateMapper.map(norm, into: drawRect)
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
                            selectedSplineId = spline.id
                            selectedVertexIndex = index
                            lightHaptic.impactOccurred()

                            if isStart {
                                updateCoaching(text: LocalizedStrings.tr("editor.hintMovingStart", lang: selectedLanguage), autoDismissAfter: nil)
                            } else if isEnd {
                                updateCoaching(text: LocalizedStrings.tr("editor.hintMovingEnd", lang: selectedLanguage), autoDismissAfter: nil)
                            } else {
                                updateCoaching(text: LocalizedStrings.tr("editor.hintMovingCurve", lang: selectedLanguage), autoDismissAfter: nil)
                            }
                        }

                        let unmapped = ImageCoordinateMapper.unmap(value.location, from: drawRect)
                        let clamped = CGPoint(
                            x: min(max(unmapped.x, 0.01), 0.99),
                            y: min(max(unmapped.y, 0.01), 0.99)
                        )

                        loupeTouchPoint = value.location
                        loupeTargetNorm = clamped
                        withAnimation(.easeOut(duration: 0.15)) {
                            showLoupe = true
                        }

                        moveVertex(splineId: spline.id, index: index, to: clamped)
                    }
                    .onEnded { _ in
                        isDraggingVertex = false
                        withAnimation(.easeOut(duration: 0.2)) {
                            showLoupe = false
                        }
                        mediumHaptic.impactOccurred()
                        updateCoaching(text: LocalizedStrings.tr("editor.coachingUpdated", lang: selectedLanguage), autoDismissAfter: 2.0)
                    }
            )
        }
    }

    // MARK: - Native segmented line selector

    private var linePicker: some View {
        HStack(spacing: 4) {
            ForEach(workingSplines) { spline in
                let isSelected = spline.id == selectedSplineId
                let lineColor = Color(hex: spline.lineType.hexColor)
                Button {
                    selectedSplineId = spline.id
                } label: {
                    HStack(spacing: 5) {
                        Circle()
                            .fill(lineColor)
                            .frame(width: 7, height: 7)
                        Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                            .font(.system(size: 13, weight: isSelected ? .semibold : .medium, design: .rounded))
                            .foregroundColor(isSelected ? Color.primary : Color.secondary)
                            .lineLimit(1)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 7)
                    .frame(maxWidth: .infinity)
                    .background {
                        if isSelected {
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(Color(uiColor: .secondarySystemGroupedBackground))
                                .shadow(color: Color.black.opacity(0.10), radius: 3, y: 1)
                        }
                    }
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
            }
        }
        .padding(3)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color(uiColor: .tertiarySystemFill))
        )
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(Color(uiColor: .systemBackground))
        .onChange(of: selectedSplineId) { _ in
            selectedVertexIndex = nil
            lightHaptic.impactOccurred()
            if let id = selectedSplineId,
               let spline = workingSplines.first(where: { $0.id == id }) {
                updateCoaching(text: coachingSelected(name: LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage)), autoDismissAfter: 2.0)
            }
        }
    }

    private func coachingSelected(name: String) -> String {
        LocalizedStrings.tr("editor.coachingSelected", lang: selectedLanguage)
            .replacingOccurrences(of: "{name}", with: name)
    }

    private func coachingReassigned(name: String) -> String {
        LocalizedStrings.tr("editor.coachingReassigned", lang: selectedLanguage)
            .replacingOccurrences(of: "{name}", with: name)
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

    // MARK: - Precision Control Handle View

    @ViewBuilder
    private func controlHandleView(color: Color, isStart: Bool, isEnd: Bool, isSelected: Bool) -> some View {
        let isEndpoint = isStart || isEnd

        ZStack {
            // Invisible generous 44x44pt touch area
            Color.clear
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())

            if isSelected {
                // Active Expanded State: Precision Hollow Reticle (Zero-Occlusion)
                Circle()
                    .stroke(Color.white.opacity(0.85), lineWidth: 1.5)
                    .frame(width: 34, height: 34)
                    .shadow(color: color.opacity(0.8), radius: 4)

                Circle()
                    .stroke(Color.black.opacity(0.60), lineWidth: 3.5)
                    .frame(width: 26, height: 26)

                Circle()
                    .stroke(color, lineWidth: 2.2)
                    .frame(width: 26, height: 26)

                Circle()
                    .stroke(Color.white.opacity(0.85), lineWidth: 0.8)
                    .frame(width: 21, height: 21)
            } else if isEndpoint {
                // Resting Endpoint: Small Dot Circle (10pt)
                Circle()
                    .stroke(Color.black.opacity(0.50), lineWidth: 2.5)
                    .frame(width: 10, height: 10)

                Circle()
                    .stroke(color, lineWidth: 1.8)
                    .frame(width: 10, height: 10)

                Circle()
                    .fill(Color.white)
                    .frame(width: 3.5, height: 3.5)
            } else {
                // Resting Mid Points: Pinpoint Dot (5pt)
                Circle()
                    .fill(Color.black.opacity(0.50))
                    .frame(width: 7, height: 7)

                Circle()
                    .fill(color)
                    .frame(width: 5, height: 5)

                Circle()
                    .stroke(Color.white.opacity(0.90), lineWidth: 0.7)
                    .frame(width: 5, height: 5)
            }
        }
        .animation(.spring(response: 0.28, dampingFraction: 0.72), value: isSelected)
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

    private func translateActiveLine(splineId: String, initialPoints: [CGPoint], dx: CGFloat, dy: CGFloat) {
        guard let sIndex = workingSplines.firstIndex(where: { $0.id == splineId }) else { return }
        let translated = initialPoints.map { pt in
            CGPoint(
                x: min(max(pt.x + dx, 0.01), 0.99),
                y: min(max(pt.y + dy, 0.01), 0.99)
            )
        }
        workingSplines[sIndex] = workingSplines[sIndex].replacing(points: translated, markAdjusted: true)
    }

    private func finishFreehandDraw(drawRect: CGRect) {
        guard freehandCanvasPoints.count >= 4,
              let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }) else {
            freehandCanvasPoints.removeAll()
            return
        }

        // Verify minimum path length to filter out accidental taps
        var totalDist: CGFloat = 0
        for i in 1..<freehandCanvasPoints.count {
            totalDist += hypot(freehandCanvasPoints[i].x - freehandCanvasPoints[i - 1].x,
                               freehandCanvasPoints[i].y - freehandCanvasPoints[i - 1].y)
        }
        guard totalDist > 20 else {
            freehandCanvasPoints.removeAll()
            return
        }

        pushHistory()

        // 5-tap Gaussian smoothing filter
        let smoothed = ImageCoordinateMapper.smoothPoints(freehandCanvasPoints)
        let normDrawn = smoothed.map { canvasPt in
            let unmapped = ImageCoordinateMapper.unmap(canvasPt, from: drawRect)
            return CGPoint(
                x: min(max(unmapped.x, 0.01), 0.99),
                y: min(max(unmapped.y, 0.01), 0.99)
            )
        }

        let fourPoints = CreaseSpline.resampleToControlPoints(normDrawn, targetCount: 4)
        workingSplines[sIndex] = workingSplines[sIndex].replacing(points: fourPoints, markAdjusted: true)
        freehandCanvasPoints.removeAll()
        mediumHaptic.impactOccurred()
        updateCoaching(text: LocalizedStrings.tr("editor.coachingCreated", lang: selectedLanguage), autoDismissAfter: 2.5)
    }

    private func resetActiveLine() {
        guard let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }),
              let initial = initialSplines.first(where: { $0.id == sid }) else { return }
        pushHistory()
        workingSplines[sIndex] = initial
        selectedVertexIndex = nil
        lightHaptic.impactOccurred()
    }

    private func reassign(splineId: String, to newType: CreaseLineType) {
        guard let idx = workingSplines.firstIndex(where: { $0.id == splineId }) else { return }
        workingSplines[idx] = workingSplines[idx].replacing(lineType: newType, markAdjusted: true)
        lightHaptic.impactOccurred()
    }

    // MARK: - Geometry & Hit-Testing

    private func nearestSplineId(at point: CGPoint, in rect: CGRect) -> String? {
        var best: (String, CGFloat)?
        for spline in workingSplines {
            let d = ImageCoordinateMapper.distanceToSpline(point: point, normPoints: spline.points, in: rect)
            if d < 28, best == nil || d < best!.1 {
                best = (spline.id, d)
            }
        }
        return best?.0
    }
}
