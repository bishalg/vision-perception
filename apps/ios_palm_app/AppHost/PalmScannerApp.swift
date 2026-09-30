import SwiftUI
import UIKit
import CurvPalmOverlay

@main
struct PalmScannerApp: App {
    @State private var isShowingSplash: Bool = true
    @State private var demoSplines: [CreaseSpline] = []

    /// DEBUG QA hook: `xcrun simctl launch <device> org.curv.palmapp -DemoCaptureResult`
    /// presents the post-capture result view with synthetic, landmark-consistent data —
    /// lets the result tabs be exercised on the camera-less simulator.
    private var isDemoCapture: Bool {
        #if DEBUG
        ProcessInfo.processInfo.arguments.contains("-DemoCaptureResult")
        #else
        false
        #endif
    }

    var body: some Scene {
        WindowGroup {
            #if DEBUG
            if isDemoCapture {
                if ProcessInfo.processInfo.arguments.contains("-DemoMountDetail") {
                    // Mount detail sheet screenshot mode (Jupiter with full relief data).
                    MountDetailSheet(
                        mount: .jupiter,
                        entry: DemoCapturePhoto.reliefMap.entry(for: .jupiter),
                        fallbackMm: DemoCapturePhoto.padDepths["jupiter"],
                        capturedImage: DemoCapturePhoto.render(),
                        landmarks: DemoCapturePhoto.landmarks,
                        handSide: .right
                    )
                } else if ProcessInfo.processInfo.arguments.contains("-DemoReport") {
                    // Full reading sheet with the 3D topography report populated.
                    SamudrikaReadingSheet(
                        result: nil,
                        splines: DemoCapturePhoto.demoSplines,
                        handSide: .right,
                        selectedLanguage: .english,
                        onLanguageChange: { _ in },
                        onRetake: {},
                        mountReliefMap: DemoCapturePhoto.reliefMap,
                        padDepths: DemoCapturePhoto.padDepths,
                        landmarks: DemoCapturePhoto.landmarks,
                        capturedImage: DemoCapturePhoto.render()
                    )
                } else if ProcessInfo.processInfo.arguments.contains("-DemoPhotoOverlay") {
                    // Standalone inspector with the photo underlay pre-enabled —
                    // deterministic screenshot mode (no UI automation needed).
                    HandMeshInspectorView(
                        landmarks: DemoCapturePhoto.landmarks,
                        padDepths: DemoCapturePhoto.padDepths,
                        handSide: .right,
                        capturedImage: DemoCapturePhoto.render(),
                        initialPhotoOverlay: true
                    )
                } else {
                CaptureResultView(
                    image: DemoCapturePhoto.render(),
                    landmarks: DemoCapturePhoto.landmarks,
                    splines: $demoSplines,
                    handSide: .right,
                    padDepths: DemoCapturePhoto.padDepths,
                    mountReliefMap: DemoCapturePhoto.reliefMap,
                    initialTab: .topography,
                    onDismiss: {},
                    onRetake: {}
                )
                }
            } else {
                mainFlow
            }
            #else
            mainFlow
            #endif
        }
    }

    private var mainFlow: some View {
        ZStack {
            MainScannerView()

            if isShowingSplash {
                ZStack {
                    // System backdrop (adapts); brand mark sits on a fixed
                    // dark tile since the logo art is white-on-transparent.
                    Color(uiColor: .systemBackground).ignoresSafeArea()

                    Image("LaunchLogo")
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .frame(width: 112, height: 112)
                        .padding(20)
                        .background(
                            RoundedRectangle(cornerRadius: 30, style: .continuous)
                                .fill(Color(red: 0.04, green: 0.06, blue: 0.10))
                        )
                }
                .transition(.opacity)
                .zIndex(999)
                .allowsHitTesting(false)
            }
        }
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
                withAnimation(.easeOut(duration: 0.35)) {
                    isShowingSplash = false
                }
            }
        }
        // NOTE: no global preferredColorScheme here — the app follows the
        // system theme on launch. Camera/editor canvases stay dark media
        // surfaces (contrast for tracing); all chrome uses semantic
        // colors. The reading sheet follows the system so it feels
        // native in both modes.
    }
}

