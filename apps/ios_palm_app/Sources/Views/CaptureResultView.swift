import SwiftUI
import UniformTypeIdentifiers
import CurvPalmOverlay

/**
 * Post-capture review: photo + verified crease overlays.
 * Phase 2 of Palm Scanner workflow:
 * - Clean display of detected crease polylines
 * - Top-right standard iOS Edit button opens dedicated CreaseLineEditorView
 * - Reachable safe-area close button
 * - Retake or proceed to Samudrika Reading Analysis
 */
public struct CaptureResultView: View {
    public let image: UIImage?
    public let landmarks: [CGPoint]
    @Binding public var splines: [CreaseSpline]
    public let selectedLanguage: AppLanguage
    public let overlaySource: CreaseOverlaySource
    public let handSide: HandSide
    public let onDismiss: () -> Void
    public let onRetake: () -> Void
    public let onProceedToAnalysis: () -> Void

    @State private var showEditor: Bool = false
    @State private var selectedSplineId: String?

    public init(
        image: UIImage?,
        landmarks: [CGPoint],
        splines: Binding<[CreaseSpline]>,
        selectedLanguage: AppLanguage = .english,
        overlaySource: CreaseOverlaySource = .landmarkFallback,
        handSide: HandSide = .right,
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
        ZStack {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                // Top Safe-Area Navigation Header
                HStack(alignment: .center) {
                    // Reachable Close Button
                    Button(action: onDismiss) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 28))
                            .symbolRenderingMode(.hierarchical)
                            .foregroundStyle(.white.opacity(0.85))
                            .shadow(color: .black.opacity(0.5), radius: 4)
                    }
                    .frame(width: 44, height: 44)

                    Spacer()

                    VStack(spacing: 2) {
                        Text(LocalizedStrings.tr("result.title", lang: selectedLanguage))
                            .font(.system(size: 17, weight: .bold, design: .rounded))
                            .foregroundColor(.white)
                        Text(sourceSubtitle)
                            .font(.system(size: 11, weight: .regular))
                            .foregroundColor(.white.opacity(0.55))
                            .fixedSize(horizontal: false, vertical: true)
                    }

                    Spacer()

                    // Standard iOS Edit Button on top-right of header
                    Button {
                        showEditor = true
                    } label: {
                        HStack(spacing: 5) {
                            Image(systemName: "slider.horizontal.3")
                                .font(.system(size: 14, weight: .semibold))
                            Text(LocalizedStrings.tr("result.edit", lang: selectedLanguage))
                                .font(.system(size: 14, weight: .semibold))
                        }
                        .foregroundColor(.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(
                            Capsule()
                                .fill(Color.white.opacity(0.14))
                                .overlay(Capsule().stroke(Color.white.opacity(0.22), lineWidth: 1))
                        )
                    }
                }
                .padding(.top, 50) // Safe-area top padding below notch / Dynamic Island
                .padding(.horizontal, 16)
                .padding(.bottom, 10)
                .background(Color.black.opacity(0.80).background(.ultraThinMaterial))
                .zIndex(10)

                // Captured Image and Detected Creases Canvas
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
                            isFallback: overlaySource == .landmarkFallback
                        )
                    }
                }
                .frame(maxHeight: .infinity)

                // Bottom Verification Summary Card
                VStack(spacing: 14) {
                    // Header inside card
                    HStack {
                        Text("Detected Creases (\(splines.count))")
                            .font(.system(size: 14, weight: .bold, design: .rounded))
                            .foregroundColor(.white.opacity(0.9))

                        Spacer()

                        Button {
                            showEditor = true
                        } label: {
                            Text("Edit Points")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundColor(Color(hex: "34C759"))
                        }
                    }

                    // Detected lines list
                    VStack(spacing: 8) {
                        ForEach(splines) { spline in
                            lineRow(spline)
                                .contentShape(Rectangle())
                                .onTapGesture {
                                    selectedSplineId = spline.id
                                    showEditor = true
                                }
                        }
                    }

                    // Bottom Navigation Buttons: [Retake] on left, [Analyze Reading ➔] on right
                    HStack(spacing: 12) {
                        Button(action: onRetake) {
                            HStack(spacing: 6) {
                                Image(systemName: "arrow.counterclockwise")
                                Text(LocalizedStrings.tr("result.retake", lang: selectedLanguage))
                            }
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundColor(.white)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 14)
                            .background(
                                Capsule()
                                    .fill(Color.white.opacity(0.10))
                                    .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 1))
                            )
                        }
                        .frame(width: 110)

                        Button(action: onProceedToAnalysis) {
                            HStack(spacing: 8) {
                                Image(systemName: "sparkles")
                                Text(LocalizedStrings.tr("result.continueReading", lang: selectedLanguage))
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
                .padding(.top, 16)
                .padding(.bottom, 28)
                .background(
                    RoundedRectangle(cornerRadius: 24, style: .continuous)
                        .fill(Color(white: 0.12))
                        .ignoresSafeArea(edges: .bottom)
                )
            }
        }
        .fullScreenCover(isPresented: $showEditor) {
            CreaseLineEditorView(
                image: image,
                splines: splines,
                selectedLanguage: selectedLanguage,
                onDismiss: { showEditor = false },
                onDone: { updatedSplines in
                    splines = updatedSplines
                    showEditor = false
                }
            )
        }
    }

    @ViewBuilder
    private func lineRow(_ spline: CreaseSpline) -> some View {
        HStack(spacing: 10) {
            Capsule()
                .fill(Color(hex: spline.lineType.hexColor))
                .frame(width: 18, height: 5)
                .shadow(color: Color(hex: spline.lineType.hexColor).opacity(0.6), radius: 4)

            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                .font(.system(size: 15, weight: .semibold))
                .foregroundColor(.white)

            if spline.isUserAdjusted {
                Text("•").foregroundColor(Color(hex: "34C759"))
                Text(LocalizedStrings.tr("result.adjusted", lang: selectedLanguage))
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(Color(hex: "34C759"))
            }

            Spacer()

            Text("\(spline.points.count) pts")
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundColor(.white.opacity(0.55))

            Text("\(Int(spline.confidence * 100))%")
                .font(.system(size: 12, weight: .bold, design: .rounded))
                .foregroundColor(Color(hex: "34C759"))
                .padding(.horizontal, 7).padding(.vertical, 3)
                .background(Capsule().fill(Color(hex: "34C759").opacity(0.14)))

            Image(systemName: "chevron.right")
                .font(.system(size: 11, weight: .semibold))
                .foregroundColor(.white.opacity(0.25))
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 10)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(Color.white.opacity(0.04))
        )
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
