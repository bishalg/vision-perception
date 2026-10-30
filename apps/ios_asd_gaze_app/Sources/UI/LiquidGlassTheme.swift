import SwiftUI
import UIKit

/// Semantic palette and design tokens for the Liquid Glass aesthetic.
///
/// Every surface, text and accent token is *adaptive* and resolves to an
/// opaque sRGB colour: airy light surfaces with deep accents in light mode,
/// twilight surfaces with the bright brand accents in dark mode. Each
/// text/background pair meets WCAG AA (4.5:1), pinned by
/// `LiquidGlassContrastTests`; Increase Contrast swaps in ≥ 7:1 light accents.
/// Apply `.opacity` only at call sites, never inside a token, and only
/// `tintOpacity` under accent-coloured text. Fixed white is reserved for media
/// treatments directly over video, photo, or stimulus fields (`mediaDark()`).
public enum LiquidGlass {
    public static let celestialTeal = adaptive(light: 0x00655E, dark: 0x00D2C4, lightHighContrast: 0x004742)
    public static let warmAmber = adaptive(light: 0x7F4D00, dark: 0xFFB74D, lightHighContrast: 0x5B3700)
    public static let emeraldMint = adaptive(light: 0x006834, dark: 0x00E676, lightHighContrast: 0x004924)
    public static let deactivatedBrown = adaptive(light: 0x765034, dark: 0xD4A074, lightHighContrast: 0x543925)
    /// Atypical / stuck-item signal.
    public static let signalAtypical = adaptive(light: 0xA91414, dark: 0xFF8080, lightHighContrast: 0x7C0F0F)
    /// Text and icons on any solid accent fill.
    public static let onAccent = adaptive(light: 0xFFFFFF, dark: 0x0A0F1A)
    /// Flat fill for primary buttons (semantic alias of the brand teal).
    public static let accentFill = celestialTeal
    /// Disabled primary fill; its label uses `textTertiary`.
    public static let accentFillDisabled = adaptive(light: 0xC9D1DC, dark: 0x2F394B)
    /// The one opacity for a tinted fill that carries accent-coloured text.
    public static let tintOpacity: CGFloat = 0.14
    /// SceneKit wireframe. Matches celestial teal at 65% opacity.
    public static let celestialTealMesh = UIColor(red: 0.0, green: 0.824, blue: 0.769, alpha: 0.65)

    // Balloon calibration pairs. Fixed hues — they sit on the camera field.
    public static let balloonSky = Color(red: 0.0, green: 0.6, blue: 0.9)
    public static let balloonRose = Color(red: 1.0, green: 0.35, blue: 0.55)
    public static let balloonRoseDeep = Color(red: 0.9, green: 0.1, blue: 0.4)
    public static let balloonLeaf = Color(red: 0.3, green: 0.85, blue: 0.4)
    public static let balloonApricot = Color(red: 1.0, green: 0.65, blue: 0.2)
    public static let balloonViolet = Color(red: 0.6, green: 0.4, blue: 0.95)
    public static let balloonIndigo = Color(red: 0.4, green: 0.2, blue: 0.85)

    /// Dark studio backdrop behind the synthetic face mesh.
    public static let studioMeshTop = Color(red: 0.06, green: 0.10, blue: 0.16)
    public static let studioMeshBottom = Color(red: 0.02, green: 0.04, blue: 0.08)
    /// Fixed media panes for the scanpath hero. White labels sit on these.
    public static let stimulusSocialMedia = Color(red: 0.10, green: 0.16, blue: 0.28)
    public static let stimulusGeometryMedia = Color(red: 0.20, green: 0.08, blue: 0.24)
    public static let deepCanvas = adaptive(light: 0xF2F5FA, dark: 0x0A0F1A)
    public static let deepCanvasSecondary = adaptive(light: 0xE6ECF4, dark: 0x141C2E)
    /// Solid card fill.
    public static let cardBackground = adaptive(light: 0xFFFFFF, dark: 0x161D2B)
    /// Inner panel inside a card.
    public static let cardInset = adaptive(light: 0xEDF1F6, dark: 0x1F2838)
    public static let cardBorder = adaptive(light: 0xD5DCE5, dark: 0x2F394B)

