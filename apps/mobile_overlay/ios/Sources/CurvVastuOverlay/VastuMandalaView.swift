import CurvCameraOverlay
import SwiftUI

/**
 * 9-pad Vastu Purusha Mandala drawn inside the shared HUD reticle.
 * Zone names match `domains/floor_plan` (`VastuMandalaOverlay`).
 */
public struct VastuMandalaView: View {
    public let isReady: Bool

    public init(isReady: Bool) {
        self.isReady = isReady
    }

    public var body: some View {
        GeometryReader { proxy in
            let inset: CGFloat = 18
            let grid = CGRect(
                x: inset,
                y: inset,
                width: proxy.size.width - inset * 2,
                height: proxy.size.height - inset * 2
            )
            let cellW = grid.width / 3
            let cellH = grid.height / 3
            let stroke = isReady ? OverlayPalette.emerald.opacity(0.85) : Color.white.opacity(0.45)

            ZStack {
                ForEach(0..<3, id: \.self) { row in
                    ForEach(0..<3, id: \.self) { col in
                        let zone = VastuZoneInfo.mandalaGrid[row][col]
                        let rect = CGRect(
                            x: grid.minX + CGFloat(col) * cellW,
                            y: grid.minY + CGFloat(row) * cellH,
                            width: cellW,
                            height: cellH
                        )
                        RoundedRectangle(cornerRadius: 4, style: .continuous)
                            .stroke(stroke, lineWidth: zone.key == "brahmasthan" ? 2.0 : 1.0)
                            .frame(width: rect.width - 4, height: rect.height - 4)
                            .position(x: rect.midX, y: rect.midY)

                        VStack(spacing: 2) {
                            Text(zone.name)
                                .font(.system(size: 9, weight: .semibold, design: .rounded))
                            Text(zone.element)
                                .font(.system(size: 8, weight: .regular))
                                .opacity(0.75)
                        }
                        .foregroundColor(isReady ? OverlayPalette.emerald.opacity(0.9) : Color.white.opacity(0.7))
                        .position(x: rect.midX, y: rect.midY)
                    }
                }

                Image(systemName: "location.north.line")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundColor(isReady ? OverlayPalette.emerald : Color.white.opacity(0.8))
                    .position(x: grid.midX, y: grid.minY - 2)
            }
        }
        .padding(8)
    }
}
