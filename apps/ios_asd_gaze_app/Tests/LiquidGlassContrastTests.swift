@testable import AsdGazeCollector
import SwiftUI
import UIKit
import XCTest

/// THM-8: every text/background token pair meets WCAG AA (4.5:1) in light and
/// dark appearance. The ratio is the contract, not the hex values.
final class LiquidGlassContrastTests: XCTestCase {
    private let styles: [UIUserInterfaceStyle] = [.light, .dark]

    private let textTokens: [(String, Color)] = [
        ("textPrimary", LiquidGlass.textPrimary),
        ("textHeadline", LiquidGlass.textHeadline),
        ("textSecondary", LiquidGlass.textSecondary),
        ("textTertiary", LiquidGlass.textTertiary),
    ]
    private let accents: [(String, Color)] = [
        ("celestialTeal", LiquidGlass.celestialTeal),
        ("emeraldMint", LiquidGlass.emeraldMint),
        ("warmAmber", LiquidGlass.warmAmber),
        ("signalAtypical", LiquidGlass.signalAtypical),
        ("deactivatedBrown", LiquidGlass.deactivatedBrown),
    ]
    private let surfaces: [(String, Color)] = [
        ("deepCanvas", LiquidGlass.deepCanvas),
        ("deepCanvasSecondary", LiquidGlass.deepCanvasSecondary),
        ("cardBackground", LiquidGlass.cardBackground),
        ("cardInset", LiquidGlass.cardInset),
    ]

    func testTextTokensPassAAOnEverySurface() {
        for style in styles {
            for (fgName, fg) in textTokens + accents {
                for (bgName, bg) in surfaces {
                    let r = ratio(resolve(fg, style), over: resolve(bg, style))
                    XCTAssertGreaterThanOrEqual(r, 4.5, "\(fgName) on \(bgName) (\(name(style))): \(fmt(r))")
                }
            }
        }
    }

    func testAccentsPassAAOnOwnTint() {
        for style in styles {
            for (fgName, fg) in accents {
                for (bgName, bg) in surfaces {
                    let accent = resolve(fg, style)
                    let tint = composite(accent, alpha: LiquidGlass.tintOpacity, over: resolve(bg, style))
                    let r = ratio(accent, over: tint)
                    XCTAssertGreaterThanOrEqual(r, 4.5, "\(fgName) on its tint over \(bgName) (\(name(style))): \(fmt(r))")
                }
            }
        }
    }

    func testOnAccentPassesAAOnSolidAccents() {
        for style in styles {
            for (fillName, fill) in accents + [("accentFill", LiquidGlass.accentFill)] {
                let r = ratio(resolve(LiquidGlass.onAccent, style), over: resolve(fill, style))
                XCTAssertGreaterThanOrEqual(r, 4.5, "onAccent on \(fillName) (\(name(style))): \(fmt(r))")
            }
        }
    }

    func testIncreaseContrastAccentsReachAAA() {
        let traits = UITraitCollection(traitsFrom: [
            UITraitCollection(userInterfaceStyle: .light),
            UITraitCollection(accessibilityContrast: .high),
        ])
        let white = UIColor(white: 1, alpha: 1)
        for (fgName, fg) in accents {
            let r = ratio(UIColor(fg).resolvedColor(with: traits), over: white)
            XCTAssertGreaterThanOrEqual(r, 7.0, "\(fgName) high contrast on white: \(fmt(r))")
        }
    }

    func testTokensAreOpaque() {
        let all = textTokens + accents + surfaces + [
            ("onAccent", LiquidGlass.onAccent),
            ("accentFillDisabled", LiquidGlass.accentFillDisabled),
            ("cardBorder", LiquidGlass.cardBorder),
            ("dividerColor", LiquidGlass.dividerColor),
        ]
        for style in styles {
            for (tokenName, token) in all {
                var alpha: CGFloat = 0
                resolve(token, style).getRed(nil, green: nil, blue: nil, alpha: &alpha)
                XCTAssertEqual(alpha, 1, accuracy: 0.001, "\(tokenName) is translucent in \(name(style))")
            }
        }
    }

    func testAccentsDifferBetweenModes() {
        for (tokenName, token) in accents {
            XCTAssertNotEqual(resolve(token, .light), resolve(token, .dark), "\(tokenName) is not adaptive")
        }
    }

    func testTealKeepsBrandHue() {
        var hue: CGFloat = 0
        resolve(LiquidGlass.celestialTeal, .dark).getHue(&hue, saturation: nil, brightness: nil, alpha: nil)
        XCTAssertEqual(hue * 360, 176, accuracy: 4)
        resolve(LiquidGlass.celestialTeal, .light).getHue(&hue, saturation: nil, brightness: nil, alpha: nil)
        XCTAssertEqual(hue * 360, 176, accuracy: 8)
    }

    // MARK: - WCAG 2.x helpers

    private func resolve(_ color: Color, _ style: UIUserInterfaceStyle) -> UIColor {
        UIColor(color).resolvedColor(with: UITraitCollection(userInterfaceStyle: style))
    }

    private func rgb(_ color: UIColor) -> (CGFloat, CGFloat, CGFloat) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        color.getRed(&r, green: &g, blue: &b, alpha: &a)
        return (r, g, b)
    }

    private func luminance(_ color: UIColor) -> CGFloat {
        func channel(_ c: CGFloat) -> CGFloat {
            c <= 0.03928 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4)
        }
        let (r, g, b) = rgb(color)
        return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }

    private func ratio(_ fg: UIColor, over bg: UIColor) -> CGFloat {
        let (l1, l2) = (luminance(fg), luminance(bg))
        return (max(l1, l2) + 0.05) / (min(l1, l2) + 0.05)
    }

    private func composite(_ fg: UIColor, alpha: CGFloat, over bg: UIColor) -> UIColor {
        let (fr, fgG, fb) = rgb(fg)
        let (br, bgG, bb) = rgb(bg)
        return UIColor(
            red: fr * alpha + br * (1 - alpha),
            green: fgG * alpha + bgG * (1 - alpha),
            blue: fb * alpha + bb * (1 - alpha),
            alpha: 1
        )
    }

    private func name(_ style: UIUserInterfaceStyle) -> String { style == .dark ? "dark" : "light" }
    private func fmt(_ r: CGFloat) -> String { String(format: "%.2f", r) }
}
