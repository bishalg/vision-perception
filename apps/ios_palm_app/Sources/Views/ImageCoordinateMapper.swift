import CoreGraphics
import Foundation

/**
 * Maps normalized image coordinates (0…1, top-left origin) into a view that
 * displays the image with aspect-fit or aspect-fill.
 */
enum ImageCoordinateMapper {
    enum ContentMode {
        case fit
        case fill
    }

    static func imageRect(
        imageSize: CGSize,
        viewSize: CGSize,
        mode: ContentMode
    ) -> CGRect {
        guard imageSize.width > 1, imageSize.height > 1,
              viewSize.width > 1, viewSize.height > 1
        else {
            return CGRect(origin: .zero, size: viewSize)
        }

        let imageAspect = imageSize.width / imageSize.height
        let viewAspect = viewSize.width / viewSize.height

        switch mode {
        case .fit:
            if imageAspect > viewAspect {
                let width = viewSize.width
                let height = width / imageAspect
                return CGRect(
                    x: 0,
                    y: (viewSize.height - height) * 0.5,
                    width: width,
                    height: height
                )
            } else {
                let height = viewSize.height
                let width = height * imageAspect
                return CGRect(
                    x: (viewSize.width - width) * 0.5,
                    y: 0,
                    width: width,
                    height: height
                )
            }
        case .fill:
            if imageAspect > viewAspect {
                let height = viewSize.height
                let width = height * imageAspect
                return CGRect(
                    x: (viewSize.width - width) * 0.5,
                    y: 0,
                    width: width,
                    height: height
                )
            } else {
                let width = viewSize.width
                let height = width / imageAspect
                return CGRect(
                    x: 0,
                    y: (viewSize.height - height) * 0.5,
                    width: width,
                    height: height
                )
            }
        }
    }

    static func map(_ point: CGPoint, into imageRect: CGRect) -> CGPoint {
        CGPoint(
            x: imageRect.minX + point.x * imageRect.width,
            y: imageRect.minY + point.y * imageRect.height
        )
    }

    /// Inverse of `map` — view point → normalized image coords (clamped 0…1).
    static func unmap(_ point: CGPoint, from imageRect: CGRect) -> CGPoint {
        guard imageRect.width > 1, imageRect.height > 1 else { return .zero }
        let nx = (point.x - imageRect.minX) / imageRect.width
        let ny = (point.y - imageRect.minY) / imageRect.height
        return CGPoint(
            x: min(1, max(0, nx)),
            y: min(1, max(0, ny))
        )
    }
}