    // MARK: - Central Typography & Text Colors (Single Source of Truth)
    public static let textPrimary = adaptive(light: 0x141A24, dark: 0xF4F6F8)
    public static let textSecondary = adaptive(light: 0x525B69, dark: 0xB4BAC4)
    /// Hints. Deliberately close to `textSecondary` in light mode: lighter fails AA.
    public static let textTertiary = adaptive(light: 0x5F6873, dark: 0x8E96A3)
    public static let textHeadline = adaptive(light: 0x0F141F, dark: 0xEDF0F4)
    public static let dividerColor = adaptive(light: 0xE3E8EE, dark: 0x26303F)

    // MARK: - Adaptive Stimulus Panes (Clinical Observation)
    public static let stimulusSocialTop = adaptive(
        dark: Color(red: 0.10, green: 0.16, blue: 0.28),
        light: Color(red: 0.88, green: 0.93, blue: 0.99)
    )
    public static let stimulusSocialBottom = adaptive(
        dark: Color(red: 0.05, green: 0.08, blue: 0.16),
        light: Color(red: 0.80, green: 0.88, blue: 0.97)
    )
    public static let stimulusGeometryTop = adaptive(
        dark: Color(red: 0.20, green: 0.08, blue: 0.24),
        light: Color(red: 0.95, green: 0.90, blue: 0.98)
    )
    public static let stimulusGeometryBottom = adaptive(
        dark: Color(red: 0.08, green: 0.04, blue: 0.12),
        light: Color(red: 0.88, green: 0.81, blue: 0.94)
    )

    public static let defaultCornerRadius: CGFloat = 16.0
}

/// Resolves per-environment so screens follow the system theme on launch.
private func adaptive(dark: Color, light: Color) -> Color {
    Color(UIColor { trait in
        trait.userInterfaceStyle == .dark ? UIColor(dark) : UIColor(light)
    })
}

/// Opaque sRGB token from 0xRRGGBB values; Increase Contrast picks
/// `lightHighContrast` in light mode when one is given.
private func adaptive(light: UInt32, dark: UInt32, lightHighContrast: UInt32? = nil) -> Color {
    Color(UIColor { trait in
        if trait.userInterfaceStyle == .dark { return UIColor(hex: dark) }
        if trait.accessibilityContrast == .high, let lightHighContrast { return UIColor(hex: lightHighContrast) }
        return UIColor(hex: light)
    })
}

private extension UIColor {
    convenience init(hex: UInt32) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: 1
        )
    }
}

public extension View {
    /// Pins a media-immersive root (camera, stimulus, replay) to dark so tokens,
    /// `.primary` and materials resolve light-on-dark over the media field.
    /// Never use `.preferredColorScheme` for this: it changes the whole window.
    func mediaDark() -> some View {
        environment(\.colorScheme, .dark)
    }

    /// Inner panel inside a card: `cardInset` fill with a `dividerColor` hairline.
    func glassInset(cornerRadius: CGFloat = 12) -> some View {
        background(
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .fill(LiquidGlass.cardInset)
        )
        .overlay(
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .stroke(LiquidGlass.dividerColor, lineWidth: 1)
        )
    }
}

/// ViewModifier implementing the Liquid Glass card styling.
public struct GlassCardModifier: ViewModifier {
    public var cornerRadius: CGFloat
    public var padding: CGFloat

    public init(cornerRadius: CGFloat = LiquidGlass.defaultCornerRadius, padding: CGFloat = 16.0) {
        self.cornerRadius = cornerRadius
        self.padding = padding
    }

    @Environment(\.colorScheme) private var colorScheme

    public func body(content: Content) -> some View {
        let isDark = colorScheme == .dark
        content
            .padding(padding)
            .background(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .fill(LiquidGlass.cardBackground)
            )
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(LiquidGlass.cardBorder, lineWidth: 1.0)
            )
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .shadow(
                color: Color.black.opacity(isDark ? 0.25 : 0.08),
                radius: isDark ? 10 : 8, x: 0, y: isDark ? 5 : 3
            )
    }
}

public extension View {
    func glassCard(cornerRadius: CGFloat = LiquidGlass.defaultCornerRadius, padding: CGFloat = 16.0) -> some View {
        modifier(GlassCardModifier(cornerRadius: cornerRadius, padding: padding))
    }
}