#if DEBUG
/// Synthetic "palm photo" + matching landmarks for the demo result view.
/// The silhouette is drawn *from* the landmark set, so photo and landmarks
/// align by construction — the same contract the real capture pipeline must
/// satisfy and the Photo underlay overlay is meant to validate.
private enum DemoCapturePhoto {
    /// Normalized landmark set (0…1, top-left origin) — upright right palm.
    static let landmarks: [CGPoint] = {
        var pts = [CGPoint](repeating: CGPoint(x: 0.5, y: 0.5), count: 21)
        pts[0] = CGPoint(x: 0.50, y: 0.85)  // wrist
        pts[1] = CGPoint(x: 0.40, y: 0.72)  // thumb
        pts[2] = CGPoint(x: 0.32, y: 0.62)
        pts[3] = CGPoint(x: 0.28, y: 0.54)
        pts[4] = CGPoint(x: 0.24, y: 0.48)
        pts[5] = CGPoint(x: 0.38, y: 0.50)  // index
        pts[6] = CGPoint(x: 0.36, y: 0.38)
        pts[7] = CGPoint(x: 0.35, y: 0.28)
        pts[8] = CGPoint(x: 0.34, y: 0.20)
        pts[9] = CGPoint(x: 0.50, y: 0.48)  // middle
        pts[10] = CGPoint(x: 0.50, y: 0.34)
        pts[11] = CGPoint(x: 0.50, y: 0.24)
        pts[12] = CGPoint(x: 0.50, y: 0.15)
        pts[13] = CGPoint(x: 0.62, y: 0.51) // ring
        pts[14] = CGPoint(x: 0.63, y: 0.37)
        pts[15] = CGPoint(x: 0.64, y: 0.27)
        pts[16] = CGPoint(x: 0.65, y: 0.19)
        pts[17] = CGPoint(x: 0.72, y: 0.55) // pinky
        pts[18] = CGPoint(x: 0.74, y: 0.44)
        pts[19] = CGPoint(x: 0.75, y: 0.36)
        pts[20] = CGPoint(x: 0.76, y: 0.29)
        return pts
    }()

    static let padDepths: [String: Float] = [
        "venus": 2.5, "jupiter": 1.8, "saturn": 1.5, "apollo": 1.6,
        "mercury": 1.2, "luna": 2.0, "palmar_fossa": -1.0, "wrist": -0.3
    ]

    /// Plausible per-mount relief for the detail sheet demo.
    static let reliefMap: MountReliefMap = {        func entry(_ m: PalmMount, mm: Float, rel: Float, prom: MountProminence, conf: Float, views: Int) -> MountReliefEntry {
            MountReliefEntry(mount: m.rawValue, heightMm: mm, heightRelative: rel,
                             prominence: prom, confidence: conf, viewCount: views)
        }
        return MountReliefMap(
            mounts: [
                entry(.venus, mm: 2.5, rel: 0.83, prom: .elevated, conf: 0.91, views: 14),
                entry(.jupiter, mm: 2.3, rel: 0.76, prom: .elevated, conf: 0.88, views: 12),
                entry(.saturn, mm: 1.5, rel: 0.50, prom: .average, conf: 0.84, views: 12),
                entry(.apollo, mm: 1.6, rel: 0.53, prom: .average, conf: 0.86, views: 12),
                entry(.mercury, mm: 1.2, rel: 0.40, prom: .average, conf: 0.80, views: 11),
                entry(.luna, mm: 2.0, rel: 0.66, prom: .elevated, conf: 0.78, views: 10),
                entry(.mars, mm: -1.0, rel: 0.18, prom: .deficient, conf: 0.72, views: 9)
            ],
            venusElevated: true,
            source: "demo_orbit_session"
        )
    }()

