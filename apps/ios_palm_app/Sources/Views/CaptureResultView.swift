import SwiftUI
import UniformTypeIdentifiers
import CurvCameraOverlay
import CurvPalmOverlay

/**
 * Post-capture review: photo + verified crease overlays.
 * Phase 2 of Palm Scanner workflow:
 * - Native navigation bar (Cancel / title+subtitle / Edit) via NavigationStack.
 * - Native List of detected crease lines; tap a row to open the precision editor.
 * - Native bordered button styles for Retake / Analyze Reading.
 * - The photo canvas itself stays custom (no native equivalent for crease overlays).
 */
public struct CaptureResultView: View {
    public let image: UIImage?
    public let landmarks: [CGPoint]
    @Binding public var splines: [CreaseSpline]
    public let selectedLanguage: AppLanguage
    public let overlaySource: CreaseOverlaySource
    public let handSide: HandSide
    public let padDepths: [String: Float]
    public let fingertipSession: FingertipScanSession?
    public let onPatternOverride: ((FingertipDigit, DermatoglyphicPattern) -> Void)?
    public let onDismiss: () -> Void
    public let onRetake: () -> Void
    public let onProceedToAnalysis: () -> Void

    public enum ResultInspectorTab: String, CaseIterable, Identifiable {
        case creases = "2D Creases"
        case topography = "3D Topography"
        case fingertips = "Fingertips"
        case anthropometry = "Anthropometry"
        public var id: String { rawValue }
    }

    @State private var selectedTab: ResultInspectorTab = .creases
    @State private var showEditor: Bool = false
    @State private var selectedSplineId: String?

