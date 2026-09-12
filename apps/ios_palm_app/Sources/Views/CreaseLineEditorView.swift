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

    // Drag state
    @State private var isDraggingVertex: Bool = false

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
        self._selectedVertexIndex = State(initialValue: nil)
    }

    private var activeSpline: CreaseSpline? {
        workingSplines.first(where: { $0.id == selectedSplineId })
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                Color.black.ignoresSafeArea()

                VStack(spacing: 0) {
                    // Native segmented line selector (Heart / Head / Life / Fate)
                    linePicker

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

                                        // Outer soft glow (subtle, non-occluding)
                                        context.stroke(
                                            path,
                                            with: .color(color.opacity(isSelected ? 0.25 : 0.08)),
                                            style: StrokeStyle(lineWidth: isSelected ? 5 : 2.5, lineCap: .round, lineJoin: .round)
                                        )
                                        // Precision continuous curve
                                        context.stroke(
                                            path,
                                            with: .color(color.opacity(isSelected ? 0.90 : 0.40)),
                                            style: StrokeStyle(lineWidth: isSelected ? 2.4 : 1.6, lineCap: .round, lineJoin: .round)
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
                                                }
                                                .onEnded { _ in
                                                    isDraggingVertex = false
                                                    mediumHaptic.impactOccurred()
                                                    updateCoaching(text: LocalizedStrings.tr("editor.coachingUpdated", lang: selectedLanguage), autoDismissAfter: 2.0)
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
                    // Native bottom-bar tools: [Adjust] [Draw] [Undo] [Reset] [Label]
                    Button {
                        isDrawMode = false
                        lightHaptic.impactOccurred()
                        updateCoaching(text: LocalizedStrings.tr("editor.hintAdjust", lang: selectedLanguage), autoDismissAfter: 3.5)
                    } label: {
                        Label(LocalizedStrings.tr("editor.toolAdjust", lang: selectedLanguage), systemImage: "slider.horizontal.3")
                    }
                    .tint(!isDrawMode ? Color(hex: "34C759") : .white)

                    Spacer()

                    Button {
                        isDrawMode = true
                        lightHaptic.impactOccurred()
                        updateCoaching(text: LocalizedStrings.tr("editor.hintDraw", lang: selectedLanguage), autoDismissAfter: 4.0)
                    } label: {
                        Label(LocalizedStrings.tr("editor.toolDraw", lang: selectedLanguage), systemImage: "pencil.line")
                    }
                    .tint(isDrawMode ? Color(hex: "34C759") : .white)

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
        }
        .preferredColorScheme(.dark)
        .onAppear {
            lightHaptic.prepare()
            mediumHaptic.prepare()
            updateCoaching(text: LocalizedStrings.tr("editor.hintAdjust", lang: selectedLanguage), autoDismissAfter: 4.0)
        }
    }

    // MARK: - Native segmented line selector

    private var linePicker: some View {
        Picker("Line", selection: $selectedSplineId) {
            ForEach(workingSplines) { spline in
                Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                    .tag(spline.id as String?)
            }
        }
        .pickerStyle(.segmented)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
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
    // Mid points: 5pt pinpoint dot (resting).
    // Start / End points: 10pt small dot circle (resting, twice the mid dot).
    // Clicked / Dragged: Smoothly grows into 26pt hollow reticle (zero-occlusion, 100% transparent center).
    @ViewBuilder
    private func controlHandleView(color: Color, isStart: Bool, isEnd: Bool, isSelected: Bool) -> some View {
        let isEndpoint = isStart || isEnd

        ZStack {
            // Invisible 44x44pt touch area for effortless finger target
            Color.clear
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())

            if isSelected {
                // Active Expanded State: Precision Hollow Reticle (Zero-Occlusion)
                // Outer glow
                Circle()
                    .stroke(Color.white.opacity(0.85), lineWidth: 1.5)
                    .frame(width: 34, height: 34)
                    .shadow(color: color.opacity(0.8), radius: 4)

                // Dark contrast border for high visibility on light & dark skin
                Circle()
                    .stroke(Color.black.opacity(0.60), lineWidth: 3.5)
                    .frame(width: 26, height: 26)

                // Precision colored ring (hollow transparent center)
                Circle()
                    .stroke(color, lineWidth: 2.2)
                    .frame(width: 26, height: 26)

                // Inner delicate guide ring
                Circle()
                    .stroke(Color.white.opacity(0.85), lineWidth: 0.8)
                    .frame(width: 21, height: 21)
            } else if isEndpoint {
                // Resting Endpoint (Start / End): Small Dot Circle (10pt, twice the mid dot)
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
                // Resting Mid Points: Almost Dot Only (5pt pinpoint dot)
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

    private func finishFreehandDraw(baseDrawRect: CGRect) {
        guard freehandCanvasPoints.count >= 4,
              let sid = selectedSplineId,
              let sIndex = workingSplines.firstIndex(where: { $0.id == sid }) else {
            freehandCanvasPoints.removeAll()
            return
        }
        pushHistory()

        // 1. Moving-average smoothing filter to eliminate finger digitizer micro-jitter
        let smoothedPoints = Self.smoothPoints(freehandCanvasPoints)

        let normDrawn = smoothedPoints.map { canvasPt in
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
        updateCoaching(text: LocalizedStrings.tr("editor.coachingCreated", lang: selectedLanguage), autoDismissAfter: 2.5)
    }

    /// 3-tap moving-average smoothing filter to produce silky-smooth drawn curves.
    private static func smoothPoints(_ pts: [CGPoint]) -> [CGPoint] {
        guard pts.count > 3 else { return pts }
        var result: [CGPoint] = []
        result.append(pts.first!)
        for i in 1..<(pts.count - 1) {
            let prev = pts[i - 1]
            let curr = pts[i]
            let next = pts[i + 1]
            let sx = (prev.x * 0.25) + (curr.x * 0.50) + (next.x * 0.25)
            let sy = (prev.y * 0.25) + (curr.y * 0.50) + (next.y * 0.25)
            result.append(CGPoint(x: sx, y: sy))
        }
        result.append(pts.last!)
        return result
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
}