/// Frosted Glass Button styled for caregiver actions.
public struct GlassButton: View {
    public let title: String
    public let systemImage: String?
    public let isPrimary: Bool
    public let action: () -> Void

    public init(
        title: String,
        systemImage: String? = nil,
        isPrimary: Bool = true,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.systemImage = systemImage
        self.isPrimary = isPrimary
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let systemImage {
                    Image(systemName: systemImage)
                        .font(.system(size: 16, weight: .semibold))
                }
                Text(title)
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
            }
            .foregroundColor(isPrimary ? LiquidGlass.onAccent : LiquidGlass.textPrimary)
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
            .frame(maxWidth: .infinity)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(isPrimary ? LiquidGlass.accentFill : LiquidGlass.cardBackground)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(isPrimary ? Color.clear : LiquidGlass.cardBorder, lineWidth: 1)
            )
            .shadow(color: isPrimary ? LiquidGlass.accentFill.opacity(0.25) : .clear, radius: 8, y: 4)
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Single Source of Truth Typography Components

/// Standardized typography scale ensuring complete light and dark theme legibility.
public enum GlassTextStyle {
    case largeTitle     // 28pt bold rounded
    case title          // 20pt bold rounded
    case sectionHeader  // 14pt bold rounded
    case headline       // 14pt semibold
    case body           // 13pt regular
    case subheadline    // 12pt medium
    case caption        // 11pt regular
    case metricNumber   // 24pt bold rounded
}

public struct GlassTextModifier: ViewModifier {
    public let style: GlassTextStyle
    public let color: Color?

    public func body(content: Content) -> some View {
        switch style {
        case .largeTitle:
            content
                .font(.system(size: 28, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .title:
            content
                .font(.system(size: 20, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .sectionHeader:
            content
                .font(.system(size: 14, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textHeadline)
        case .headline:
            content
                .font(.system(size: 14, weight: .semibold, design: .default))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .body:
            content
                .font(.system(size: 13, weight: .regular, design: .default))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .subheadline:
            content
                .font(.system(size: 12, weight: .medium, design: .default))
                .foregroundColor(color ?? LiquidGlass.textSecondary)
        case .caption:
            content
                .font(.system(size: 11, weight: .regular, design: .default))
                .foregroundColor(color ?? LiquidGlass.textSecondary)
        case .metricNumber:
            content
                .font(.system(size: 24, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        }
    }
}

public extension View {
    func glassStyle(_ style: GlassTextStyle, color: Color? = nil) -> some View {
        modifier(GlassTextModifier(style: style, color: color))
    }
}

/// Centralized Section Header adhering strictly to Liquid Glass & Apple HIG themes.
/// Automatically adapts icon and typography for flawless contrast in both light and dark mode.
public struct GlassSectionHeader: View {
    public let title: String
    public let icon: String?
    public var iconColor: Color

    public init(title: String, icon: String? = nil, iconColor: Color = LiquidGlass.celestialTeal) {
        self.title = title
        self.icon = icon
        self.iconColor = iconColor
    }

    public var body: some View {
        HStack(spacing: 6) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(iconColor)
            }
            Text(title)
                .glassStyle(.sectionHeader)
        }
        .padding(.horizontal, 4)
    }
}

/// Theme-adaptive subtle Divider.
public struct GlassDivider: View {
    public init() {}

    public var body: some View {
        Divider()
            .overlay(LiquidGlass.dividerColor)
    }
}

/// Standardized hardware / sensor diagnostic row adhering to light and dark theme contrast.
public struct GlassDiagnosticRow: View {
    public let title: String
    public let subtitle: String
    public let statusText: String
    public let isGood: Bool

    public init(title: String, subtitle: String, statusText: String, isGood: Bool) {
        self.title = title
        self.subtitle = subtitle
        self.statusText = statusText
        self.isGood = isGood
    }

    public var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .glassStyle(.headline)
                Text(subtitle)
                    .glassStyle(.caption)
            }
            Spacer()
            Text(statusText)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Capsule().fill((isGood ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber).opacity(LiquidGlass.tintOpacity)))
                .foregroundColor(isGood ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
        }
    }
}