    public init(
        image: UIImage?,
        landmarks: [CGPoint],
        splines: Binding<[CreaseSpline]>,
        selectedLanguage: AppLanguage = .english,
        overlaySource: CreaseOverlaySource = .landmarkFallback,
        handSide: HandSide = .right,
        padDepths: [String: Float] = [:],
        fingertipSession: FingertipScanSession? = nil,
        initialTab: ResultInspectorTab = .creases,
        onPatternOverride: ((FingertipDigit, DermatoglyphicPattern) -> Void)? = nil,
        onLanguageChange: ((AppLanguage) -> Void)? = nil,
        onConfirmAdjustedLines: (() -> Void)? = nil,
        onDismiss: @escaping () -> Void = {},
        onRetake: @escaping () -> Void = {},
        onProceedToAnalysis: (() -> Void)? = nil
    ) {
        self.image = image
        self.landmarks = landmarks
        self._splines = splines
        self.selectedLanguage = selectedLanguage
        self.overlaySource = overlaySource
        self.handSide = handSide
        self.padDepths = padDepths
        self.fingertipSession = fingertipSession
        self.onPatternOverride = onPatternOverride
        self._selectedTab = State(initialValue: initialTab)
        self.onDismiss = onDismiss
        self.onRetake = onRetake
        self.onProceedToAnalysis = {
            onConfirmAdjustedLines?()
            onProceedToAnalysis?()
        }
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
        NavigationStack {
            VStack(spacing: 0) {
                tabPicker

                switch selectedTab {
                case .creases:
                    // Captured Image and Detected Creases Canvas
                    photoCanvas

                    // Detected lines as a native list
                    List {
                        Section(
                            String(
                                format: LocalizedStrings.tr("result.detectedCreases", lang: selectedLanguage),
                                splines.count
                            )
                        ) {
                            ForEach(splines) { spline in
                                Button {
                                    selectedSplineId = spline.id
                                    showEditor = true
                                } label: {
                                    lineRow(spline)
                                }
                            }
                        }
                    }
                    .listStyle(.insetGrouped)

                case .topography:
                    HandMeshInspectorView(
                        landmarks: landmarks,
                        padDepths: padDepths,
                        referencePalmWidthMm: 80.0
                    )

                case .fingertips:
                    if let session = fingertipSession {
                        FingertipPatternsInspectorView(
                            session: session,
                            onPatternOverride: onPatternOverride
                        )
                    } else {
                        VStack(spacing: 8) {
                            Image(systemName: "touchid")
                                .font(.system(size: 32))
                                .foregroundColor(.secondary)
                            Text("No fingertip data captured")
                                .font(.system(size: 14, weight: .medium, design: .rounded))
                                .foregroundColor(.secondary)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }

                case .anthropometry:
                    AnthropometrySummaryView(
                        landmarks: landmarks,
                        padDepths: padDepths,
                        referencePalmWidthMm: 80.0,
                        handSide: handSide
                    )
                }
            }
            .background(Color(uiColor: .systemBackground).ignoresSafeArea())
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbarBackground(Color(uiColor: .systemBackground), for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(action: onDismiss) {
                        Text(LocalizedStrings.tr("editor.cancel", lang: selectedLanguage))
                    }
                }
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 2) {
                        Text(LocalizedStrings.tr("result.title", lang: selectedLanguage))
                            .font(.headline)
                        Text(sourceSubtitle)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .foregroundStyle(Color.primary)
                }
                ToolbarItem(placement: .primaryAction) {
                    if selectedTab == .creases {
                        Button {
                            if selectedSplineId == nil {
                                selectedSplineId = splines.first?.id
                            }
                            showEditor = true
                        } label: {
                            Label(LocalizedStrings.tr("result.edit", lang: selectedLanguage), systemImage: "slider.horizontal.3")
                        }
                    }
                }
            }
            .safeAreaInset(edge: .bottom) {
                // Retake / Analyze Reading action bar with native button styles
                HStack(spacing: 12) {
                    Button(action: onRetake) {
                        Label(LocalizedStrings.tr("result.retake", lang: selectedLanguage), systemImage: "arrow.counterclockwise")
                            .frame(maxWidth: 110)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .tint(Color.primary)

                    Button(action: onProceedToAnalysis) {
                        Label(LocalizedStrings.tr("result.continueReading", lang: selectedLanguage), systemImage: "sparkles")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .tint(Color(hex: "34C759"))
                    .disabled(splines.isEmpty)
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 12)
                .background(Color(uiColor: .secondarySystemBackground))
            }
        }
        .fullScreenCover(isPresented: $showEditor) {
            CreaseLineEditorView(
                image: image,
                splines: splines,
                selectedLanguage: selectedLanguage,
                initialSelectedSplineId: selectedSplineId,
                onDismiss: { showEditor = false },
                onDone: { updatedSplines in
                    splines = updatedSplines
                    showEditor = false
                }
            )
        }
    }

    // MARK: - Photo canvas (custom: crease overlays have no native equivalent)

    private var photoCanvas: some View {
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
                    Text(LocalizedStrings.tr("result.noPhoto", lang: selectedLanguage))
                        .foregroundStyle(.secondary)
                }

                LinearGradient(
                    colors: [Color.black.opacity(0.2), Color.clear, Color.black.opacity(0.3)],
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
                    isEditing: false,
                    isFallback: overlaySource == .landmarkFallback,
                    onSelect: { tappedId in
                        selectedSplineId = tappedId
                        showEditor = true
                    }
                )
            }
        }
        .frame(minHeight: 220)
    }

    // MARK: - Native inspector segmented tab bar

    private var tabPicker: some View {
        HStack(spacing: 4) {
            ForEach(ResultInspectorTab.allCases) { tab in
                let isSelected = tab == selectedTab
                Button {
                    withAnimation(.easeInOut(duration: 0.15)) {
                        selectedTab = tab
                    }
                } label: {
                    Text(tab.rawValue)
                        .font(.system(size: 13, weight: isSelected ? .semibold : .medium, design: .rounded))
                        .foregroundColor(isSelected ? Color.primary : Color.secondary)
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
                .accessibilityIdentifier(tab.rawValue)
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
    }

    // MARK: - Native list row

    @ViewBuilder
    private func lineRow(_ spline: CreaseSpline) -> some View {
        HStack(spacing: 10) {
            Capsule()
                .fill(Color(hex: spline.lineType.hexColor))
                .frame(width: 18, height: 5)

            VStack(alignment: .leading, spacing: 2) {
                Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                    .font(.headline)
                if spline.isUserAdjusted {
                    Text(LocalizedStrings.tr("result.adjusted", lang: selectedLanguage))
                        .font(.caption)
                        .foregroundStyle(Color(hex: "34C759"))
                }
            }

            Spacer()

            Text("\(Int(spline.confidence * 100))%")
                .font(.subheadline.monospacedDigit())
                .bold()
                .foregroundStyle(Color(hex: "34C759"))

            Image(systemName: "chevron.right")
                .font(.caption)
                .bold()
                .foregroundStyle(.tertiary)
        }
        .foregroundStyle(.primary)
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
                with: .color(Color.white.opacity(0.38)),
                style: StrokeStyle(lineWidth: 1.7, lineCap: .round, lineJoin: .round)
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
                    with: .color(Color.white.opacity(0.20)),
                    style: StrokeStyle(lineWidth: 1.1, lineCap: .round, lineJoin: .round)
                )
            }

            for pt in mapped {
                let r = CGRect(x: pt.x - 2.8, y: pt.y - 2.8, width: 5.6, height: 5.6)
                context.fill(Path(ellipseIn: r), with: .color(Color.white.opacity(0.58)))
            }
        }
        .allowsHitTesting(false)
    }
}
