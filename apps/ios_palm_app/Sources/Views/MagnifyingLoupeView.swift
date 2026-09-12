import SwiftUI
import UIKit
import CurvPalmOverlay

/**
 * Apple-style precision magnifying loupe for palm crease line editing.
 *
 * Appears dynamically above the user's finger while dragging handles or curves
 * to completely eliminate finger occlusion and allow sub-millimeter precision.
 */
public struct MagnifyingLoupeView: View {
    public let image: UIImage?
    public let splines: [CreaseSpline]
    public let activeSplineId: String?
    public let targetNormPoint: CGPoint
    public let touchLocation: CGPoint
    public let canvasSize: CGSize
    public let accentColor: Color

    private let loupeDiameter: CGFloat = 96.0
    private let magnification: CGFloat = 2.2

    public init(
        image: UIImage?,
        splines: [CreaseSpline],
        activeSplineId: String?,
        targetNormPoint: CGPoint,
        touchLocation: CGPoint,
        canvasSize: CGSize,
        accentColor: Color
    ) {
        self.image = image
        self.splines = splines
        self.activeSplineId = activeSplineId
        self.targetNormPoint = targetNormPoint
        self.touchLocation = touchLocation
        self.canvasSize = canvasSize
        self.accentColor = accentColor
    }

    /// Position loupe 72pt above finger, or 72pt below if too close to top bar.
    private var loupeCenter: CGPoint {
        let x = min(max(touchLocation.x, loupeDiameter * 0.5 + 8), canvasSize.width - loupeDiameter * 0.5 - 8)
        let y: CGFloat
        if touchLocation.y < loupeDiameter + 40 {
            y = touchLocation.y + 76
        } else {
            y = touchLocation.y - 76
        }
        return CGPoint(x: x, y: y)
    }

    public var body: some View {
        ZStack {
            // Loupe Content
            ZStack {
                Color.black

                if let image {
                    // Render image centered on targetNormPoint with magnification
                    let imgAspect = image.size.width / max(image.size.height, 1)
                    let baseH = loupeDiameter * magnification * 2.8
                    let baseW = baseH * imgAspect

                    Image(uiImage: image)
                        .resizable()
                        .frame(width: baseW, height: baseH)
                        .position(
                            x: loupeDiameter * 0.5 + (0.5 - targetNormPoint.x) * baseW,
                            y: loupeDiameter * 0.5 + (0.5 - targetNormPoint.y) * baseH
                        )

                    // Splines inside loupe
                    Canvas { context, _ in
                        let loupeRect = CGRect(
                            x: loupeDiameter * 0.5 - targetNormPoint.x * baseW,
                            y: loupeDiameter * 0.5 - targetNormPoint.y * baseH,
                            width: baseW,
                            height: baseH
                        )

                        for spline in splines {
                            guard spline.points.count >= 2 else { continue }
                            let canvasPoints = spline.points.map { ImageCoordinateMapper.map($0, into: loupeRect) }
                            let path = Path.catmullRom(points: canvasPoints)
                            let isSelected = spline.id == activeSplineId
                            let color = Color(hex: spline.lineType.hexColor)

                            // Glow
                            context.stroke(
                                path,
                                with: .color(color.opacity(isSelected ? 0.35 : 0.12)),
                                style: StrokeStyle(lineWidth: isSelected ? 6 : 3, lineCap: .round, lineJoin: .round)
                            )
                            // Core line
                            context.stroke(
                                path,
                                with: .color(color.opacity(isSelected ? 0.95 : 0.40)),
                                style: StrokeStyle(lineWidth: isSelected ? 3.0 : 1.8, lineCap: .round, lineJoin: .round)
                            )
                        }
                    }
                }

                // Crosshair reticle (indicates exact center point)
                ZStack {
                    Circle()
                        .stroke(accentColor.opacity(0.9), lineWidth: 1.5)
                        .frame(width: 14, height: 14)

                    Circle()
                        .fill(Color.white)
                        .frame(width: 3.5, height: 3.5)

                    // Delicate tick marks
                    Path { p in
                        let r: CGFloat = 16
                        p.move(to: CGPoint(x: loupeDiameter * 0.5 - r, y: loupeDiameter * 0.5))
                        p.addLine(to: CGPoint(x: loupeDiameter * 0.5 - 9, y: loupeDiameter * 0.5))

                        p.move(to: CGPoint(x: loupeDiameter * 0.5 + 9, y: loupeDiameter * 0.5))
                        p.addLine(to: CGPoint(x: loupeDiameter * 0.5 + r, y: loupeDiameter * 0.5))

                        p.move(to: CGPoint(x: loupeDiameter * 0.5, y: loupeDiameter * 0.5 - r))
                        p.addLine(to: CGPoint(x: loupeDiameter * 0.5, y: loupeDiameter * 0.5 - 9))

                        p.move(to: CGPoint(x: loupeDiameter * 0.5, y: loupeDiameter * 0.5 + 9))
                        p.addLine(to: CGPoint(x: loupeDiameter * 0.5, y: loupeDiameter * 0.5 + r))
                    }
                    .stroke(Color.white.opacity(0.85), lineWidth: 1.0)
                }
            }
            .frame(width: loupeDiameter, height: loupeDiameter)
            .clipShape(Circle())
            // Glass bezel & contrast borders
            .overlay(
                Circle()
                    .stroke(Color.black.opacity(0.60), lineWidth: 3.5)
            )
            .overlay(
                Circle()
                    .stroke(
                        LinearGradient(
                            colors: [Color.white.opacity(0.9), Color.white.opacity(0.3)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        lineWidth: 1.5
                    )
            )
            .shadow(color: .black.opacity(0.55), radius: 10, y: 5)
            .position(loupeCenter)
        }
        .allowsHitTesting(false)
        .transition(.scale(scale: 0.7).combined(with: .opacity))
    }
}