    /// Three plausible crease splines so the reading sheet shows both report
    /// and line sections in the demo.
    static let demoSplines: [CreaseSpline] = [
        CreaseSpline(
            lineType: .heart,
            points: [CGPoint(x: 0.35, y: 0.34), CGPoint(x: 0.50, y: 0.31),
                     CGPoint(x: 0.64, y: 0.32), CGPoint(x: 0.77, y: 0.29)],
            confidence: 0.92, tortuosity: 1.08, arcLengthPx: 318
        ),
        CreaseSpline(
            lineType: .head,
            points: [CGPoint(x: 0.42, y: 0.42), CGPoint(x: 0.54, y: 0.44),
                     CGPoint(x: 0.66, y: 0.45), CGPoint(x: 0.75, y: 0.47)],
            confidence: 0.88, tortuosity: 1.15, arcLengthPx: 296
        ),
        CreaseSpline(
            lineType: .life,
            points: [CGPoint(x: 0.44, y: 0.36), CGPoint(x: 0.46, y: 0.52),
                     CGPoint(x: 0.44, y: 0.66), CGPoint(x: 0.52, y: 0.79)],
            confidence: 0.95, tortuosity: 1.22, arcLengthPx: 402
        ),
    ]

    static func render() -> UIImage {
        let size = CGSize(width: 900, height: 1200)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { ctx in
            let cg = ctx.cgContext
            let px = { (p: CGPoint) -> CGPoint in
                CGPoint(x: p.x * size.width, y: p.y * size.height)
            }

            // Table backdrop
            let backdrop = CGGradient(
                colorsSpace: CGColorSpaceCreateDeviceRGB(),
                colors: [
                    UIColor(red: 0.16, green: 0.13, blue: 0.11, alpha: 1).cgColor,
                    UIColor(red: 0.09, green: 0.08, blue: 0.07, alpha: 1).cgColor
                ] as CFArray,
                locations: nil
            )!
            cg.drawLinearGradient(backdrop, start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: size.height), options: [])

            // Hand silhouette: palm blob + finger chains from the landmarks.
            let lm = landmarks.map(px)
            cg.setLineCap(.round)
            cg.setLineJoin(.round)

            let palmCenter = CGPoint(
                x: (lm[0].x + lm[9].x) * 0.5,
                y: (lm[0].y + lm[9].y) * 0.5
            )
            cg.setFillColor(UIColor(red: 0.85, green: 0.66, blue: 0.52, alpha: 1).cgColor)
            cg.setStrokeColor(UIColor(red: 0.85, green: 0.66, blue: 0.52, alpha: 1).cgColor)
            let palmRadius = hypot(lm[5].x - lm[17].x, lm[5].y - lm[17].y) * 0.62
            cg.fillEllipse(in: CGRect(
                x: palmCenter.x - palmRadius, y: palmCenter.y - palmRadius * 1.15,
                width: palmRadius * 2, height: palmRadius * 2.3
            ))
            for chain in [[0, 1, 2, 3, 4], [0, 5, 6, 7, 8], [0, 9, 10, 11, 12],
                          [0, 13, 14, 15, 16], [0, 17, 18, 19, 20]] {
                cg.setLineWidth(52)
                cg.move(to: lm[chain[0]])
                for idx in chain.dropFirst() { cg.addLine(to: lm[idx]) }
                cg.strokePath()
            }

            // Soft shading + creases for a photo-like feel
            let shade = CGGradient(
                colorsSpace: CGColorSpaceCreateDeviceRGB(),
                colors: [
                    UIColor(red: 0.0, green: 0.0, blue: 0.0, alpha: 0.0).cgColor,
                    UIColor(red: 0.25, green: 0.12, blue: 0.08, alpha: 0.35).cgColor
                ] as CFArray,
                locations: nil
            )!
            cg.drawLinearGradient(shade, start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: size.height), options: [])

            cg.setStrokeColor(UIColor(red: 0.55, green: 0.34, blue: 0.26, alpha: 0.8).cgColor)
            for crease in [
                [CGPoint(x: 0.40, y: 0.58), CGPoint(x: 0.55, y: 0.60), CGPoint(x: 0.68, y: 0.57)],
                [CGPoint(x: 0.41, y: 0.66), CGPoint(x: 0.56, y: 0.675), CGPoint(x: 0.67, y: 0.65)],
                [CGPoint(x: 0.44, y: 0.74), CGPoint(x: 0.55, y: 0.75), CGPoint(x: 0.64, y: 0.72)]
            ] {
                cg.setLineWidth(7)
                cg.move(to: px(crease[0]))
                cg.addQuadCurve(to: px(crease[2]), control: px(crease[1]))
                cg.strokePath()
            }
        }
    }
}
#endif

