import SwiftUI

/**
 * Primary post-capture experience: frozen photo + neon crease overlays + hand outline.
 * Line legend / light metrics sit below; Samudrika copy is secondary.
 */
public struct CaptureResultView: View {
    public let image: UIImage?
    public let landmarks: [CGPoint]
    public let splines: [CreaseSpline]
    public let selectedLanguage: AppLanguage
    public let usedNativeEngine: Bool
    public let onLanguageChange: (AppLanguage) -> Void
    public let onRetake: () -> Void

    public init(
        image: UIImage?,
        landmarks: [CGPoint],
        splines: [CreaseSpline],
        selectedLanguage: AppLanguage,
        usedNativeEngine: Bool,
        onLanguageChange: @escaping (AppLanguage) -> Void,
        onRetake: @escaping () -> Void
    ) {
        self.image = image
        self.landmarks = landmarks
        self.splines = splines
        self.selectedLanguage = selectedLanguage
        self.usedNativeEngine = usedNativeEngine
        self.onLanguageChange = onLanguageChange
        self.onRetake = onRetake
    }

    public var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                // Photo stage with overlays
                GeometryReader { proxy in
                    ZStack {
                        if let image {
                            Image(uiImage: image)
                                .resizable()
                                .scaledToFill()
                                .frame(width: proxy.size.width, height: proxy.size.height)
                                .clipped()
                        } else {
                            Color.black
                            Text("No photo")
                                .foregroundColor(.white.opacity(0.5))
                        }

                        // Dim vignette so neon lines pop
                        LinearGradient(
                            colors: [Color.black.opacity(0.35), Color.clear, Color.black.opacity(0.45)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                        .allowsHitTesting(false)

                        HandOutlineOverlay(landmarks: landmarks)
                        CreaseSplineOverlayView(splines: splines)
                    }
                }
                .frame(maxHeight: .infinity)

                // Bottom legend panel
                VStack(spacing: 14) {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(LocalizedStrings.tr("result.title", lang: selectedLanguage))
                                .font(.system(size: 18, weight: .bold, design: .rounded))
                                .foregroundColor(.white)
                            Text(
                                usedNativeEngine
                                    ? LocalizedStrings.tr("result.native", lang: selectedLanguage)
                                    : LocalizedStrings.tr("result.landmark", lang: selectedLanguage)
                            )
                            .font(.system(size: 12, weight: .regular))
                            .foregroundColor(.white.opacity(0.55))
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

                    ForEach(splines) { spline in
                        HStack(spacing: 12) {
                            Capsule()
                                .fill(Color(hex: spline.lineType.hexColor))
                                .frame(width: 22, height: 4)
                                .shadow(color: Color(hex: spline.lineType.hexColor).opacity(0.7), radius: 4)

                            Text(LocalizedStrings.tr(spline.lineType.displayNameKey, lang: selectedLanguage))
                                .font(.system(size: 15, weight: .semibold))
                                .foregroundColor(.white)

                            Spacer()

                            Text("\(Int(spline.confidence * 100))%")
                                .font(.system(size: 14, weight: .bold, design: .rounded))
                                .foregroundColor(Color(red: 0.20, green: 0.84, blue: 0.29))
                        }
                        .padding(.horizontal, 4)
                    }

                    Button(action: onRetake) {
                        HStack {
                            Image(systemName: "arrow.counterclockwise")
                            Text(LocalizedStrings.tr("button.retake", lang: selectedLanguage))
                        }
                        .font(.system(size: 16, weight: .semibold, design: .rounded))
                        .foregroundColor(.white)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(
                            RoundedRectangle(cornerRadius: 18, style: .continuous)
                                .fill(.ultraThinMaterial)
                                .overlay(
                                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                                        .stroke(Color.white.opacity(0.25), lineWidth: 1)
                                )
                        )
                    }
                    .padding(.top, 4)
                }
                .padding(.horizontal, 20)
                .padding(.top, 16)
                .padding(.bottom, 28)
                .background(
                    Color.black.opacity(0.72)
                        .background(.ultraThinMaterial)
                )
            }
        }
    }
}

/**
 * Soft hand silhouette outline from the 21 capture landmarks.
 */
public struct HandOutlineOverlay: View {
    public let landmarks: [CGPoint]

    public init(landmarks: [CGPoint]) {
        self.landmarks = landmarks
    }

    public var body: some View {
        Canvas { context, size in
            guard landmarks.count >= 21 else { return }

            let mapped = landmarks.map { CGPoint(x: $0.x * size.width, y: $0.y * size.height) }

            // Palm hull: wrist → thumb → index → middle → ring → pinky → wrist
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

            // Finger bones lightly
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

            // Landmark dots
            for pt in mapped {
                let rect = CGRect(x: pt.x - 2.5, y: pt.y - 2.5, width: 5, height: 5)
                context.fill(Path(ellipseIn: rect), with: .color(Color.white.opacity(0.55)))
            }
        }
        .allowsHitTesting(false)
    }
}
